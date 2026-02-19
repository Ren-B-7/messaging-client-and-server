use serde::Deserialize;
use std::fmt;

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum CacheStrategy {
    Yes,      // Default (1 year)
    No,       // 1 hour cache
    Explicit, // No cache at all
}

impl fmt::Display for CacheStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheStrategy::Yes => write!(f, "Yes (1 year)"),
            CacheStrategy::No => write!(f, "No (1 hour)"),
            CacheStrategy::Explicit => write!(f, "Explicit (no-cache)"),
        }
    }
}
