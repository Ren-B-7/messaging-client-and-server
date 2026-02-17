/**
 * Admin — State
 * Single source of truth for all admin panel data held this session.
 * Depends on: (none)
 */

const AdminState = {
  /** All users returned from /admin/api/users */
  users: [],

  /** Activity log entries (newest first), capped at 200 */
  log: [],

  /** Last fetched stats from /admin/api/stats */
  stats: null,

  /** Currently active tab name */
  activeTab: 'dashboard',

  // ── Log ──────────────────────────────────────────────────────────────────

  addLog(type, message) {
    this.log.unshift({ type, message, time: new Date() });
    if (this.log.length > 200) this.log.pop();
  },

  clearLog() {
    this.log = [];
  },

  // ── Users ─────────────────────────────────────────────────────────────────

  setUserBanned(id, banned) {
    const u = this.users.find(u => u.id === id);
    if (u) u.banned = banned;
  },

  removeUser(id) {
    this.users = this.users.filter(u => u.id !== id);
  },
};
