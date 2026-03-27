/**
 * Settings — Initialiser
 * Fetches the current user profile from the API (so avatar_url is always
 * fresh), merges into localStorage, then boots all settings sub-modules.
 *
 * Load order (all deferred):
 *   theme.manager.js → platform.config.js → utils.js
 *   → settings.nav.js → settings.profile.js → settings.account.js
 *   → settings.preferences.js → settings.init.js
 */

document.addEventListener("DOMContentLoaded", async () => {
    // ── Theme ──────────────────────────────────────────────────────────────────
    themeManager.init(["base", "chat", "settings"]);

    const settingsThemeBtn = document.getElementById("themeToggle");
    themeManager.syncIcon(settingsThemeBtn);
    settingsThemeBtn?.addEventListener("click", () => {
        themeManager.toggle();
    });

    // ── Fetch fresh user data from the API ─────────────────────────────────────
    // Always hit /api/profile so avatar_url, username, and email are current.
    let user = Utils.getStorage("user") || {};

    try {
        const res = await fetch("/api/profile");
        if (res.ok) {
            const data = await res.json();
            const profile = data.data ?? data;
            // Merge API fields into the local user object.
            user = {
                ...user,
                id: Number(profile.user_id),
                username: profile.username ?? "",
                email: profile.email ?? "",
                firstName: profile.first_name ?? "",
                lastName: profile.last_name ?? "",
                isAdmin: profile.is_admin ?? false,
                avatarUrl: profile.avatar_url ?? null,
            };
            Utils.setStorage("user", user);
        }
    } catch (e) {
        console.warn("[settings] Profile fetch failed, using cached data:", e);
    }

    // ── Navbar avatar chip ─────────────────────────────────────────────────────
    const initialsEl = document.getElementById("userInitials");
    const userAvatarEl = document.getElementById("userAvatarImg");

    if (user.avatarUrl && userAvatarEl) {
        userAvatarEl.src = user.avatarUrl;
        userAvatarEl.style.display = "block";
        if (initialsEl) initialsEl.style.display = "none";
    } else if (initialsEl) {
        const displayName = user.firstName
            ? `${user.firstName} ${user.lastName || ""}`.trim()
            : user.username || user.email || "?";
        initialsEl.textContent = Utils.getInitials(displayName);
        initialsEl.style.display = "";
    }

    // ── Platform info (Help tab) ───────────────────────────────────────────────
    const platformEl = document.getElementById("platformInfo");
    if (platformEl && window.PlatformConfig) {
        platformEl.textContent = window.PlatformConfig.platform || "Web";
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
