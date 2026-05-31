pub mod internal_api {
    #[derive(Debug, strum::EnumString, strum::Display, strum::IntoStaticStr)]
    #[strum(serialize_all = "snake_case")]
    pub enum InternalAPIAction {
        AddOrUpdateUser,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct AddOrUpdateUserRequest {
        pub user_id: String,
        pub username: String,
        pub password_hash: String,
        pub subscription_expires_at_timestamp: u64,
    }
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub enum AddOrUpdateUserResponse {
        Added,
        Updated,
    }
}

pub mod public_api {
    #[derive(Debug, strum::EnumString, strum::Display, strum::IntoStaticStr)]
    #[strum(serialize_all = "snake_case")]
    pub enum PublicAPIAction {
        Login,
        ValidateSessionToken,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct LoginRequest {
        pub username: String,
        pub password: String,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct LoginSuccess {
        pub token: String,
    }
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct LoginError {
        pub kind: LoginErrorKind,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub enum LoginErrorKind {
        InvalidCredentials,
        SubscriptionExpired,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub enum LoginResponse {
        Success(LoginSuccess),
        Error(LoginError),
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct VerifySessionTokenRequest {
        pub token: String,
    }
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct VerifySessionTokenResponse {
        pub valid: bool,
    }
}
 
 /*
 
  curl -X POST http://localhost:55555/public/login -H "content-type: application/json" -d '{"username": "test", "password": "11111111111"}'
  password 11111111111 (11 ones)
  curl -X POST http://localhost:55555/secret_prefix_123/add_or_update_user -H "content-type: application/json" -d '{"username": "test", "password_hash": "$argon2id$v=19$m=16000,t=2,p=2$Zs3HqI3ykNBixwG4OK7TEZJo8Vb2rTAI0m8VPAvGwaXYImxBUxcY27NmXPOO/sr0$kLDLHR33Z/FtobDJWN7P2Wl0yWDtoI2LlVqlde3yvnc", "user_id": "id123", "subscription_expires_at_timestamp":99999999999}' -H "x-api-key: key_123"

  invalid test:
    curl -X POST http://localhost:55555/public/login -H "content-type: application/json" -d '{"username": "test", "password": "34324e11"}'
  */