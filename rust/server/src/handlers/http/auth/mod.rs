pub mod admin_login;
pub mod login;
pub mod logout;
pub mod register;

// Re-export main handlers
#[allow(unused_imports)]
pub use login::handle_login;

#[allow(unused_imports)]
pub use logout::handle_logout;

#[allow(unused_imports)]
pub use register::handle_register;

// Admin login — used by the admin server's router
#[allow(unused_imports)]
pub use admin_login::handle_login as handle_admin_login;
