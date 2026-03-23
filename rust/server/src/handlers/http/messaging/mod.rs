pub mod files;
pub mod groups;
pub mod messages;

// Chat management (create, list)
pub use messages::{handle_create_chat, handle_get_chats};

// Message operations
pub use messages::{
    // New in this PR:
    handle_delete_message,
    handle_get_messages,
    handle_get_unread,
    handle_mark_read,
    handle_send_message,
    handle_typing,
};

// Group management
pub use groups::{
    handle_add_member, handle_create_group, handle_delete_group, handle_get_groups,
    handle_get_members, handle_remove_member, handle_rename_group, handle_search_users,
};
