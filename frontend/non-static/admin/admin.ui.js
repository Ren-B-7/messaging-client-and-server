/**
 * Admin — UI
 * Clock, tab switching, toast notifications, activity log rendering,
 * and shared empty-state / loading helpers.
 * Depends on: Utils, AdminState
 */

const AdminUI = {
  // ── Clock ──────────────────────────────────────────────────────────────────

  startClock() {
    const el = document.getElementById("clock");
    if (!el) return;
    const tick = () => {
      el.textContent = new Date().toLocaleTimeString("en-GB");
    };
    tick();
    setInterval(tick, 1000);
  },

  // ── Tab switching ──────────────────────────────────────────────────────────

  switchTab(name, btn) {
    document
      .querySelectorAll(".tab-section")
      .forEach((s) => s.classList.remove("active"));
    document
      .querySelectorAll(".nav-btn")
      .forEach((b) => b.classList.remove("active"));

    const section = document.getElementById(`tab-${name}`);
    if (section) section.classList.add("active");
    if (btn) btn.classList.add("active");

    AdminState.activeTab = name;

    if (name === "users") AdminUsers.loadUsers();
    if (name === "sessions") AdminUsers.loadSessions();
    if (name === "server") AdminUsers.loadStats();
    if (name === "metrics") AdminUsers.loadMetrics();
    if (name === "log") this.renderFullLog();
  },

  // ── Toast ──────────────────────────────────────────────────────────────────

  toast(message, type = "info") {
    const icons = { success: "✓", error: "✕", warn: "⚠", info: "ℹ" };
    const area = document.getElementById("toast-area");
    if (!area) return;

    const el = document.createElement("div");
    el.className = `toast ${type}`;
    el.innerHTML = `
      <span class="toast-icon">${icons[type] || "ℹ"}</span>
      <span class="toast-msg">${Utils.escapeHtml(message)}</span>`;
    area.appendChild(el);
    setTimeout(() => el.remove(), 3500);
  },

  // ── Log ────────────────────────────────────────────────────────────────────

  logAction(type, message) {
    AdminState.addLog(type, message);
    this.renderRecentLog();
    if (AdminState.activeTab === "log") this.renderFullLog();
  },

  renderRecentLog() {
    const el = document.getElementById("recent-log");
    if (!el) return;
    el.innerHTML = AdminState.log.length
      ? AdminState.log
          .slice(0, 8)
          .map((e) => this._logEntry(e))
          .join("")
      : this.emptyHtml("📋", "No activity yet.");
  },

  renderFullLog() {
    const el = document.getElementById("full-log");
    if (!el) return;
    el.innerHTML = AdminState.log.length
      ? AdminState.log.map((e) => this._logEntry(e)).join("")
      : this.emptyHtml("📋", "No activity recorded yet.");
  },

  _logEntry({ type, message, time }) {
    const icons = { success: "✓", error: "✕", warn: "⚠", info: "ℹ" };
    const badges = {
      success: "badge badge-green",
      error: "badge badge-red",
      warn: "badge badge-amber",
      info: "badge badge-muted",
    };
    return `
      <div class="log-entry">
        <span class="log-time">${time.toLocaleTimeString("en-GB")}</span>
        <span class="log-msg">${message}</span>
        <span class="${badges[type] || "badge badge-muted"} log-type">${icons[type] || "ℹ"}</span>
      </div>`;
  },

  clearLog() {
    AdminState.clearLog();
    this.renderRecentLog();
    this.renderFullLog();
  },

  // ── Shared markup ──────────────────────────────────────────────────────────

  emptyHtml(icon, text) {
    return `
      <div class="empty-state">
        <span class="empty-icon">${icon}</span>
        ${Utils.escapeHtml(text)}
      </div>`;
  },

  spinnerRow(colspan = 6) {
    return `
      <tr><td colspan="${colspan}">
        <div class="empty-state"><span class="spinner"></span></div>
      </td></tr>`;
  },

  // ── Button loading state ───────────────────────────────────────────────────

  setLoading(btnId, loading) {
    const btn = document.getElementById(btnId);
    if (!btn) return;
    btn.disabled = loading;
    if (loading) {
      btn._html = btn.innerHTML;
      btn.innerHTML = '<span class="spinner"></span> Working…';
    } else if (btn._html) {
      btn.innerHTML = btn._html;
      delete btn._html;
    }
  },
};
