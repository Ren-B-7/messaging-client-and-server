pub mod config;

pub use self::config::load_config;

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::types::server_config::AppConfig;

/// A cheaply-cloneable, live config handle.
///
/// All clones share the same underlying `RwLock<AppConfig>`, so a call to
/// [`reload`] is immediately visible to every part of the application that
/// holds a clone — including spawned tasks and per-connection handlers.
///
/// # Usage
/// ```rust,no_run
/// // Read (short-lived guard — do not hold across .await points)
/// // let cfg = state.config.read().await;
/// // let expiry = cfg.auth.token_expiry_secs();
///
/// // If you need a value across an await, copy it out first
/// // let max_conn = state.config.read().await.server.max_connections;
/// // do_something_async().await;
///
/// // Hot-reload from an admin endpoint or SIGHUP handler
/// // state.config.reload(new_app_config).await;
/// ```
#[derive(Clone, Debug)]
pub struct LiveConfig(Arc<RwLock<AppConfig>>);

impl LiveConfig {
    /// Wrap an `AppConfig` in a new `LiveConfig`.
    pub fn new(config: AppConfig) -> Self {
        Self(Arc::new(RwLock::new(config)))
    }

    /// Acquire a read guard. Keep it short-lived; never hold across `.await`.
    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, AppConfig> {
        self.0.read().await
    }

    /// Atomically swap in a new config. All existing clones see the new
    /// values on their next `.read()` call.
    pub async fn reload(&self, new: AppConfig) {
        *self.0.write().await = new;
    }
}
