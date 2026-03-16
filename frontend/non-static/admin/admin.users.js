/**
 * Admin — Users & Stats
 * Fetches and renders the user table, search/filter, server stats,
 * and live metrics from the Tower middleware.
 * Depends on: Utils, AdminState, AdminUI
 *
 * API field mapping (from handlers/http/admin/users.rs):
 *   id, username, email, created_at (unix secs), is_banned, ban_reason,
 * is_admin
 *
 * API field mapping (from handlers/http/admin/stats.rs):
 *   stats response: { server: { total_users, active_sessions, banned_users,
 *                               max_connections, bind, port_client, port_admin
 * }, auth:   { token_expiry_minutes, email_required } }
 *
 *   metrics response: { total_requests, active_connections, error_count,
 *                       bytes_sent, bytes_received, rate_limited, ip_blocked,
 *                       uptime_secs, requests_per_second, error_rate_pct,
 *                       latency_avg_ms, latency_p50_ms, latency_p95_ms,
 *                       latency_p99_ms,
 *                       rate_limiter: { total_ips, rate_limited, capacity,
 * refill_rate } }
 */

const AdminUsers = {
  _loaded : false,

  // ── User list ──────────────────────────────────────────────────────────────

  async loadUsers(force = false) {
    if (this._loaded && !force)
      return;

    const tbody = document.getElementById("user-tbody");
    if (!tbody)
      return;
    tbody.innerHTML = AdminUI.spinnerRow(6);

    try {
      const data = await this._get("/admin/api/users");
      AdminState.users = data.data?.users ?? [];
      this._loaded = true;
      this.renderTable(AdminState.users);
      AdminUI.logAction("info", `Loaded ${AdminState.users.length} users`);
    } catch (e) {
      tbody.innerHTML = `
        <tr><td colspan="6">
          <div class="empty-state">
            <span class="empty-icon">⚠</span>
            Failed to load users — check the console.
          </div>
        </td></tr>`;
      AdminUI.logAction("error", "Failed to fetch user list from API");
      console.error("[admin] load users:", e);
    }
  },

  reload() { this.loadUsers(true); },

  renderTable(users) {
    const tbody = document.getElementById("user-tbody");
    if (!tbody)
      return;

    if (!users.length) {
      tbody.innerHTML = `
        <tr><td colspan="6">
          ${AdminUI.emptyHtml("⊞", "No users found.")}
        </td></tr>`;
      return;
    }

    tbody.innerHTML = users.map((u) => this._row(u)).join("");
  },

  _row(u) {
    // API returns is_banned (bool), username (string), created_at (unix secs)
    const joined =
        u.created_at ? new Date(u.created_at * 1000).toLocaleDateString() : "—";

    const statusBadge = u.is_banned
                            ? '<span class="badge badge-red">● Banned</span>'
                            : '<span class="badge badge-green">● Active</span>';

    const adminBadge =
        u.is_admin ? '<span class="badge badge-amber" title="Admin">★</span>'
                   : "";

    const primaryBtn =
        u.is_banned
            ? `<button class="btn btn-sm btn-green" onclick="AdminActions.quickUnban(${
                  u.id})">Unban</button>`
            : `<button class="btn btn-sm btn-amber" onclick="AdminActions.quickBan(${
                  u.id})">Ban</button>`;

    const promoteBtn =
        u.is_admin
            ? `<button class="btn btn-sm btn-secondary" onclick="AdminActions.submitDemote(${
                  u.id})" title="Remove admin">Demote</button>`
            : `<button class="btn btn-sm btn-secondary" onclick="AdminActions.submitPromote(${
                  u.id})" title="Make admin">Promote</button>`;

    return `
      <tr class="${u.is_banned ? "is-banned" : ""}">
        <td>
          <div class="user-cell">
            <div class="user-avatar">${
        Utils.getInitials(u.username || "?")}</div>
            <div>
              <div class="user-name">${
        Utils.escapeHtml(u.username || "Unknown")} ${adminBadge}</div>
              <div class="user-id">#${u.id}</div>
            </div>
          </div>
        </td>
        <td>${u.id}</td>
        <td>${Utils.escapeHtml(u.email || "—")}</td>
        <td>${joined}</td>
        <td>${statusBadge}</td>
        <td>
          <div class="action-cell">
            ${primaryBtn}
            ${promoteBtn}
            <button class="btn btn-sm btn-red"
                    onclick="AdminActions.quickDelete(${u.id})">Delete</button>
          </div>
        </td>
      </tr>`;
  },

  filter() {
    const q =
        (document.getElementById("user-search")?.value || "").toLowerCase();
    const status = document.getElementById("status-filter")?.value || "all";

    const filtered = AdminState.users.filter((u) => {
      const matchQ = !q || String(u.id).includes(q) ||
                     (u.username || "").toLowerCase().includes(q) ||
                     (u.email || "").toLowerCase().includes(q);
      const matchS = status === "all" || (status === "banned" && u.is_banned) ||
                     (status === "active" && !u.is_banned);
      return matchQ && matchS;
    });

    this.renderTable(filtered);
  },

  // ── Stats (/admin/api/stats) ───────────────────────────────────────────────

  async loadStats() {
    try {
      const data = await this._get("/admin/api/stats");
      AdminState.stats = data;
      this._renderStatCards(data);
      this._renderServerInfo(data);
    } catch (e) {
      AdminUI.toast("Failed to load server stats", "error");
      AdminUI.logAction("error", "Failed to fetch /admin/api/stats");
      console.error("[admin] loadStats:", e);
    }
  },

  _renderStatCards(data) {
    // Stats handler returns the object directly (not nested under data.data)
    const db = data.database || {};
    this._setText("stat-total", db.total_users ?? "—");
    this._setText("stat-sessions", db.active_sessions ?? "—");
    this._setText("stat-banned", db.banned_users ?? "—");
    // max_connections comes from the server config block
    this._setText("stat-maxconn", data.server?.max_connections ?? "—");
  },

  _renderServerInfo(data) {
    const sv = data.server || {};
    const au = data.auth || {};
    const db = data.database || {};

    const serverEl = document.getElementById("server-info");
    if (serverEl) {
      serverEl.innerHTML = this._infoRows({
        "Bind address" : sv.bind ?? "—",
        "Client port" : sv.port_client ?? "—",
        "Admin port" : sv.port_admin ?? "—",
        "Max connections" : sv.max_connections ?? "—",
      });
    }

    const authEl = document.getElementById("auth-info");
    if (authEl) {
      authEl.innerHTML = this._infoRows({
        "Token expiry (min)" : au.token_expiry_minutes ?? "—",
        "Email required" : String(au.email_required ?? "—"),
      });
    }

    const dbEl = document.getElementById("db-info");
    if (dbEl) {
      dbEl.innerHTML = this._infoRows({
        "Total users" : db.total_users ?? "—",
        "Active sessions" : db.active_sessions ?? "—",
        "Banned users" : db.banned_users ?? "—",
        "Total messages" : db.total_messages ?? "—",
        "Total groups" : db.total_groups ?? "—",
        "Database path" : db.path ?? "—",
      });
    }
  },

  // ── Metrics (/admin/api/metrics) ───────────────────────────────────────────

  async loadMetrics() {
    try {
      const data = await this._get("/admin/api/metrics");
      const m = data.data || {};
      this._renderMetrics(m);
    } catch (e) {
      AdminUI.toast("Failed to load metrics", "error");
      AdminUI.logAction("error", "Failed to fetch /admin/api/metrics");
      console.error("[admin] loadMetrics:", e);
    }
  },

  _renderMetrics(m) {
    const rl = m.rate_limiter || {};

    const metricsEl = document.getElementById("metrics-info");
    if (metricsEl) {
      metricsEl.innerHTML = this._infoRows({
        "Total requests" : m.total_requests ?? "—",
        "Active connections" : m.active_connections ?? "—",
        "Error count" : m.error_count ?? "—",
        "Error rate" :
            m.error_rate_pct != null ? `${m.error_rate_pct.toFixed(2)}%` : "—",
        "Requests / sec" : m.requests_per_second != null
                               ? m.requests_per_second.toFixed(2)
                               : "—",
        Uptime : m.uptime_secs != null ? `${Math.floor(m.uptime_secs)}s` : "—",
        "Bytes sent" : m.bytes_sent != null ? Utils.formatFileSize(m.bytes_sent)
                                            : "—",
        "Bytes received" : m.bytes_received != null
                               ? Utils.formatFileSize(m.bytes_received)
                               : "—",
        "Rate limited" : m.rate_limited ?? "—",
        "IP blocked" : m.ip_blocked ?? "—",
      });
    }

    const latencyEl = document.getElementById("latency-info");
    if (latencyEl) {
      latencyEl.innerHTML = this._infoRows({
        "Avg latency" : m.latency_avg_ms != null
                            ? `${m.latency_avg_ms.toFixed(2)} ms`
                            : "—",
        "P50 latency" : m.latency_p50_ms != null
                            ? `${m.latency_p50_ms.toFixed(2)} ms`
                            : "—",
        "P95 latency" : m.latency_p95_ms != null
                            ? `${m.latency_p95_ms.toFixed(2)} ms`
                            : "—",
        "P99 latency" : m.latency_p99_ms != null
                            ? `${m.latency_p99_ms.toFixed(2)} ms`
                            : "—",
      });
    }

    const rlEl = document.getElementById("ratelimiter-info");
    if (rlEl) {
      rlEl.innerHTML = this._infoRows({
        "Tracked IPs" : rl.total_ips ?? "—",
        "Rate limited" : rl.rate_limited ?? "—",
        "Bucket size" : rl.capacity ?? "—",
        "Refill rate" : rl.refill_rate != null ? `${rl.refill_rate} req/s`
                                               : "—",
      });
    }
  },

  async loadSessions() {
    const tbody = document.getElementById("session-tbody");
    if (!tbody)
      return;
    tbody.innerHTML = AdminUI.spinnerRow(5);

    try {
      const data = await this._get("/admin/api/sessions");
      AdminState.sessions = data.data?.sessions ?? [];
      this._renderSessionsTable(AdminState.sessions);
      this._setText("stat-sessions", AdminState.sessions.length);
    } catch (e) {
      tbody.innerHTML = `<tr><td colspan="5">
      <div class="empty-state"><span class="empty-icon">⚠</span>Failed to load sessions.</div>
    </td></tr>`;
      console.error("[admin] loadSessions:", e);
    }
  },

  _renderSessionsTable(sessions) {
    const tbody = document.getElementById("session-tbody");
    if (!tbody)
      return;

    if (!sessions.length) {
      tbody.innerHTML = `<tr><td colspan="5">${
          AdminUI.emptyHtml("🔑", "No sessions.")}</td></tr>`;
      return;
    }

    const nowSecs = Date.now() / 1000;

    tbody.innerHTML =
        sessions
            .map((s) => {
              const expired = s.expires_at <= nowSecs;
              const created = new Date(s.created_at * 1000).toLocaleString();
              const expires = new Date(s.expires_at * 1000).toLocaleString();
              const expiresCell =
                  expired
                      ? `<span class="session-expired">${
                            expires} <span class="badge badge-red">Expired</span></span>`
                      : expires;
              return `
      <tr class="${expired ? "is-expired" : ""}">
        <td><span class="user-name">${Utils.escapeHtml(s.username)}</span>
            <div class="user-id">#${s.user_id}</div></td>
        <td><code>${Utils.escapeHtml(String(s.id).slice(0, 8))}…</code></td>
        <td>${Utils.escapeHtml(s.ip_address || "—")}</td>
        <td>${created}</td>
        <td>${expiresCell}</td>
      </tr>`;
            })
            .join("");
  },
  // ── Helpers ────────────────────────────────────────────────────────────────

  _infoRows(map) {
    return Object.entries(map)
        .map(
            ([ k, v ]) => `
      <div class="info-row">
        <span class="info-key">${Utils.escapeHtml(k)}</span>
        <span class="info-val">${Utils.escapeHtml(String(v))}</span>
      </div>`,
            )
        .join("");
  },

  _setText(id, value) {
    const el = document.getElementById(id);
    if (el)
      el.textContent = value;
  },

  async _get(url) {
    const res = await fetch(url);
    if (!res.ok)
      throw new Error(`HTTP ${res.status}`);
    return res.json();
  },
};
