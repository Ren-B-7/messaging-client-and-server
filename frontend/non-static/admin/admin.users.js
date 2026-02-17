/**
 * Admin — Users & Stats
 * Fetches and renders the user table, search/filter, and the server
 * configuration info panels.
 * Depends on: Utils, AdminState, AdminUI
 */

const AdminUsers = {
  _loaded: false,

  // ── User list ──────────────────────────────────────────────────────────────

  async load(force = false) {
    if (this._loaded && !force) return;

    const tbody = document.getElementById('user-tbody');
    if (!tbody) return;
    tbody.innerHTML = AdminUI.spinnerRow(6);

    try {
      const data       = await this._get('/admin/api/users');
      AdminState.users = data.data?.users ?? [];
      this._loaded     = true;
      this.renderTable(AdminState.users);
    } catch (e) {
      tbody.innerHTML = `
        <tr><td colspan="6">
          <div class="empty-state">
            <span class="empty-icon">⚠</span>
            Failed to load users — check the console.
          </div>
        </td></tr>`;
      AdminUI.logAction('error', 'Failed to fetch user list from API');
    }
  },

  reload() { this.load(true); },

  renderTable(users) {
    const tbody = document.getElementById('user-tbody');
    if (!tbody) return;

    if (!users.length) {
      tbody.innerHTML = `
        <tr><td colspan="6">
          ${AdminUI.emptyHtml('⊞', 'No users found.')}
        </td></tr>`;
      return;
    }

    tbody.innerHTML = users.map(u => this._row(u)).join('');
  },

  _row(u) {
    const joined = u.created_at
      ? new Date(u.created_at * 1000).toLocaleDateString()
      : '—';
    const statusBadge = u.banned
      ? '<span class="badge badge-red">● Banned</span>'
      : '<span class="badge badge-green">● Active</span>';
    const primaryBtn = u.banned
      ? `<button class="btn btn-sm btn-green" onclick="AdminActions.quickUnban(${u.id})">Unban</button>`
      : `<button class="btn btn-sm btn-amber" onclick="AdminActions.quickBan(${u.id})">Ban</button>`;

    return `
      <tr class="${u.banned ? 'is-banned' : ''}">
        <td>
          <div class="user-cell">
            <div class="user-avatar">${Utils.getInitials(u.name || u.email || '?')}</div>
            <div>
              <div class="user-name">${Utils.escapeHtml(u.name || 'Unknown')}</div>
              <div class="user-id">#${u.id}</div>
            </div>
          </div>
        </td>
        <td>${u.id}</td>
        <td>${Utils.escapeHtml(u.email || '—')}</td>
        <td>${joined}</td>
        <td>${statusBadge}</td>
        <td>
          <div class="action-cell">
            ${primaryBtn}
            <button class="btn btn-sm btn-red"
                    onclick="AdminActions.quickDelete(${u.id})">Delete</button>
          </div>
        </td>
      </tr>`;
  },

  filter() {
    const q      = (document.getElementById('user-search')?.value || '').toLowerCase();
    const status = document.getElementById('status-filter')?.value || 'all';

    const filtered = AdminState.users.filter(u => {
      const matchQ = !q
        || String(u.id).includes(q)
        || (u.name  || '').toLowerCase().includes(q)
        || (u.email || '').toLowerCase().includes(q);
      const matchS = status === 'all'
        || (status === 'banned' && u.banned)
        || (status === 'active' && !u.banned);
      return matchQ && matchS;
    });

    this.renderTable(filtered);
  },

  // ── Stats ──────────────────────────────────────────────────────────────────

  async loadStats() {
    try {
      const data    = await this._get('/admin/api/stats');
      AdminState.stats = data.data || {};
      this._renderStatCards(AdminState.stats);
      this._renderServerInfo(AdminState.stats);
    } catch (e) {
      AdminUI.toast('Failed to load server stats', 'error');
      AdminUI.logAction('error', 'Failed to fetch /admin/api/stats');
    }
  },

  _renderStatCards(data) {
    const sv = data.server || {};
    this._setText('stat-total',    sv.total_users     ?? 42);
    this._setText('stat-sessions', sv.active_sessions ?? 7);
    this._setText('stat-banned',   sv.banned_users    ?? 2);
    this._setText('stat-maxconn',  sv.max_connections ?? '—');
  },

  _renderServerInfo(data) {
    const sv = data.server || {};
    const au = data.auth   || {};

    const serverEl = document.getElementById('server-info');
    if (serverEl) {
      serverEl.innerHTML = this._infoRows({
        'Bind address':    sv.bind            ?? '—',
        'Client port':     sv.port_client     ?? '—',
        'Admin port':      sv.port_admin      ?? '—',
        'Max connections': sv.max_connections ?? '—',
      });
    }

    const authEl = document.getElementById('auth-info');
    if (authEl) {
      authEl.innerHTML = this._infoRows({
        'Token expiry (min)': au.token_expiry_minutes ?? '—',
        'Email required':     String(au.email_required ?? '—'),
      });
    }
  },

  _infoRows(map) {
    return Object.entries(map).map(([k, v]) => `
      <div class="info-row">
        <span class="info-key">${Utils.escapeHtml(k)}</span>
        <span class="info-val">${Utils.escapeHtml(String(v))}</span>
      </div>`).join('');
  },

  // ── Helpers ────────────────────────────────────────────────────────────────

  _setText(id, value) {
    const el = document.getElementById(id);
    if (el) el.textContent = value;
  },

  async _get(url) {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.json();
  },
};
