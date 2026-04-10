/**
 * Theme Manager
 * Handles loading and switching between light / dark themes.
 */

export const themeManager = {
    config: {
        themes: ["light", "dark"],
        defaultTheme: "light",
        storageKey: "theme",
        attribute: "data-theme",
    },

    _modules: [],

    init(modules = ["base"]) {
        this._modules = modules;
        const theme = this.getTheme();
        document.documentElement.setAttribute(this.config.attribute, theme);
        this._loadLight(modules);
        if (theme === "dark") this._enableDark(modules);
        this._watchSystem();
    },

    getTheme() {
        const stored = localStorage.getItem(this.config.storageKey);
        if (stored && this.config.themes.includes(stored)) return stored;
        return window.matchMedia?.("(prefers-color-scheme: dark)").matches
            ? "dark"
            : this.config.defaultTheme;
    },

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

        window.dispatchEvent(new CustomEvent("themechange", { detail: { theme } }));
    },

    toggle() {
        this.setTheme(this.getTheme() === "light" ? "dark" : "light");
    },

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
        window.addEventListener("themechange", update);
    },

    _loadLight(modules) {
        modules.forEach((mod) => {
            if (this._link(mod, "light")) return;
            this._appendLink(mod, "light");
        });
    },

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

    _disableDark(modules) {
        modules.forEach((mod) => {
            const link = this._link(mod, "dark");
            if (link) link.disabled = true;
        });
    },

    _link(mod, variant) {
        return document.querySelector(
            `link[data-theme-mod="${mod}"][data-theme-variant="${variant}"]`
        );
    },

    _appendLink(mod, variant) {
        const href = `static/css/min/${mod}/${mod}.${variant}.css`;
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

    _watchSystem() {
        window.matchMedia?.("(prefers-color-scheme: dark)").addEventListener("change", (e) => {
            if (!localStorage.getItem(this.config.storageKey)) {
                this.setTheme(e.matches ? "dark" : "light");
            }
        });
    },
};

window.themeManager = themeManager;
