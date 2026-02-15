pub mod admin;
pub mod chats;
pub mod groups;
pub mod profile;

// Re-export for convenience
pub use chats::{handle_get_chats, handle_create_chat, handle_get_messages, handle_send_message};
pub use groups::{handle_get_groups, handle_create_group, handle_get_members, handle_add_member, handle_remove_member};
pub use profile::{handle_get_profile, handle_logout};
