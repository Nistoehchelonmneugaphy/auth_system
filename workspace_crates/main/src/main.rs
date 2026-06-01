use std::{path::PathBuf, time::SystemTimeError};

use auth_system_structs::{
    internal_api::{AddOrUpdateUserResponse, InternalAPIAction},
    public_api::{
        LoginError, LoginErrorKind, LoginResponse, LoginSuccess, PublicAPIAction, VerifySessionTokenResponse
    },
};
use derive_more::{Deref, DerefMut, From, FromStr, Into};

use app_config::AppConfig;
use server::{
    BackgroundSpawnTasks,
    config_reader::read_config,
    http_stuff::{
        http_request_parsing::HttpRequestData,
        http_response::{
            Charset::Utf8, ContentType::ApplicationJson, HttpResponseData, ok, ok_html,
        },
    },
    server_config::ServerConfig,
};
use sqlx::{Pool, Postgres};

macro_rules! ok_or_return_error_msg {
    ($expr:expr $(, $msg:expr)?) => {
        match $expr {
            Ok(v) => v,
            Err(e) => {
                println!("Err: {e:#?}");
                $(
                    println!("Msg: {:#?}", $msg);
                )?
                return server::http_stuff::http_response::ok_html("Error".to_string());
            }
        }
    };
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    let app_config = read_config::<AppConfig>(PathBuf::from(".env"));
    let db_pool = sqlx::PgPool::connect(&app_config.database_url)
        .await
        .expect("I expected pool construction to succeed :)");
    let handler: server::run::HandlerFn =
        std::sync::Arc::new(move |request_data, server_config| {
            let db_pool = db_pool.clone();
            let app_config = app_config.clone();
            Box::pin(async move { route(request_data, db_pool, server_config, app_config).await })
        });
    server::run(
        env!("CARGO_PKG_VERSION").to_string(),
        handler,
        BackgroundSpawnTasks {
            new_tasks_to_spawn: None,
            cancellation_token: None,
        },
    )
    .await
}

async fn route(
    request_data: HttpRequestData,
    db_pool: Pool<Postgres>,
    server_config: ServerConfig,
    app_config: AppConfig,
) -> HttpResponseData {
    let headers = request_data.headers.clone();
    let path = request_data.path.clone();

    let parts = parse_path(&path);
    let APIPath { prefix, action } = match parts {
        Some(p) => p,
        None => {
            return ok_html("Invalid parts...".to_string());
        }
    };
    match prefix.as_str() {
        "public" => {
            let public_action = ok_or_return_error_msg!(action.parse(), "Invalid action");
            return handle_public_api_call(
                public_action,
                request_data.clone(),
                db_pool.clone(),
                server_config.clone(),
                app_config.clone(),
            )
            .await;
        }
        p if p == app_config.secret_api_prefix => {
            let api_key_valid = verify_api_key(
                headers.get("x-api-key").unwrap_or_default().to_owned(),
                app_config.api_key.clone(),
            );
            if !api_key_valid {
                return ok_html("Invalid API key".to_string());
            }
            let internal_action =
                ok_or_return_error_msg!(action.parse(), "Invalid internal action");
            return handle_internal_api_call(
                internal_action,
                request_data.clone(),
                db_pool.clone(),
                server_config.clone(),
                app_config.clone(),
            )
            .await;
        }
        _ => {
            println!("Prefix part is {prefix:#?} and it is unknown");
            return ok_html("Invalid request...".to_string());
        }
    }
}

fn verify_api_key(s: String, key: String) -> bool {
    s == key
}

#[derive(Debug, Clone, derive_more::Display, From, Into, FromStr, Deref, DerefMut)]
pub struct APIPrefix(String);
#[derive(Debug, Clone, derive_more::Display, From, Into, FromStr, Deref, DerefMut)]
pub struct APIAction(String);

