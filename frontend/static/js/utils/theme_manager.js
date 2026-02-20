/**
 * Theme Manager
 * Handles loading and switching between light/dark themes
 */

const themeManager = {
  // Theme configuration
  config: {
    themes: ["light", "dark"],
    defaultTheme: "light",
    storageKey: "theme",
    attribute: "data-theme",
  },

  // Loaded stylesheets
  loadedStyles: new Set(),

  /**
   * Initialize theme manager
   * @param {string[]} modules - Array of module names to load (e.g., ['base', 'chat'])
   */
  init(modules = ["base"]) {
    // Set initial theme
    const theme = this.getCurrentTheme();
    document.documentElement.setAttribute(this.config.attribute, theme);

    // Load CSS modules for BOTH themes (light has structure, dark has overrides)
    this.loadModules(modules, theme);

    // Listen for system theme changes
    this.setupSystemListener();
  },

  /**
   * Get current theme from storage or system preference
   */
  getCurrentTheme() {
    const stored = localStorage.getItem(this.config.storageKey);
    if (stored && this.config.themes.includes(stored)) {
      return stored;
    }

    // Check system preference
    if (
      window.matchMedia &&
      window.matchMedia("(prefers-color-scheme: dark)").matches
    ) {
      return "dark";
    }

    return this.config.defaultTheme;
  },

  /**
   * Set theme and reload page
   */
  setTheme(theme) {
    if (!this.config.themes.includes(theme)) {
      console.warn(`Invalid theme: ${theme}`);
      return;
    }

    localStorage.setItem(this.config.storageKey, theme);
    document.documentElement.setAttribute(this.config.attribute, theme);

    // Reload to apply new theme CSS files
    window.location.reload();
  },

  /**
   * Toggle between light and dark
   */
  toggle() {
    const current = this.getCurrentTheme();
    const next = current === "light" ? "dark" : "light";
    this.setTheme(next);
  },

  /**
   * Load CSS modules for current theme
   * CRITICAL: Load light theme FIRST (structure), then dark theme (overrides)
   */
  loadModules(modules, theme) {
    modules.forEach((module) => {
      // Always load light theme first for structure
      const lightHref = `/static/css/${module}/${module}.light.css`;
      if (!this.loadedStyles.has(lightHref)) {
        const lightLink = document.createElement("link");
        lightLink.rel = "stylesheet";
        lightLink.href = lightHref;
        document.head.appendChild(lightLink);
        this.loadedStyles.add(lightHref);
      }

      // If dark theme, also load dark overrides
      if (theme === "dark") {
        const darkHref = `/static/css/${module}/${module}.dark.css`;
        if (!this.loadedStyles.has(darkHref)) {
          const darkLink = document.createElement("link");
          darkLink.rel = "stylesheet";
          darkLink.href = darkHref;
          document.head.appendChild(darkLink);
          this.loadedStyles.add(darkHref);
        }
      }
    });
  },

  /**
   * Listen for system theme changes
   */
  setupSystemListener() {
    if (!window.matchMedia) return;

    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");

    mediaQuery.addEventListener("change", (e) => {
      // Only auto-switch if user hasn't manually set preference
      if (!localStorage.getItem(this.config.storageKey)) {
        const newTheme = e.matches ? "dark" : "light";
        document.documentElement.setAttribute(this.config.attribute, newTheme);
        window.location.reload();
      }
    });
  },
};

// Export for module systems or global use
if (typeof module !== "undefined" && module.exports) {
  module.exports = themeManager;
}

window.addEventListener("DOMContentLoaded", function () {
  if (typeof themeManager !== "undefined") {
    themeManager.init(["base", "chat"]);
    const themeToggle = document.getElementById("themeToggle");
    if (themeToggle) {
      themeToggle.addEventListener("click", function () {
        themeManager.toggle();
      });
    }
  }
});
