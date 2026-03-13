#[derive(Debug, Clone)]
pub struct PasswordResetToken {
    pub id: i64,
    pub user_id: i64,
    pub token: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub used: bool,
}
