/**
 * Admin — Initialiser
 * Entry point. Boots all sub-modules and attaches event listeners.
 *
 * Load order (all deferred):
 *   theme.manager.js → platform.config.js → utils.js
 *   → admin.state.js → admin.ui.js → admin.users.js
 *   → admin.actions.js → admin.init.js
 */

window.addEventListener('DOMContentLoaded', () => {
  // ── Theme ──────────────────────────────────────────────────────────────────
  themeManager.init(['base', 'admin']);

  document.getElementById('theme-toggle')?.addEventListener('click', () => {
    themeManager.toggle();
  });

  // ── Clock ──────────────────────────────────────────────────────────────────
  AdminUI.startClock();

  // ── Tab navigation ─────────────────────────────────────────────────────────
  // Both the topbar buttons and sidebar items share the same data-tab attribute,
  // so we bind them with one loop each and route through AdminUI.switchTab.
  document.querySelectorAll('.nav-btn[data-tab]').forEach(btn => {
    btn.addEventListener('click', () => AdminUI.switchTab(btn.dataset.tab, btn));
  });

  document.querySelectorAll('.sidebar-item[data-tab]').forEach(item => {
    item.addEventListener('click', () => {
      // Keep the topbar active state in sync with the sidebar click.
      const navBtn = document.querySelector(`.nav-btn[data-tab="${item.dataset.tab}"]`);
      AdminUI.switchTab(item.dataset.tab, navBtn);
    });
  });

  // ── Modals ─────────────────────────────────────────────────────────────────
  AdminActions.setupBackdropDismiss();
  AdminActions.setupKeyboard();

  // data-open-modal buttons (both topbar and sidebar use these)
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
  // refresh-btn exists in both the sidebar and the dashboard header (refresh-btn-dash);
  // bind each separately so both trigger a full refresh.
  document.getElementById('refresh-btn')?.addEventListener('click',
    () => AdminActions.refreshAll());
  document.getElementById('refresh-btn-dash')?.addEventListener('click',
    () => AdminActions.refreshAll());
  document.getElementById('reload-users-btn')?.addEventListener('click',
    () => AdminUsers.reload());
  document.getElementById('reload-users-btn-tab')?.addEventListener('click',
    () => AdminUsers.reload());
  document.getElementById('reload-stats-btn')?.addEventListener('click',
    () => AdminUsers.loadStats());
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
