/**
 * Theme Manager
 * Handles loading and switching between light / dark themes.
 *
 * Usage (called by each page's init script):
 *   themeManager.init(['base', 'chat']);
 *
 * Pages must call init() themselves — this file does NOT auto-initialise.
 * Each page knows which CSS modules it needs; hardcoding them here would
 * load the wrong stylesheets on auth / settings / admin pages.
 */

const themeManager = {

  config: {
    themes:       ['light', 'dark'],
    defaultTheme: 'light',
    storageKey:   'theme',
    attribute:    'data-theme',
  },

  loadedStyles: new Set(),

  // ── Public API ─────────────────────────────────────────────────────────────

  /**
   * Initialise: apply the current theme and load CSS for the given modules.
   * Also wires the system preference change listener.
   * @param {string[]} modules  e.g. ['base', 'chat']
   */
  init(modules = ['base']) {
    const theme = this.getTheme();
    document.documentElement.setAttribute(this.config.attribute, theme);
    this._loadModules(modules, theme);
    this._watchSystem();
  },

  /**
   * Return the active theme name ('light' or 'dark').
   * Reads localStorage first, then falls back to the OS preference.
   */
  getTheme() {
    const stored = localStorage.getItem(this.config.storageKey);
    if (stored && this.config.themes.includes(stored)) return stored;
    return window.matchMedia?.('(prefers-color-scheme: dark)').matches
      ? 'dark'
      : this.config.defaultTheme;
  },

  /**
   * Persist a theme choice and reload the page so all CSS is applied cleanly.
   * @param {string} theme  'light' | 'dark'
   */
  setTheme(theme) {
    if (!this.config.themes.includes(theme)) {
      console.warn(`[themeManager] Unknown theme: "${theme}"`);
      return;
    }
    localStorage.setItem(this.config.storageKey, theme);
    window.location.reload();
  },

  /** Toggle between light and dark. */
  toggle() {
    this.setTheme(this.getTheme() === 'light' ? 'dark' : 'light');
  },

  // ── Private helpers ────────────────────────────────────────────────────────

  /**
   * Inject <link> tags for each module.
   * Light CSS is always loaded (it carries layout structure).
   * Dark CSS is loaded on top when the active theme is dark.
   */
  _loadModules(modules, theme) {
    modules.forEach(mod => {
      const lightHref = `static/css/${mod}/${mod}.light.css`;
      if (!this.loadedStyles.has(lightHref)) {
        this._appendLink(lightHref);
        this.loadedStyles.add(lightHref);
      }

      if (theme === 'dark') {
        const darkHref = `static/css/${mod}/${mod}.dark.css`;
        if (!this.loadedStyles.has(darkHref)) {
          this._appendLink(darkHref);
          this.loadedStyles.add(darkHref);
        }
      }
    });
  },

  _appendLink(href) {
    const link = document.createElement('link');
    link.rel   = 'stylesheet';
    link.href  = href;
    document.head.appendChild(link);
  },

  /** Auto-switch when the OS preference changes, unless the user has pinned a theme. */
  _watchSystem() {
    window.matchMedia?.('(prefers-color-scheme: dark)')
      .addEventListener('change', e => {
        if (!localStorage.getItem(this.config.storageKey)) {
          document.documentElement.setAttribute(
            this.config.attribute,
            e.matches ? 'dark' : 'light',
          );
          window.location.reload();
        }
      });
  },
};

// CommonJS compatibility (Node / test environments)
if (typeof module !== 'undefined' && module.exports) {
  module.exports = themeManager;
}

window.themeManager = themeManager;
