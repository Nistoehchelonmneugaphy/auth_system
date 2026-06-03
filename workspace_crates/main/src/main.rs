use std::{path::PathBuf, time::SystemTimeError};

use auth_system_structs::{
    internal_api::{
        AddOrUpdateUserResponse, AddTestUserWithPlaintextPasswordResponse,
        InternalAPIAction::{self},
    },
    public_api::{
        CreateJoinTokenError, CreateJoinTokenErrorKind,
        CreateJoinTokenResponse::{self, Success},
        CreateJoinTokenSuccess, LoginError, LoginErrorKind, LoginResponse, LoginSuccess,
        PublicAPIAction::{self},
        ValidateJoinTokenResponse, ValidateJoinTokenSuccess, VerifySessionTokenResponse,
    },
};
use derive_more::{Deref, DerefMut, From, FromStr, Into};

use app_config::AppConfig;
use password_hashing::{PasswordHasherConfig, PasswordHasherConfigParams};
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

macro_rules! ok_or_return_error_http_resp {
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
macro_rules! ok_or_return_err_string {
    ($expr:expr $(, $msg:expr)?) => {
        match $expr {
            Ok(v) => v,
            Err(e) => {
                println!("Err: {e:#?}");
                $(
                    println!("Msg: {:#?}", $msg);
                )?
                return Err(String::from("Error"));
            }
        }
    };
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    let app_config = read_config::<AppConfig>(PathBuf::from(".env"));
    let app_config_clone_for_run = app_config.clone();
    let db_pool = sqlx::PgPool::connect(&app_config.database_url)
        .await
        .expect("I expected pool construction to succeed :)");
    let handler: server::run::HandlerFn =
        std::sync::Arc::new(move |request_data, server_config| {
            let db_pool = db_pool.clone();
            let app_config = app_config.clone();
            Box::pin(async move { route(request_data, db_pool, server_config, app_config).await })
        });
    let server_config = ServerConfig::new(
        app_config_clone_for_run.ip,
        app_config_clone_for_run.port,
        app_config_clone_for_run.rust_log_directory_path,
    );
    server::run(
        server_config,
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
            let public_action = ok_or_return_error_http_resp!(action.parse(), "Invalid action");
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
                ok_or_return_error_http_resp!(action.parse(), "Invalid internal action");
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
                ok_or_return_error_http_resp!(serde_json::from_slice(&request_data.body_bytes));
            let user_data = ok_or_return_error_http_resp!(sqlx::query!("SELECT user_id, password_hash, subscription_expires_at_timestamp FROM users WHERE username = $1", req.username).fetch_optional(&db_pool).await);
            let user_data = match user_data {
                Some(ud) => ud,
                None => {
                    let resp = invalid_creds_resp.clone();
                    let bytes = ok_or_return_error_http_resp!(serde_json::to_vec(&resp));
                    return ok(bytes, ApplicationJson, Some(Utf8));
                }
            };

            let password_validation = ok_or_return_error_http_resp!(
                password_hashing::verify_password(req.password.clone(), user_data.password_hash)
            );
            let curr_timestamp = ok_or_return_error_http_resp!(
                std::time::SystemTime::UNIX_EPOCH
                    .elapsed()
                    .map(|d| d.as_secs())
            );
            let subscription_expires_at_timestamp = ok_or_return_error_http_resp!(u64::try_from(
                user_data.subscription_expires_at_timestamp
            ));
            let subscription_active = subscription_expires_at_timestamp > curr_timestamp;
            let resp = match password_validation {
                true => match subscription_active {
                    true => {
                        const ONE_DAY: u64 = 60 * 60 * 24;
                        let expires_at_timestamp = ok_or_return_error_http_resp!(i64::try_from(
                            (ok_or_return_error_http_resp!(current_timestamp()) + ONE_DAY)
                        ));
                        let row = ok_or_return_error_http_resp!(
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
                        LoginResponse::Success(LoginSuccess {
                            token: row.token,
                            username: req.username,
                            user_id: row.user_id,
                        })
                    }
                    false => sub_expired_resp.clone(),
                },
                false => invalid_creds_resp.clone(),
            };
            let bytes = ok_or_return_error_http_resp!(serde_json::to_vec(&resp));
            return ok(bytes, ApplicationJson, Some(Utf8));
        }
        PublicAPIAction::ValidateSessionToken => {
            let req: auth_system_structs::public_api::VerifySessionTokenRequest =
                ok_or_return_error_http_resp!(serde_json::from_slice(&request_data.body_bytes));
            let session_token = req.token;
            let _session_token_validation = ok_or_return_error_http_resp!(
                validate_session_token(db_pool.clone(), session_token.clone()).await
            );
            let resp = VerifySessionTokenResponse { valid: true };
            let bytes = ok_or_return_error_http_resp!(serde_json::to_vec(&resp));
            return ok(bytes, ApplicationJson, Some(Utf8));
        }
        PublicAPIAction::CreateJoinToken => {
            let req: auth_system_structs::public_api::CreateJoinTokenRequest =
                ok_or_return_error_http_resp!(serde_json::from_slice(&request_data.body_bytes));
            let session_token = req.session_token;
            // first of all, validate the session_token
            // then insert a new join token, linking it with the server nonce and the session token.
            // return the join token.

            match validate_session_token(db_pool.clone(), session_token.clone()).await {
                Err(_) => {
                    let resp = CreateJoinTokenResponse::Error(CreateJoinTokenError {
                        kind: CreateJoinTokenErrorKind::InvalidSessionToken,
                    });
                    let bytes = ok_or_return_error_http_resp!(serde_json::to_vec(&resp));
                    return ok(bytes, ApplicationJson, Some(Utf8));
                }
                Ok(t) => {
                    let join_token = ok_or_return_error_http_resp!(
                        create_join_token(db_pool.clone(), t, req.server_nonce).await
                    );
                    let resp = Success(CreateJoinTokenSuccess { join_token });
                    let bytes = ok_or_return_error_http_resp!(serde_json::to_vec(&resp));
                    return ok(bytes, ApplicationJson, Some(Utf8));
                }
            }
        }
        PublicAPIAction::ValidateJoinToken => {
            let req: auth_system_structs::public_api::ValidateJoinTokenRequest =
                ok_or_return_error_http_resp!(serde_json::from_slice(&request_data.body_bytes));
            let join_token = req.join_token;
            let server_nonce = req.server_nonce;
            let row = ok_or_return_error_http_resp!(
                sqlx::query!(
                    "SELECT jt.primary_key, u.user_id
         FROM join_tokens jt
         JOIN session_tokens st ON st.token = jt.session_token
         JOIN users u ON u.user_id = st.user_id
         WHERE jt.join_token = $1 AND jt.server_nonce = $2",
                    join_token,
                    server_nonce
                )
                .fetch_optional(&db_pool)
                .await
            );
            let resp = match row {
                Some(r) => {
                    let user_id = r.user_id;
                    ValidateJoinTokenResponse::Valid(ValidateJoinTokenSuccess { user_id })
                }
                None => ValidateJoinTokenResponse::Invalid,
            };
            let bytes = ok_or_return_error_http_resp!(serde_json::to_vec(&resp));
            return ok(bytes, ApplicationJson, Some(Utf8));
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
            let req: auth_system_structs::internal_api::AddOrUpdateUserRequest = ok_or_return_error_http_resp!(
                serde_json::from_slice(&request_data.body_bytes),
                "Failed to shape the json req into struct"
            );
            let subscription_expires_at_timestamp =
                ok_or_return_error_http_resp!(i64::try_from(req.subscription_expires_at_timestamp));
            let user_data = ok_or_return_error_http_resp!(
                sqlx::query!("SELECT user_id FROM users WHERE user_id = $1", req.user_id)
                    .fetch_optional(&db_pool)
                    .await,
                "DB Err"
            );
            let resp = match user_data {
                None => {
                    ok_or_return_error_http_resp!(sqlx::query!("INSERT INTO users (user_id, username, password_hash, subscription_expires_at_timestamp) VALUES ($1, $2, $3, $4)", req.user_id, req.username, req.password_hash, subscription_expires_at_timestamp).execute(&db_pool).await);
                    AddOrUpdateUserResponse::Added
                }
                Some(_) => {
                    ok_or_return_error_http_resp!(sqlx::query!("UPDATE users SET username = $1, password_hash = $2, subscription_expires_at_timestamp = $3 WHERE user_id = $4", req.username, req.password_hash, subscription_expires_at_timestamp, req.user_id).execute(&db_pool).await);
                    AddOrUpdateUserResponse::Updated
                }
            };
            let bytes = ok_or_return_error_http_resp!(serde_json::to_vec(&resp));
            return ok(bytes, ApplicationJson, Some(Utf8));
        }
        InternalAPIAction::AddTestUserWithPlaintextPassword => {
            let req: auth_system_structs::internal_api::AddTestUserWithPlaintextPasswordRequest = ok_or_return_error_http_resp!(
                serde_json::from_slice(&request_data.body_bytes),
                "Failed to shape the json req into struct"
            );
            let password_hash = ok_or_return_error_http_resp!(password_hashing::hash_password(
                req.password,
                get_a_password_hasher_config()
            ));
            ok_or_return_error_http_resp!(sqlx::query!("INSERT INTO users (user_id, username, password_hash, subscription_expires_at_timestamp) VALUES (gen_random_uuid()::text, $1, $2, $3)",  req.username, password_hash, 999999999999).execute(&db_pool).await);
            let resp = AddTestUserWithPlaintextPasswordResponse::Added;
            let bytes = ok_or_return_error_http_resp!(serde_json::to_vec(&resp));
            return ok(bytes, ApplicationJson, Some(Utf8));
        }
    }
}

fn current_timestamp() -> Result<u64, SystemTimeError> {
    std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .map(|d| d.as_secs())
}

fn get_a_password_hasher_config() -> PasswordHasherConfig {
    PasswordHasherConfig::new(PasswordHasherConfigParams {
        memory_cost: 16000,
        time_cost: 2,
        parallelism_cost: 2,
    })
}

#[derive(Clone, Debug)]
struct ValidSessionToken(String);

async fn validate_session_token(
    db_pool: Pool<Postgres>,
    session_token: String,
) -> Result<ValidSessionToken, String> {
    let valid = match sqlx::query!(
        "SELECT token, user_id, expires_at_timestamp FROM session_tokens WHERE token = $1",
        session_token
    )
    .fetch_optional(&db_pool)
    .await
    {
        Ok(Some(td)) => {
            let expiry = td.expires_at_timestamp;
            let expiry = ok_or_return_err_string!(u64::try_from(expiry));
            let curr_timestamp = ok_or_return_err_string!(
                std::time::SystemTime::UNIX_EPOCH
                    .elapsed()
                    .map(|d| d.as_secs())
            );

            expiry > curr_timestamp
        }
        Ok(None) => false,
        Err(e) => {
            println!("DB Error: {e:#?}");
            return ok_or_return_err_string!(Err("DB Error"));
        }
    };
    match valid {
        true => Ok(ValidSessionToken(session_token)),
        false => return Err("Invalid token".to_string()),
    }
}

async fn create_join_token(
    db_pool: Pool<Postgres>,
    session_token: ValidSessionToken,
    server_nonce: String,
) -> Result<String, String> {
    let row = ok_or_return_err_string!(
        sqlx::query!(
            "INSERT INTO join_tokens (server_nonce, session_token, join_token)
     VALUES ($1, $2, gen_random_uuid()::text)
     RETURNING join_token",
            server_nonce,
            session_token.0
        )
        .fetch_one(&db_pool)
        .await
    );
    let join_token = row.join_token;
    Ok(join_token)
}
