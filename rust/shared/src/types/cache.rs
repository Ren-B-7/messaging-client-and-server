use serde::Deserialize;
use std::fmt;

/// Controls the `Cache-Control` header added to static file responses.
///
/// # Variant semantics
///
/// | Variant     | Header set                                    | Use for                          |
/// |-------------|-----------------------------------------------|----------------------------------|
/// | `LongTerm`  | `public, max-age=31536000` (1 year)           | Hashed/immutable assets, icons   |
/// | `ShortTerm` | `public, max-age=3600` (1 hour)              | Assets that change occasionally  |
/// | `NoCache`   | `no-cache, no-store, must-revalidate`         | HTML pages, authenticated routes |
///
/// # Previous names (renamed for clarity)
///
/// The previous names `Yes`, `No`, and `Explicit` were confusing because
/// `No` did NOT mean "no caching" — it meant "one-hour caching".  The old
/// `Explicit` variant was the actual "no caching" option.  These names
/// have been replaced with the unambiguous names above.
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
