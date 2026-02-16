use ipnet::IpNet;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct IpFilter {
    inner: Arc<IpFilterInner>,
}

#[derive(Debug)]
struct IpFilterInner {
    allowed: RwLock<Vec<IpNet>>,
    blocked: RwLock<Vec<IpNet>>,
}

impl IpFilter {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(IpFilterInner {
                allowed: RwLock::new(Vec::<IpNet>::new()),
                blocked: RwLock::new(Vec::<IpNet>::new()),
            }),
        }
    }

    pub async fn allow_network(&self, network: &str) {
        if let Ok(net) = network.parse::<IpNet>() {
            let mut allowed: tokio::sync::RwLockWriteGuard<'_, Vec<IpNet>> =
                self.inner.allowed.write().await;
            allowed.push(net);
        }
    }

    pub async fn block_network(&self, network: &str) {
        if let Ok(net) = network.parse::<IpNet>() {
            let mut blocked: tokio::sync::RwLockWriteGuard<'_, Vec<IpNet>> =
                self.inner.blocked.write().await;
            blocked.push(net);
        }
    }

    pub async fn is_allowed(&self, ip: IpAddr) -> bool {
        let blocked: tokio::sync::RwLockReadGuard<'_, Vec<IpNet>> = self.inner.blocked.read().await;

        for blocked_net in blocked.iter() {
            if blocked_net.contains(&ip) {
                return false;
            }
        }

        let allowed: tokio::sync::RwLockReadGuard<'_, Vec<IpNet>> = self.inner.allowed.read().await;

        if !allowed.is_empty() {
            return allowed.iter().any(|net: &IpNet| net.contains(&ip));
        }

        true
    }

    pub async fn stats(&self) -> (usize, usize) {
        let allowed = self.inner.allowed.read().await;
        let blocked = self.inner.blocked.read().await;
        (allowed.len(), blocked.len())
    }
}

impl Default for IpFilter {
    fn default() -> Self {
        Self::new()
    }
}
