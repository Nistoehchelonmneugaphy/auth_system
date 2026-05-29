#[derive(Debug, Clone, serde::Deserialize)]
pub struct AppConfig {
    pub secret_api_prefix: String,
    pub api_key: String,
    pub database_url: String,

}
