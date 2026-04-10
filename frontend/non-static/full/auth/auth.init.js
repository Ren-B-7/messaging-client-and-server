/**
 * Auth — Initialiser
 * Entry point loaded on both index.html (login) and register.html.
 */

import { AuthLogin } from "./auth.login.js";
import { AuthRegister } from "./auth.register.js";

function authInit() {
    // ── Theme ────────────────────────────────────────────────────────────────
    if (window.themeManager) {
        window.themeManager.init(["base", "auth"]);
    }

    // ── Form routing ─────────────────────────────────────────────────────────
    if (document.getElementById("loginForm")) AuthLogin.setup();
    if (document.getElementById("registerForm")) AuthRegister.setup();
}

if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", authInit);
} else {
    authInit();
}