#[derive(Debug, Clone)]
struct APIPath {
    prefix: APIPrefix,
    action: APIAction,
}
fn parse_path(path: &str) -> Option<APIPath> {
    let parts: Vec<&str> = path.trim_start_matches("/").splitn(2, '/').collect();
    println!("{parts:#?}");
    match parts.as_slice() {
        [prefix, action] => Some(APIPath {
            prefix: APIPrefix(prefix.to_string()),
            action: APIAction(action.to_string()),
        }),
        _ => None,
    }
}
async fn handle_public_api_call(
    public_action: PublicAPIAction,
    request_data: HttpRequestData,
    db_pool: Pool<Postgres>,
    _server_config: ServerConfig,
    _app_config: AppConfig,
) -> HttpResponseData {
    match public_action {
        PublicAPIAction::Login => {
            let invalid_creds_resp = LoginResponse::Error(LoginError {
                kind: LoginErrorKind::InvalidCredentials,
            });
            let sub_expired_resp = LoginResponse::Error(LoginError {
                kind: LoginErrorKind::SubscriptionExpired,
            });
            let req: auth_system_structs::public_api::LoginRequest =
                ok_or_return_error_msg!(serde_json::from_slice(&request_data.body_bytes));
            let user_data = ok_or_return_error_msg!(sqlx::query!("SELECT user_id, password_hash, subscription_expires_at_timestamp FROM users WHERE username = $1", req.username).fetch_optional(&db_pool).await);
            let user_data = match user_data {
                Some(ud) => ud,
                None => {
                    let resp = invalid_creds_resp.clone();
                    let bytes = ok_or_return_error_msg!(serde_json::to_vec(&resp));
                    return ok(bytes, ApplicationJson, Some(Utf8));
                }
            };

            let password_validation = ok_or_return_error_msg!(password_hashing::verify_password(
                req.password.clone(),
                user_data.password_hash
            ));
            let curr_timestamp = ok_or_return_error_msg!(
                std::time::SystemTime::UNIX_EPOCH
                    .elapsed()
                    .map(|d| d.as_secs())
            );
            let subscription_expires_at_timestamp =
                ok_or_return_error_msg!(u64::try_from(user_data.subscription_expires_at_timestamp));
            let subscription_active = subscription_expires_at_timestamp > curr_timestamp;
            let resp = match password_validation {
                true => match subscription_active {
                    true => {
                        const ONE_DAY: u64 = 60 * 60 * 24;
                        let expires_at_timestamp = ok_or_return_error_msg!(i64::try_from(
                            (ok_or_return_error_msg!(current_timestamp()) + ONE_DAY)
                        ));
                        let row = ok_or_return_error_msg!(
                            sqlx::query!(
                                r#"
    INSERT INTO session_tokens (token, user_id, expires_at_timestamp)
    SELECT
        gen_random_uuid()::text,
        u.user_id,
        $2
    FROM users u
    WHERE u.username = $1
      AND u.subscription_expires_at_timestamp > extract(epoch from now())::bigint
    RETURNING token, user_id
    "#,
                                req.username,               
                                expires_at_timestamp  
                            )
                            .fetch_one(&db_pool)
                            .await
                        );
                    LoginResponse::Success(LoginSuccess { token: row.token, username: req.username, user_id: row.user_id })
                    }
                    false => sub_expired_resp.clone(),
                },
                false => invalid_creds_resp.clone(),
            };
            let bytes = ok_or_return_error_msg!(serde_json::to_vec(&resp));
            return ok(bytes, ApplicationJson, Some(Utf8));
        }
        PublicAPIAction::ValidateSessionToken => {
            let req: auth_system_structs::public_api::VerifySessionTokenRequest =
                ok_or_return_error_msg!(serde_json::from_slice(&request_data.body_bytes));
            let session_token = req.token;
            match sqlx::query!(
                "SELECT token, user_id, expires_at_timestamp FROM session_tokens WHERE token = $1",
                session_token
            )
            .fetch_optional(&db_pool)
            .await
            {
                Ok(Some(td)) => {
                    let expiry = td.expires_at_timestamp;
                    let expiry = ok_or_return_error_msg!(u64::try_from(expiry));
                    let curr_timestamp = ok_or_return_error_msg!(
                        std::time::SystemTime::UNIX_EPOCH
                            .elapsed()
                            .map(|d| d.as_secs())
                    );

                    let valid = expiry > curr_timestamp;
                    let resp = VerifySessionTokenResponse { valid };
                    let bytes = ok_or_return_error_msg!(serde_json::to_vec(&resp));
                    return ok(bytes, ApplicationJson, Some(Utf8));
                }
                Ok(None) => {
                    let resp = VerifySessionTokenResponse { valid: false };
                    let bytes = ok_or_return_error_msg!(serde_json::to_vec(&resp));
                    return ok(bytes, ApplicationJson, Some(Utf8));
                }
                Err(e) => {
                    println!("DB Error: {e:#?}");
                    return ok_html("Some error has occurred.".to_string());
                }
            }
        }
    }
}
async fn handle_internal_api_call(
    internal_action: InternalAPIAction,
    request_data: HttpRequestData,
    db_pool: Pool<Postgres>,
    _server_config: ServerConfig,
    _app_config: AppConfig,
) -> HttpResponseData {
    match internal_action {
        InternalAPIAction::AddOrUpdateUser => {
            let req: auth_system_structs::internal_api::AddOrUpdateUserRequest = ok_or_return_error_msg!(
                serde_json::from_slice(&request_data.body_bytes),
                "Failed to shape the json req into struct"
            );
            let subscription_expires_at_timestamp =
                ok_or_return_error_msg!(i64::try_from(req.subscription_expires_at_timestamp));
            let user_data = ok_or_return_error_msg!(
                sqlx::query!("SELECT user_id FROM users WHERE user_id = $1", req.user_id)
                    .fetch_optional(&db_pool)
                    .await,
                "DB Err"
            );
            let resp = match user_data {
                None => {
                    ok_or_return_error_msg!(sqlx::query!("INSERT INTO users (user_id, username, password_hash, subscription_expires_at_timestamp) VALUES ($1, $2, $3, $4)", req.user_id, req.username, req.password_hash, subscription_expires_at_timestamp).execute(&db_pool).await);
                    AddOrUpdateUserResponse::Added
                }
                Some(_) => {
                    ok_or_return_error_msg!(sqlx::query!("UPDATE users SET username = $1, password_hash = $2, subscription_expires_at_timestamp = $3 WHERE user_id = $4", req.username, req.password_hash, subscription_expires_at_timestamp, req.user_id).execute(&db_pool).await);
                    AddOrUpdateUserResponse::Updated
                }
            };
            let bytes = ok_or_return_error_msg!(serde_json::to_vec(&resp));
            return ok(bytes, ApplicationJson, Some(Utf8));
        }
    }
}

fn current_timestamp() -> Result<u64, SystemTimeError> {
    std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .map(|d| d.as_secs())
}
