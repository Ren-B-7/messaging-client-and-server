#[derive(Debug, Clone)]
pub struct Group {
    pub id: i64,
    pub name: String,
    pub created_by: i64,
    pub created_at: i64,
    pub description: Option<String>,
    pub chat_type: String,
}

#[derive(Debug, Clone)]
pub struct GroupMember {
    pub id: i64,
    pub chat_id: i64,
    pub user_id: i64,
    pub joined_at: i64,
    pub role: String,
}

#[derive(Debug, Clone)]
pub struct NewGroup {
    pub name: String,
    pub created_by: i64,
    pub description: Option<String>,
    /// Either "direct" or "group".
    pub chat_type: String,
}
