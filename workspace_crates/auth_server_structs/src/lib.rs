pub mod public_api {
    #[derive(Debug, strum::EnumString, strum::Display, strum::IntoStaticStr)]
    #[strum(serialize_all = "snake_case")]
    pub enum PublicAPIAction {
        Login,
        ValidatePermission,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct LoginRequest {
        username: String,
        password: String,
    }
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct VerifySessionTokenRequest {
        token: String,
    }
}
