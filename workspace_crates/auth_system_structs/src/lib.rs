pub mod internal_api {
    #[derive(Debug, strum::EnumString, strum::Display, strum::IntoStaticStr)]
    #[strum(serialize_all = "snake_case")]
    pub enum InternalAPIAction {
        AddOrUpdateUser,
        AddTestUserWithPlaintextPassword,
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
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct AddTestUserWithPlaintextPasswordRequest {
        pub username: String,
        pub password: String,
    }
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub enum AddTestUserWithPlaintextPasswordResponse {
        Added,
    }
}

pub mod public_api {
    pub fn get_full_url(auth_server_url: String, api_action: PublicAPIAction) -> String {
        format!("{auth_server_url}/public/{}", api_action.to_string())
    }
    #[derive(Debug, strum::EnumString, strum::Display, strum::IntoStaticStr)]
    #[strum(serialize_all = "snake_case")]
    pub enum PublicAPIAction {
        Login,
        ValidateSessionToken,
        CreateJoinToken,
        ValidateJoinToken,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct LoginRequest {
        pub username: String,
        pub password: String,
    }
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub enum LoginResponse {
        Success(LoginSuccess),
        Error(LoginError),
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct LoginSuccess {
        pub token: String,
        pub username: String,
        pub user_id: String,
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
    pub struct VerifySessionTokenRequest {
        pub token: String,
    }
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct VerifySessionTokenResponse {
        pub valid: bool,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct CreateJoinTokenRequest {
        pub session_token: String,
        pub server_nonce: String,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub enum CreateJoinTokenResponse {
        Success(CreateJoinTokenSuccess),
        Error(CreateJoinTokenError),
    }



    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct CreateJoinTokenSuccess {
        pub join_token: String,
    }
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct CreateJoinTokenError {
        pub kind: CreateJoinTokenErrorKind,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub enum CreateJoinTokenErrorKind {
        InvalidSessionToken,
    }


// ----------------------


    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct ValidateJoinTokenRequest {
        pub join_token: String,
        pub server_nonce: String,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub enum ValidateJoinTokenResponse {
        Valid(ValidateJoinTokenSuccess),
        Invalid,
    }
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct  ValidateJoinTokenSuccess {
        pub user_id: String
    }
}
