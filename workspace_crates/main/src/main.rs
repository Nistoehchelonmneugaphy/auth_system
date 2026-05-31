use std::path::PathBuf;

use auth_server_structs::public_api::{PublicAPIAction, VerifySessionTokenResponse};
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
            return ok_html("Invalid request...".to_string());
        }
    };
    match prefix.as_str() {
        "public" => {
            let public_action = match action.parse() {
                Ok(pa) => pa,
                Err(e) => {
                    println!("Could not parse action {action:#?}: {e:#?}");
                    return ok_html("Invalid action".to_string());
                }
            };
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
            return handle_internal_api_call(
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
    let parts: Vec<&str> = path.trim_start_matches("/").splitn(1, '/').collect();
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
    server_config: ServerConfig,
    app_config: AppConfig,
) -> HttpResponseData {
    match public_action {
        PublicAPIAction::Login => {
            let req: auth_server_structs::public_api::LoginRequest =
                match serde_json::from_slice(&request_data.body_bytes) {
                    Ok(r) => r,
                    Err(e) => {
                        println!("{e:#?}");
                        return ok_html("Could not parse the request".to_string());
                    }
                };
            let input_password_hash = 
            todo!()
        }
        PublicAPIAction::ValidateSessionToken => {
            let req: auth_server_structs::public_api::VerifySessionTokenRequest =
                match serde_json::from_slice(&request_data.body_bytes) {
                    Ok(r) => r,
                    Err(e) => {
                        println!("{e:#?}");
                        return ok_html("Could not parse the request".to_string());
                    }
                };
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
                    let Ok(expiry) = u64::try_from(expiry) else {
                        println!("Parse error");
                        return ok_html("Some error".to_string());
                    };
                    let Ok(curr_timestamp) = std::time::SystemTime::UNIX_EPOCH
                        .elapsed()
                        .map(|d| d.as_secs())
                    else {
                        return ok_html("Error".to_string());
                    };

                    let valid = expiry > curr_timestamp;
                    let resp = VerifySessionTokenResponse { valid };
                    let bytes = match serde_json::to_vec(&resp) {
                        Ok(b) => b,
                        Err(e) => {
                            println!("{e:#?}");
                            return ok_html("Error.".to_string());
                        }
                    };
                    return ok(bytes, ApplicationJson, Some(Utf8));
                }
                Ok(None) => {
                    let resp = VerifySessionTokenResponse { valid: false };
                    let bytes = match serde_json::to_vec(&resp) {
                        Ok(b) => b,
                        Err(e) => {
                            println!("{e:#?}");
                            return ok_html("Error.".to_string());
                        }
                    };
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
    request_data: HttpRequestData,
    db_pool: Pool<Postgres>,
    server_config: ServerConfig,
    app_config: AppConfig,
) -> HttpResponseData {
    todo!();
}
