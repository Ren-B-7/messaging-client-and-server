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
 *
 * How switching works (no page reload):
 *   - Light CSS (<module>.light.css) is loaded once on init and never touched
 *     again — it carries all layout structure and is always active.
 *   - Dark CSS (<module>.dark.css) only overrides CSS custom-property tokens.
 *     Switching to dark:  inject the dark <link> tags (or re-enable them).
 *     Switching to light: disable the dark <link> tags in-place so the browser
 *                         drops them from the cascade without a network request
 *                         if toggled back.
 *   - The data-theme attribute on <html> is updated so all CSS selectors like
 *     :root[data-theme="dark"] respond immediately.
 */

const themeManager = {
    config: {
        themes: ["light", "dark"],
        defaultTheme: "light",
        storageKey: "theme",
        attribute: "data-theme",
    },

    // Tracks which modules this page has registered (set by init).
    _modules: [],

    // ── Public API ─────────────────────────────────────────────────────────────

    /**
     * Initialise: apply the current theme and load CSS for the given modules.
     * Also wires the system-preference change listener.
     * @param {string[]} modules  e.g. ['base', 'chat']
     */
    init(modules = ["base"]) {
        this._modules = modules;
        const theme = this.getTheme();
        document.documentElement.setAttribute(this.config.attribute, theme);
        this._loadLight(modules);
        if (theme === "dark") this._enableDark(modules);
        this._watchSystem();
    },

    /**
     * Return the active theme name ('light' or 'dark').
     * Reads localStorage first, then falls back to the OS preference.
     */
    getTheme() {
        const stored = localStorage.getItem(this.config.storageKey);
        if (stored && this.config.themes.includes(stored)) return stored;
        return window.matchMedia?.("(prefers-color-scheme: dark)").matches
            ? "dark"
            : this.config.defaultTheme;
    },

    /**
     * Switch to the given theme without reloading the page.
     * Light <link> tags are left untouched.
     * Dark  <link> tags are injected on first use, then enabled/disabled.
     * @param {string} theme  'light' | 'dark'
     */
    setTheme(theme) {
        if (!this.config.themes.includes(theme)) {
            console.warn(`[themeManager] Unknown theme: "${theme}"`);
            return;
        }
        localStorage.setItem(this.config.storageKey, theme);
        document.documentElement.setAttribute(this.config.attribute, theme);

        if (theme === "dark") {
            this._enableDark(this._modules);
        } else {
            this._disableDark(this._modules);
        }

        // Notify any listeners (e.g. icon sync, settings radio buttons).
        window.dispatchEvent(new CustomEvent("themechange", { detail: { theme } }));
    },

    /** Toggle between light and dark. */
    toggle() {
        this.setTheme(this.getTheme() === "light" ? "dark" : "light");
    },

    /**
     * Sync a theme-toggle button's <img> to reflect the action that clicking
     * it will perform — i.e. the OPPOSITE of the current theme.
     *
     *   current = light  →  next action = go dark   →  show moon.svg
     *   current = dark   →  next action = go light  →  show sun.svg
     *
     * Automatically re-syncs on every 'themechange' event so callers don't need
     * to do anything after calling toggle().
     *
     * @param {string|HTMLElement} btn  Button element or its id string.
     */
    syncIcon(btn) {
        const el = typeof btn === "string" ? document.getElementById(btn) : btn;
        if (!el) return;

        const update = () => {
            const img = el.querySelector("img");
            if (!img) return;
            const isDark = this.getTheme() === "dark";
            img.src = isDark ? "static/icons/icons/sun.svg" : "static/icons/icons/moon.svg";
            img.alt = isDark ? "Switch to light mode" : "Switch to dark mode";
        };

        update();

        // Re-sync automatically whenever the theme changes — no manual calls
        // needed.
        window.addEventListener("themechange", update);
    },

    // ── Private helpers ────────────────────────────────────────────────────────

    /**
     * Ensure a light <link> exists for every module.
     * Light links are injected once and never removed.
     * Uses data-theme-mod / data-theme-variant attributes as stable selectors
     * so we can find them later without relying on href matching.
     */
    _loadLight(modules) {
        modules.forEach((mod) => {
            if (this._link(mod, "light")) return; // already present (e.g. from <head>)
            this._appendLink(mod, "light");
        });
    },

    /**
     * Inject dark <link> tags if not yet present, or re-enable them if they
     * were previously disabled.
     */
    _enableDark(modules) {
        modules.forEach((mod) => {
            const existing = this._link(mod, "dark");
            if (existing) {
                existing.disabled = false;
            } else {
                this._appendLink(mod, "dark");
            }
        });
    },

    /**
     * Disable dark <link> tags in-place.
     * Setting .disabled = true removes the stylesheet from the cascade
     * immediately without removing the DOM node, so re-enabling is instant
     * (no network round-trip).
     */
    _disableDark(modules) {
        modules.forEach((mod) => {
            const link = this._link(mod, "dark");
            if (link) link.disabled = true;
        });
    },

    /** Find an existing managed <link> by module name and variant. */
    _link(mod, variant) {
        return (
            document.querySelector(
                `link[data-theme-mod="${mod}"][data-theme-variant="${variant}"]`
            ) || null
        );
    },

    _appendLink(mod, variant) {
        // Also check for a <link> that was pre-rendered in <head> by the inline
        // theme-detection script (which sets href but not our data attributes).
        // If found, adopt it rather than adding a duplicate.
        const href = `static/css/${mod}/${mod}.${variant}.css`;
        const prerendered = document.querySelector(`link[href="${href}"]`);
        if (prerendered) {
            prerendered.dataset.themeMod = mod;
            prerendered.dataset.themeVariant = variant;
            return prerendered;
        }

        const link = document.createElement("link");
        link.rel = "stylesheet";
        link.href = href;
        link.dataset.themeMod = mod;
        link.dataset.themeVariant = variant;
        document.head.appendChild(link);
        return link;
    },

    /**
     Auto-switch when the OS preference changes, unless the user has pinned a
     theme.
   */
    _watchSystem() {
        window.matchMedia?.("(prefers-color-scheme: dark)").addEventListener("change", (e) => {
            if (!localStorage.getItem(this.config.storageKey)) {
                this.setTheme(e.matches ? "dark" : "light");
            }
        });
    },
};

// CommonJS compatibility (Node / test environments)
if (typeof module !== "undefined" && module.exports) {
    module.exports = themeManager;
}

window.themeManager = themeManager;
