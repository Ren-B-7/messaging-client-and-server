/**
 * Auth — Initialiser
 * Entry point loaded on both index.html (login) and register.html.
 * Detects which form is present and delegates to the correct module.
 *
 * Load order (all deferred):
 *   utils.js → auth.password.js → auth.login.js → auth.register.js → auth.init.js
 */

function authInit() {
  if (document.getElementById('loginForm'))    AuthLogin.setup();
  if (document.getElementById('registerForm')) AuthRegister.setup();
}

// DOMContentLoaded fires after the HTML is parsed. When scripts use `defer`
// they execute after parsing too, so DOMContentLoaded may have already fired
// by the time this script runs. Checking readyState covers both cases.
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', authInit);
} else {
  authInit();
}
