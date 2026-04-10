/**
 * Settings — Initialiser
 */

import { themeManager } from "../../../static/js/full/utils/theme.manager.js";
import Utils from "../../../static/js/full/utils/utils.js";
import { SettingsNav } from "./settings.nav.js";
import { SettingsProfile } from "./settings.profile.js";
import { SettingsAccount } from "./settings.account.js";
import { SettingsPreferences } from "./settings.preferences.js";

document.addEventListener("DOMContentLoaded", async () => {
    // ── Theme ──────────────────────────────────────────────────────────────────
    themeManager.init(["base", "chat", "settings"]);

    const settingsThemeBtn = document.getElementById("themeToggle");
    themeManager.syncIcon(settingsThemeBtn);
    settingsThemeBtn?.addEventListener("click", () => {
        themeManager.toggle();
    });

    // ── Fetch fresh user data from the API ─────────────────────────────────────
    let user = Utils.getStorage("user") || {};

    try {
        const res = await fetch("/api/profile");
        if (res.ok) {
            const data = await res.json();
            const profile = data.data ?? data;
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

    // ── Boot sub-modules ───────────────────────────────────────────────────────
    SettingsNav.setup();

    SettingsProfile.load(user);
    SettingsProfile.setup();

    SettingsAccount.load(user);
    SettingsAccount.setup();

    SettingsPreferences.load();
    SettingsPreferences.setup();
});
