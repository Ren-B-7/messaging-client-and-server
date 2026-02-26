/**
 * Auth — Initialiser
 * Entry point loaded on both index.html (login) and register.html.
 * Detects which form is present and delegates to the correct module.
 *
 * Load order (all deferred):
 *   theme.manager.js → platform.config.js → utils.js
 *   → auth.password.js → auth.login.js → auth.register.js → auth.init.js
 */

function authInit() {
  // ── Theme ────────────────────────────────────────────────────────────────
  themeManager.init(['base', 'auth']);

  // ── Form routing ─────────────────────────────────────────────────────────
  if (document.getElementById('loginForm'))    AuthLogin.setup();
  if (document.getElementById('registerForm')) AuthRegister.setup();
}

// DOMContentLoaded fires after HTML is parsed. Deferred scripts also run after
// parsing, so the event may have already fired by the time this runs.
// Checking readyState handles both cases safely.
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', authInit);
} else {
  authInit();
}
