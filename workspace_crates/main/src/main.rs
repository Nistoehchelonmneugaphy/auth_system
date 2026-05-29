use std::path::PathBuf;

use strum::{EnumString, IntoStaticStr};
use derive_more::{Deref, DerefMut, From, FromStr, Into};

use app_config::AppConfig;
use server::{
    BackgroundSpawnTasks,
    config_reader::read_config,
    http_stuff::{
        http_request_parsing::HttpRequestData,
        http_response::{HttpResponseData, ok_html},
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
                    println!("ParseError: {e:#?}");
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
            return handle_secret_api_call(
                action,
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



#[derive(Debug, Clone, derive_more::Display, From, Into, FromStr, Deref, DerefMut)]
pub struct APIPrefix(String);
#[derive(Debug, Clone, derive_more::Display, From, Into, FromStr, Deref, DerefMut)]
pub struct APIAction(String);
#[derive(Debug, EnumString, strum::Display, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum PublicAPIActionName {
    Login,
    VerifySessionToken,
}
pub enum InternalAPIAction {
    AddSubscribedUser,
    DeleteSubscribedUser,
}

#[derive(Debug, Clone)]
struct APIPath {
    prefix: APIPrefix,
    action: APIAction,
}
fn parse_path(path: &str) -> Option<APIPath> {
    let parts: Vec<&str> = path.trim_start_matches("/").splitn(2, '/').collect();
    match parts.as_slice() {
        [prefix, action] => Some(APIPath {
            prefix: APIPrefix(prefix.to_string()),
            action: APIAction(action.to_string()),
        }),
        _ => None,
    }
}
async fn handle_public_api_call(
    action: PublicAPIActionName,
    request_data: HttpRequestData,
    db_pool: Pool<Postgres>,
    server_config: ServerConfig,
    app_config: AppConfig,
) -> HttpResponseData {
    match action {
        PublicAPIActionName::Login => {
            todo!()
        },
        PublicAPIActionName::VerifySessionToken => todo!(),
    }
}
async fn handle_secret_api_call(
    action: APIAction,
    request_data: HttpRequestData,
    db_pool: Pool<Postgres>,
    server_config: ServerConfig,
    app_config: AppConfig,
) -> HttpResponseData {
    todo!();
}
