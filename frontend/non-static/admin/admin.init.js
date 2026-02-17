/**
 * Admin — Init
 * Entry point. Boots all sub-modules and attaches event listeners.
 *
 * Load order (all deferred, in sequence):
 *   theme_manager.js → platform.config.js → route.config.js → utils.js
 *   → admin.state.js → admin.ui.js → admin.users.js → admin.actions.js
 *   → admin.init.js
 */

window.addEventListener('DOMContentLoaded', () => {
  // ── Theme ────────────────────────────────────────────────────────────────
  themeManager.init(['base', 'admin']);

  document.getElementById('theme-toggle')?.addEventListener('click', () => {
    themeManager.toggle();
  });

  // ── Clock ─────────────────────────────────────────────────────────────────
  AdminUI.startClock();

  // ── Tab navigation ─────────────────────────────────────────────────────────
  document.querySelectorAll('.nav-btn[data-tab]').forEach(btn => {
    btn.addEventListener('click', () => AdminUI.switchTab(btn.dataset.tab, btn));
  });

  document.querySelectorAll('.sidebar-item[data-tab]').forEach(item => {
    item.addEventListener('click', () => {
      const navBtn = document.querySelector(`.nav-btn[data-tab="${item.dataset.tab}"]`);
      AdminUI.switchTab(item.dataset.tab, navBtn);
    });
  });

  // ── Modals ─────────────────────────────────────────────────────────────────
  AdminActions.setupBackdropDismiss();
  AdminActions.setupKeyboard();

  document.querySelectorAll('[data-open-modal]').forEach(el => {
    el.addEventListener('click', () => {
      const target = el.dataset.openModal;
      if (target === 'ban-modal')    AdminActions.openBanModal();
      if (target === 'unban-modal')  AdminActions.openUnbanModal();
      if (target === 'delete-modal') AdminActions.openDeleteModal();
    });
  });

  document.querySelectorAll('[data-close-modal]').forEach(el => {
    el.addEventListener('click', () => AdminActions.closeModal(el.dataset.closeModal));
  });

  document.getElementById('ban-submit-btn')?.addEventListener('click',
    () => AdminActions.submitBan());
  document.getElementById('unban-submit-btn')?.addEventListener('click',
    () => AdminActions.submitUnban());
  document.getElementById('delete-submit-btn')?.addEventListener('click',
    () => AdminActions.submitDelete());

  document.getElementById('delete-confirm-text')?.addEventListener('input',
    () => AdminActions.checkDeleteConfirm());

  // ── Search & filter ────────────────────────────────────────────────────────
  document.getElementById('user-search')?.addEventListener('input',
    Utils.debounce(() => AdminUsers.filter(), 250));
  document.getElementById('status-filter')?.addEventListener('change',
    () => AdminUsers.filter());

  // ── Misc buttons ───────────────────────────────────────────────────────────
  document.getElementById('refresh-btn')?.addEventListener('click',
    () => AdminActions.refreshAll());
  document.getElementById('reload-users-btn')?.addEventListener('click',
    () => AdminUsers.reload());
  document.getElementById('clear-log-btn')?.addEventListener('click',
    () => AdminUI.clearLog());

  // ── Platform info ──────────────────────────────────────────────────────────
  const platformEl = document.getElementById('platform-info');
  if (platformEl && window.PlatformConfig) {
    platformEl.textContent = window.PlatformConfig.platform || 'Web';
  }

  // ── Initial data load ──────────────────────────────────────────────────────
  const tsEl = document.getElementById('dash-ts');
  if (tsEl) tsEl.textContent = new Date().toLocaleString();

  AdminUsers.loadStats();
  AdminUI.renderRecentLog();
  AdminUI.logAction('info', 'Admin panel initialised');
});
