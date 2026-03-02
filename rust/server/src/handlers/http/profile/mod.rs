pub mod profile;

// Re-export main handlers
#[allow(unused_imports)]
pub use profile::{
    handle_change_password, handle_get_profile, handle_logout, handle_logout_all,
    handle_update_profile,
};
