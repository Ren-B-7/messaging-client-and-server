#[allow(dead_code)]
pub mod groups;
#[allow(dead_code)]
pub mod messages;

// Chat management (create, list)
#[allow(unused_imports)]
pub use messages::{handle_create_chat, handle_get_chats};

// Message operations (send, receive, mark read)
#[allow(unused_imports)]
pub use messages::{handle_get_messages, handle_mark_read, handle_send_message};

// Group management
#[allow(unused_imports)]
pub use groups::{
    handle_add_member, handle_create_group, handle_get_groups, handle_get_members,
    handle_remove_member,
};
