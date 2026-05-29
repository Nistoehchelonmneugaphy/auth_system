#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PublicAPIRequest {
    Login { username: String, password: String },
    VerifySessionToken { token: String },
}


