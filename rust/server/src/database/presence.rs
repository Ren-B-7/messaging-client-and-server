use std::time::{SystemTime, UNIX_EPOCH};

use tokio_rusqlite::{Connection, Result, params, rusqlite};

/// How long without a heartbeat before a user is considered offline (120 s).
pub const PRESENCE_TIMEOUT_SECS: i64 = 120;

// ── DB helpers ────────────────────────────────────────────────────────────────

/// Upsert a heartbeat for `user_id`.
///
/// Sets `last_seen = NOW()` and `is_online = 1`.
/// Called by `POST /api/presence` and implicitly on login.
pub async fn touch_presence(conn: &Connection, user_id: i64) -> Result<()> {
    let now = now_secs();

    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "INSERT INTO user_presence (user_id, last_seen, is_online)
             VALUES (?1, ?2, 1)
             ON CONFLICT(user_id) DO UPDATE
                 SET last_seen = excluded.last_seen,
                     is_online  = 1",
            params![user_id, now],
        )?;
        Ok(())
    })
    .await
}

/// Explicitly mark a user offline.
///
/// Called by `POST /api/presence/offline` (sendBeacon on tab close) and on
/// logout so other users see the change immediately rather than waiting for
/// the 2-minute timeout to lapse.
pub async fn set_offline(conn: &Connection, user_id: i64) -> Result<()> {
    let now = now_secs();

    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "INSERT INTO user_presence (user_id, last_seen, is_online)
             VALUES (?1, ?2, 0)
             ON CONFLICT(user_id) DO UPDATE
                 SET last_seen = excluded.last_seen,
                     is_online  = 0",
            params![user_id, now],
        )?;
        Ok(())
    })
    .await
}

/// Check whether a single user is currently online.
///
/// A user is online if their row exists, `is_online = 1`, AND
/// `last_seen` is within `PRESENCE_TIMEOUT_SECS` of now.
/// The two-condition check means a crashed tab that never sent the offline
/// beacon will still time out correctly.
pub async fn is_online(conn: &Connection, user_id: i64) -> Result<bool> {
    let cutoff = now_secs() - PRESENCE_TIMEOUT_SECS;

    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT COUNT(*) FROM user_presence
             WHERE user_id  = ?1
               AND is_online = 1
               AND last_seen >= ?2",
        )?;
        let count: i64 = stmt.query_row(params![user_id, cutoff], |r| r.get(0))?;
        Ok(count > 0)
    })
    .await
}

/// Return `(user_id, is_online)` for every user in a given chat.
///
/// Useful when building the chat list response so the caller can attach an
/// `is_online` flag to each DM entry in a single query.
pub async fn get_presence_for_chat(
    conn: &Connection,
    chat_id: i64,
) -> Result<Vec<(i64, bool)>> {
    let cutoff = now_secs() - PRESENCE_TIMEOUT_SECS;

    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT gm.user_id,
                    CASE
                        WHEN up.is_online = 1 AND up.last_seen >= ?2 THEN 1
                        ELSE 0
                    END AS online
             FROM group_members gm
             LEFT JOIN user_presence up ON up.user_id = gm.user_id
             WHERE gm.chat_id = ?1",
        )?;

        let rows = stmt
            .query_map(params![chat_id, cutoff], |row| {
                let uid: i64 = row.get(0)?;
                let online: i64 = row.get(1)?;
                Ok((uid, online != 0))
            })?
            .collect::<std::result::Result<Vec<_>, rusqlite::Error>>()?;

        Ok(rows)
    })
    .await
}

/// Sweep rows that have timed out — sets `is_online = 0` for any row whose
/// `last_seen` is older than `PRESENCE_TIMEOUT_SECS`.
///
/// Call this from a periodic background task (e.g. every 60 s) so the DB
/// stays tidy and queries don't have to re-evaluate the cutoff every time.
pub async fn cleanup_stale_presence(conn: &Connection) -> Result<usize> {
    let cutoff = now_secs() - PRESENCE_TIMEOUT_SECS;

    conn.call(move |conn: &mut rusqlite::Connection| {
        let count = conn.execute(
            "UPDATE user_presence SET is_online = 0
             WHERE is_online = 1 AND last_seen < ?1",
            params![cutoff],
        )?;
        Ok(count)
    })
    .await
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
