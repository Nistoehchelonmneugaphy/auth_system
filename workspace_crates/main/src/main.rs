use server::{BackgroundSpawnTasks, http_stuff::http_response::ok_html};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    let handler: server::run::HandlerFn =
        std::sync::Arc::new(move |_request_data, _server_config| {
    
            Box::pin(async move {
                ok_html("Hey there".to_string())
            })
        });
    server::run(env!("CARGO_PKG_VERSION").to_string(), handler, BackgroundSpawnTasks { new_tasks_to_spawn: None, cancellation_token: None}).await
}