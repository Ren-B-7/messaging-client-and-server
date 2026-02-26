pub mod groups;
pub mod messages;

// Chat management (create, list)
pub use messages::{handle_create_chat, handle_get_chats};

// Message operations (send, receive, mark read)
pub use messages::{handle_get_messages, handle_mark_read, handle_send_message};

// Group management
pub use groups::{
    handle_add_member, handle_create_group, handle_get_groups, handle_get_members,
    handle_remove_member,
};
