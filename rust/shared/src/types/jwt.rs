use serde::{Deserialize, Serialize};

/// Claims embedded in every JWT issued by the server.
///
/// # Fast path (GET requests)
/// Decode the JWT and verify the HMAC signature — **zero DB reads**.
/// The claims carry enough information to identify and authorise the user.
///
/// # Secure path (POST / PUT / DELETE)
/// Decode the JWT, then:
///   1. Look up `session_id` in the `sessions` table to confirm the session
///      hasn't been explicitly revoked (logout / ban).
///   2. Compare `sessions.ip_address` against the current request IP.
///   3. Warn (but do not block) when the `user_agent` prefix has changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Standard JWT subject — set to the username.
    pub sub: String,

    /// Numeric user ID (matches `users.id`).
    pub user_id: i64,

    /// UUID stored in `sessions.session_id`.
    /// This is the revocation handle: deleting the row invalidates the JWT
    /// even before its `exp` is reached.
    pub session_id: String,

    /// Full user-agent string captured at login time.
    /// On secure requests the current UA *prefix* is compared against this
    /// to detect device changes (warn-only — UA strings can legitimately
    /// change on browser update).
    pub user_agent: String,

    /// Whether this user has admin privileges.
    /// Embedded so the admin fast-path requires no extra DB query on GET
    /// routes.  A newly-promoted/demoted user must log in again for this
    /// field to update.
    pub is_admin: bool,

    /// Standard JWT expiry (Unix timestamp, seconds).
    pub exp: usize,

    /// Issued-at (Unix timestamp, seconds).
    pub iat: usize,
}
