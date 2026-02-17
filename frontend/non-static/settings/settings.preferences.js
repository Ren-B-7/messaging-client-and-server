/**
 * Settings — Preferences
 * Handles theme radio selection and all toggle-switch preferences
 * (notifications, sound, last seen, profile photo).
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
    this._setChecked('showLastSeen',      prefs.showLastSeen      !== false);
    this._setChecked('showProfilePhoto',  prefs.showProfilePhoto  !== false);

    // Set the active theme radio.
    const currentTheme = themeManager.getTheme();
    const radio = document.getElementById(`${currentTheme}Theme`);
    if (radio) radio.checked = true;
  },

  /** Wire theme radios and all toggle switches. */
  setup() {
    this._setupTheme();
    this._setupToggles();
  },

  _setupTheme() {
    document.querySelectorAll('input[name="theme"]').forEach(radio => {
      radio.addEventListener('change', e => {
        if (e.target.checked) {
          themeManager.setTheme(e.target.value, ['base', 'chat', 'settings']);
        }
      });
    });
  },

  _setupToggles() {
    const bind = (id, prefKey, extraHandler) => {
      document.getElementById(id)?.addEventListener('change', e => {
        this._savePref(prefKey, e.target.checked);
        extraHandler?.(e.target.checked);
      });
    };

    bind('pushNotifications', 'pushNotifications', enabled => {
      if (enabled && window.PlatformConfig?.hasFeature('pushNotifications')) {
        if ('Notification' in window && Notification.permission === 'default') {
          Notification.requestPermission();
        }
      }
    });

    bind('notificationSound', 'notificationSound');
    bind('showLastSeen',      'showLastSeen');
    bind('showProfilePhoto',  'showProfilePhoto');
  },

  _savePref(key, value) {
    const prefs = Utils.getStorage('preferences') || {};
    prefs[key]  = value;
    Utils.setStorage('preferences', prefs);
  },

  _setChecked(id, value) {
    const el = document.getElementById(id);
    if (el) el.checked = value;
  },
};
