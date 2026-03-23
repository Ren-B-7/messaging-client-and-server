pub mod admin_login;
pub mod login;
pub mod register;
pub mod reset;

// Re-export main handlers
#[allow(unused_imports)]
pub use login::{handle_login, handle_login_api};

#[allow(unused_imports)]
pub use register::{handle_register, handle_register_api};

// Admin login — used by the admin server's router
#[allow(unused_imports)]
pub use admin_login::{
    handle_login as handle_admin_login, handle_login_api as handle_admin_login_api,
};

// Password reset — open endpoints (no auth required, token is the credential)
#[allow(unused_imports)]
pub use reset::{handle_reset_confirm, handle_reset_request};
