#[derive(Debug, strum::EnumString, strum::Display, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum PublicAPIAction {
    Login,
    VerifySessionToken,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PublicAPILoginRequest {
    username: String,
    password: String,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PublicAPIVerifySessionTokenRequest {
    token: String,
}
