/**
 * Presence — client-side heartbeat
 *
 * Drop this block into chat_init.js (and any other authenticated page)
 * inside the DOMContentLoaded handler, AFTER the profile fetch succeeds.
 *
 * Calls POST /api/presence every 60 s while the tab is open.
 * Fires POST /api/presence/offline via sendBeacon on tab close.
 */

// ── Heartbeat ──────────────────────────────────────────────────────────────

/** Send a single heartbeat and swallow any errors (fire-and-forget). */
function sendHeartbeat() {
  fetch('/api/presence', { method: 'POST' }).catch(() => {});
}

// Kick one off immediately so the user shows as online right away,
// then repeat every 60 seconds.
sendHeartbeat();
const _presenceTimer = setInterval(sendHeartbeat, 60_000);

// ── Offline signal ─────────────────────────────────────────────────────────

// sendBeacon is guaranteed to complete even as the page unloads —
// unlike fetch(), which would be cancelled by the browser.
window.addEventListener('beforeunload', () => {
  navigator.sendBeacon('/api/presence/offline');
});

// ── Visibility change (optional but recommended) ───────────────────────────
//
// When the user switches tabs or minimises the browser the tab becomes
// hidden.  We don't pause the heartbeat — the 2-minute timeout on the server
// is generous enough that a brief visibility loss won't flip the user offline.
// If you want tighter presence accuracy you can stop/restart the interval:
//
// document.addEventListener('visibilitychange', () => {
//   if (document.hidden) {
//     clearInterval(_presenceTimer);
//   } else {
//     sendHeartbeat();
//     _presenceTimer = setInterval(sendHeartbeat, 60_000);
//   }
// });
