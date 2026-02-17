/**
 * Settings — Initialiser
 * Auth guard, user data loading, and sub-module boot sequence.
 *
 * Load order (all deferred):
 *   utils.js → settings.nav.js → settings.profile.js → settings.account.js
 *            → settings.preferences.js → settings.init.js
 */

document.addEventListener('DOMContentLoaded', () => {
  // ── Auth guard ─────────────────────────────────────────────────────────────
  const user = Utils.getStorage('user');
  if (!user?.loggedIn) {
    window.location.href = '/';
    return;
  }

  // ── Navbar avatar ──────────────────────────────────────────────────────────
  const initialsEl = document.getElementById('userInitials');
  if (initialsEl) initialsEl.textContent = Utils.getInitials(user.name || user.email);

  // ── Platform info (Help tab) ───────────────────────────────────────────────
  const platformInfoEl = document.getElementById('platformInfo');
  if (platformInfoEl && window.PlatformConfig) {
    platformInfoEl.textContent = window.PlatformConfig.platform || 'Web';
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
