#[derive(Debug, Clone)]
pub struct NewUser {
    pub username: String,
    pub password_hash: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: Option<String>,
    pub created_at: i64,
    pub is_banned: bool,
}

#[derive(Debug, Clone)]
pub struct BanInfo {
    pub user_id: i64,
    pub username: String,
    pub is_banned: bool,
    pub ban_reason: Option<String>,
    pub banned_at: Option<i64>,
    pub banned_by: Option<i64>,
}
