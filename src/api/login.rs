use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct LoginCredentials<'a> {
    pub login: &'a str,
    pub password: &'a str,
    pub remember_me: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LoginResponseUser {
    pub email: String,
    pub username: String,
    pub external_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LoginResponse {
    pub user: LoginResponseUser,
    pub session_token: String,
    pub remember_token: Option<String>,
}
