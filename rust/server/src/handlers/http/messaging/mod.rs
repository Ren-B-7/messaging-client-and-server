pub mod chats;
pub mod groups;
pub mod messages;

// Re-export from messages
#[allow(unused_imports)]
pub use messages::{handle_get_messages, handle_mark_read, handle_send_message};

// Re-export from chats
#[allow(unused_imports)]
pub use chats::{
    handle_create_chat, handle_get_chats, handle_get_messages as handle_get_chat_messages,
    handle_send_message as handle_send_chat_message,
};

// Re-export from groups
#[allow(unused_imports)]
pub use groups::{
    handle_add_member, handle_create_group, handle_get_groups, handle_get_members,
    handle_remove_member,
};
