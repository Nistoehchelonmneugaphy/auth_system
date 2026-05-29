use std::path::PathBuf;

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
    ok_html(format!("Headers: {headers:#?}, <br> Path: {path:#?} <br>"))
}
