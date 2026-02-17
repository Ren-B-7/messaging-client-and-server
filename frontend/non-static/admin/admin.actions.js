/**
 * Admin — Actions
 * Modal open/close, form submit handlers for ban, unban, and delete,
 * and the quick-action helpers called from table-row inline buttons.
 * Depends on: Utils, AdminState, AdminUI, AdminUsers
 */

const AdminActions = {
  // ── Modal helpers ──────────────────────────────────────────────────────────

  openModal(id) {
    document.getElementById(id)?.classList.add('open');
  },

  closeModal(id) {
    document.getElementById(id)?.classList.remove('open');
  },

  openBanModal() {
    document.getElementById('ban-user-id').value = '';
    document.getElementById('ban-reason').value  = '';
    this.openModal('ban-modal');
    document.getElementById('ban-user-id').focus();
  },

  openUnbanModal() {
    document.getElementById('unban-user-id').value = '';
    this.openModal('unban-modal');
    document.getElementById('unban-user-id').focus();
  },

  openDeleteModal() {
    document.getElementById('delete-user-id').value      = '';
    document.getElementById('delete-confirm-text').value = '';
    document.getElementById('delete-submit-btn').disabled = true;
    this.openModal('delete-modal');
    document.getElementById('delete-user-id').focus();
  },

  checkDeleteConfirm() {
    const val = document.getElementById('delete-confirm-text')?.value || '';
    const btn = document.getElementById('delete-submit-btn');
    if (btn) btn.disabled = val !== 'DELETE';
  },

  // ── Quick-action (table row buttons) ──────────────────────────────────────

  quickBan(id) {
    document.getElementById('ban-user-id').value = id;
    this.openBanModal();
  },

  quickUnban(id) {
    document.getElementById('unban-user-id').value = id;
    this.openUnbanModal();
  },

  quickDelete(id) {
    document.getElementById('delete-user-id').value = id;
    this.openDeleteModal();
  },

  // ── Submit: ban ────────────────────────────────────────────────────────────

  async submitBan() {
    const userId = document.getElementById('ban-user-id')?.value.trim();
    const reason = document.getElementById('ban-reason')?.value.trim();

    if (!userId) { AdminUI.toast('User ID is required', 'warn'); return; }

    AdminUI.setLoading('ban-submit-btn', true);

    try {
      const res = await this._post('/admin/api/users/ban', { user_id: userId, reason });

      if (res.status === 'success') {
        AdminUI.toast(`User #${userId} has been banned`, 'success');
        AdminUI.logAction('warn',
          `Banned user <strong>#${userId}</strong>${reason ? ` — ${reason}` : ''}`);
        this.closeModal('ban-modal');
        AdminState.setUserBanned(Number(userId), true);
        AdminUsers.filter();
      } else {
        AdminUI.toast(res.message || 'Ban failed', 'error');
        AdminUI.logAction('error', `Ban failed for #${userId}: ${res.message || 'unknown'}`);
      }
    } catch (e) {
      AdminUI.toast('Request failed — see console', 'error');
      AdminUI.logAction('error', `Ban request error for #${userId}`);
      console.error('[admin] ban:', e);
    }

    AdminUI.setLoading('ban-submit-btn', false);
  },

  // ── Submit: unban ──────────────────────────────────────────────────────────

  async submitUnban() {
    const userId = document.getElementById('unban-user-id')?.value.trim();

    if (!userId) { AdminUI.toast('User ID is required', 'warn'); return; }

    AdminUI.setLoading('unban-submit-btn', true);

    try {
      const res = await this._post('/admin/api/users/unban', { user_id: userId });

      if (res.status === 'success') {
        AdminUI.toast(`User #${userId} has been unbanned`, 'success');
        AdminUI.logAction('success', `Unbanned user <strong>#${userId}</strong>`);
        this.closeModal('unban-modal');
        AdminState.setUserBanned(Number(userId), false);
        AdminUsers.filter();
      } else {
        AdminUI.toast(res.message || 'Unban failed', 'error');
        AdminUI.logAction('error', `Unban failed for #${userId}: ${res.message || 'unknown'}`);
      }
    } catch (e) {
      AdminUI.toast('Request failed — see console', 'error');
      AdminUI.logAction('error', `Unban request error for #${userId}`);
      console.error('[admin] unban:', e);
    }

    AdminUI.setLoading('unban-submit-btn', false);
  },

  // ── Submit: delete ─────────────────────────────────────────────────────────

  async submitDelete() {
    const userId = document.getElementById('delete-user-id')?.value.trim();

    if (!userId) { AdminUI.toast('User ID is required', 'warn'); return; }

    AdminUI.setLoading('delete-submit-btn', true);

    try {
      const res = await this._delete(`/admin/api/users/${encodeURIComponent(userId)}`);

      if (res.status === 'success') {
        AdminUI.toast(`User #${userId} has been deleted`, 'success');
        AdminUI.logAction('error', `Deleted user <strong>#${userId}</strong>`);
        this.closeModal('delete-modal');
        AdminState.removeUser(Number(userId));
        AdminUsers.filter();
      } else {
        AdminUI.toast(res.message || 'Delete failed', 'error');
        AdminUI.logAction('error', `Delete failed for #${userId}: ${res.message || 'unknown'}`);
      }
    } catch (e) {
      AdminUI.toast('Request failed — see console', 'error');
      AdminUI.logAction('error', `Delete request error for #${userId}`);
      console.error('[admin] delete:', e);
    }

    AdminUI.setLoading('delete-submit-btn', false);
  },

  // ── Refresh all ────────────────────────────────────────────────────────────

  refreshAll() {
    AdminUsers.loadStats();
    if (AdminState.activeTab === 'users')  AdminUsers.reload();
    if (AdminState.activeTab === 'server') AdminUsers.loadStats();
    AdminUI.toast('Data refreshed', 'info');
    AdminUI.logAction('info', 'Manual refresh triggered');
  },

  // ── Setup ──────────────────────────────────────────────────────────────────

  setupBackdropDismiss() {
    document.querySelectorAll('.modal-backdrop').forEach(el => {
      el.addEventListener('click', e => {
        if (e.target === el) el.classList.remove('open');
      });
    });
  },

  setupKeyboard() {
    document.addEventListener('keydown', e => {
      if (e.key === 'Escape') {
        document.querySelectorAll('.modal-backdrop.open')
          .forEach(el => el.classList.remove('open'));
      }
    });
  },

  // ── HTTP ───────────────────────────────────────────────────────────────────

  async _post(url, body) {
    const res = await fetch(url, {
      method: 'POST',
      headers: { 'content-type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams(body).toString(),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.json();
  },

  async _delete(url) {
    const res = await fetch(url, { method: 'DELETE' });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.json();
  },
};
