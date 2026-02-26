/**
 * Settings — Initialiser
 * User data loading and sub-module boot sequence.
 *
 * Load order (all deferred):
 *   theme.manager.js → platform.config.js → utils.js
 *   → settings.nav.js → settings.profile.js → settings.account.js
 *   → settings.preferences.js → settings.init.js
 */

document.addEventListener('DOMContentLoaded', () => {
  // ── Theme ──────────────────────────────────────────────────────────────────
  themeManager.init(['base', 'chat', 'settings']);

  document.getElementById('themeToggle')?.addEventListener('click', () => {
    themeManager.toggle();
  });

  // ── User data ──────────────────────────────────────────────────────────────
  // Read from storage — never rely on a bare `user` global.
  const user = Utils.getStorage('user') || {};

  // ── Navbar avatar ──────────────────────────────────────────────────────────
  const initialsEl = document.getElementById('userInitials');
  if (initialsEl) initialsEl.textContent = Utils.getInitials(user.name || user.email || '?');

  // ── Platform info (Help tab) ───────────────────────────────────────────────
  const platformEl = document.getElementById('platformInfo');
  if (platformEl && window.PlatformConfig) {
    platformEl.textContent = window.PlatformConfig.platform || 'Web';
  }

  // ── Boot sub-modules ───────────────────────────────────────────────────────
  SettingsNav.setup();

  SettingsProfile.load(user);
  SettingsProfile.setup();

  SettingsAccount.load(user);
  SettingsAccount.setup();

  SettingsPreferences.load();
  SettingsPreferences.setup();
});
