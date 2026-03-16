/**
 * Settings — Preferences
 * Handles theme radio selection and all toggle-switch preferences.
 * Each change is persisted to localStorage for instant local effect
 * and also synced to /api/user/preferences as JSON.
 * Depends on: Utils, themeManager
 */

const SettingsPreferences = {

  /**
   * Read stored preferences and pre-check the relevant controls.
   */
  load() {
    const prefs = Utils.getStorage('preferences') || {};

    this._setChecked('pushNotifications', prefs.pushNotifications !== false);
    this._setChecked('notificationSound', prefs.notificationSound !== false);
    this._setChecked('showLastSeen', prefs.showLastSeen !== false);
    this._setChecked('showProfilePhoto', prefs.showProfilePhoto !== false);

    const radio = document.getElementById(`${themeManager.getTheme()}Theme`);
    if (radio)
      radio.checked = true;
  },

  /** Wire theme radios and all toggle switches. */
  setup() {
    this._setupThemeRadios();
    this._setupToggles();
  },

  // ── Private ───────────────────────────────────────────────────────────────

  _setupThemeRadios() {
    document.querySelectorAll('input[name="theme"]').forEach(radio => {
      radio.addEventListener('change', e => {
        if (e.target.checked) {
          themeManager.setTheme(e.target.value);
          // Theme is managed client-side only; no API call needed.
        }
      });
    });
  },

  _setupToggles() {
    const bind = (id, prefKey, extraHandler) => {
      document.getElementById(id)?.addEventListener('change', e => {
        const value = e.target.checked;
        this._savePref(prefKey, value);
        this._syncToApi({[prefKey] : value});
        extraHandler?.(value);
      });
    };

    bind('pushNotifications', 'pushNotifications', enabled => {
      if (enabled && 'Notification' in window &&
          Notification.permission === 'default') {
        Notification.requestPermission();
      }
    });

    bind('notificationSound', 'notificationSound');
    bind('showLastSeen', 'showLastSeen');
    bind('showProfilePhoto', 'showProfilePhoto');
  },

  _savePref(key, value) {
    const prefs = Utils.getStorage('preferences') || {};
    prefs[key] = value;
    Utils.setStorage('preferences', prefs);
  },

  async _syncToApi(patch) {
    try {
      const res = await fetch('/api/user/preferences', {
        method : 'PUT',
        headers : {'content-type' : 'application/json'},
        body : JSON.stringify(patch),
      });
      if (!res.ok)
        throw new Error(`HTTP ${res.status}`);
    } catch (e) {
      // Preference sync failures are non-critical; log but don't surface to
      // user.
      console.warn('[settings] preferences sync failed:', e);
    }
  },

  _setChecked(id, value) {
    const el = document.getElementById(id);
    if (el)
      el.checked = value;
  },
};
