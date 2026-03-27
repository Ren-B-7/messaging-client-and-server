use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Clone)]
pub struct NewUser {
    pub username: String,
    pub password_hash: String,
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<NameSurname>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: Option<String>,
    pub created_at: i64,
    pub is_banned: bool,
    #[serde(default)]
    pub name: Option<NameSurname>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NameSurname {
    #[serde(default = "default_first_name")]
    pub first_name: Option<String>,
    #[serde(default = "default_last_name")]
    pub last_name: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BanInfo {
    pub user_id: i64,
    pub username: String,
    pub is_banned: bool,
    pub ban_reason: Option<String>,
    pub banned_at: Option<i64>,
    pub banned_by: Option<i64>,
}

pub fn default_first_name() -> Option<String> {
    Some("".to_string())
}

pub fn default_last_name() -> Option<String> {
    Some("".to_string())
}

impl Default for NameSurname {
    fn default() -> Self {
        Self {
            first_name: Some("".to_string()),
            last_name: Some("".to_string()),
        }
    }
}
