pub mod admin;
pub mod stats;
pub mod users;

// User management handlers
#[allow(unused_imports)]
pub use admin::{
    handle_ban_user, handle_delete_user, handle_demote_user, handle_get_users, handle_promote_user,
    handle_unban_user,
};

// Stats and metrics handlers
#[allow(unused_imports)]
pub use stats::{handle_metrics, handle_server_config};
