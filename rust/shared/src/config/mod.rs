pub mod config_loader;

pub use self::config_loader::load_config;

use std::sync::Arc;
use tokio::sync::{RwLock, watch};

use crate::types::server_config::AppConfig;

/// A cheaply-cloneable, live config handle.
///
/// All clones share the same underlying `RwLock<AppConfig>`, so a call to
/// [`reload`] is immediately visible to every part of the application that
/// holds a clone — including spawned tasks and per-connection handlers.
///
/// Added in this PR: a `tokio::sync::watch` channel that broadcasts the entire
/// `AppConfig` whenever [`reload`] is called, allowing long-lived tasks
/// (like the HTTP server listeners) to react to address/port changes without
/// polling.
#[derive(Clone, Debug)]
pub struct LiveConfig {
    inner: Arc<RwLock<AppConfig>>,
    tx: Arc<watch::Sender<AppConfig>>,
}

impl LiveConfig {
    /// Wrap an `AppConfig` in a new `LiveConfig`.
    pub fn new(config: AppConfig) -> Self {
        let (tx, _) = watch::channel(config.clone());
        Self {
            inner: Arc::new(RwLock::new(config)),
            tx: Arc::new(tx),
        }
    }

    /// Acquire a read guard. Keep it short-lived; never hold across `.await`.
    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, AppConfig> {
        self.inner.read().await
    }

    /// Atomically swap in a new config and broadcast it to all subscribers.
    pub async fn reload(&self, new: AppConfig) {
        *self.inner.write().await = new.clone();
        let _ = self.tx.send(new);
    }

    /// Subscribe to config changes. The returned receiver will receive a
    /// notification immediately with the current config, then whenever
    /// [`reload`] is called.
    pub fn subscribe(&self) -> watch::Receiver<AppConfig> {
        self.tx.subscribe()
    }
}
