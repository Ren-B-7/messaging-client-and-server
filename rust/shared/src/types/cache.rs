use serde::Deserialize;
use std::fmt;

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum CacheStrategy {
    /// Cache for one year.  Use for fingerprinted / immutable assets.
    LongTerm,
    /// Cache for one hour.  Use for assets that are stable but may be updated.
    ShortTerm,
    /// Do not cache at all (`no-store`).  Use for HTML pages and any response
    /// that varies per-user or per-session.
    NoCache,
}

impl fmt::Display for CacheStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheStrategy::LongTerm => write!(f, "LongTerm (1 year)"),
            CacheStrategy::ShortTerm => write!(f, "ShortTerm (1 hour)"),
            CacheStrategy::NoCache => write!(f, "NoCache (no-store)"),
        }
    }
}
