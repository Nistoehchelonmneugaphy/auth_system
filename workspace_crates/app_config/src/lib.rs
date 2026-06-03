#[derive(Debug, Clone, serde::Deserialize)]
pub struct AppConfig {
    pub ip: String,
    pub port: u64,
    pub rust_log_directory_path: String,
    pub secret_api_prefix: String,
    pub api_key: String,
    pub database_url: String,

}
