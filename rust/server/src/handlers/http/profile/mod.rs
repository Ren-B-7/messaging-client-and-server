pub mod get;
pub mod settings;
pub mod update;

// Re-export main handlers
#[allow(unused_imports)]
pub use get::handle_get_profile;

#[allow(unused_imports)]
pub use settings::{handle_change_password, handle_logout_all};

#[allow(unused_imports)]
pub use update::handle_update_profile;
