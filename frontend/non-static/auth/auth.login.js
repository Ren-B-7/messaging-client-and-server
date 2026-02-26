/**
 * Auth — Login
 * Handles the login form: validation, fetch, server error mapping,
 * and URL-error-param fallback for non-JS server redirects.
 * Depends on: AuthPassword
 */

const AuthLogin = {
  setup() {
    const form = document.getElementById("loginForm");
    if (!form) return;

    AuthPassword.setupToggles();

    form.addEventListener("submit", async (e) => {
      e.preventDefault();

      const username = document.getElementById("username")?.value.trim();
      const password = document.getElementById("password")?.value;
      const submitBtn = form.querySelector('button[type="submit"]');

      AuthLogin.clearErrors();
      if (!AuthLogin.validate(username, password)) return;

      AuthLogin._setLoading(submitBtn, true);

      try {
        const response = await fetch("/api/login", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ username, password }),
        });

        // fetch followed a server-side 302 redirect automatically.
        if (response.redirected) {
          window.location.href = response.url;
          return;
        }

        // Guard against non-JSON error pages.
        const contentType = response.headers.get("content-type") || "";
        if (!contentType.includes("application/json")) {
          throw new Error(`Unexpected response (${response.status})`);
        }

        const data = await response.json();

        if (data.status === "error") {
          AuthLogin._handleServerError(data);
          return;
        }

        if (data.redirect) {
          window.location.href = data.redirect;
          return;
        }

        // Success but no redirect — shouldn't normally happen.
        AuthLogin.showError(
          "username",
          "Login succeeded but no redirect was provided.",
        );
      } catch (err) {
        console.error("[login] error:", err);
        AuthLogin.showError(
          "username",
          "Could not reach the server. Please check your connection and try again.",
        );
      } finally {
        AuthLogin._setLoading(submitBtn, false);
      }
    });

    this._checkUrlError();
  },

  // ── Server error mapping ──────────────────────────────────────────────────

  _handleServerError(data) {
    const msg = data.message ?? "An unexpected error occurred.";
    switch (data.code) {
      case "INVALID_CREDENTIALS":
        AuthLogin.showError("username", msg);
        AuthLogin.showError("password", " "); // mark field red, text shown on username
        break;
      case "USER_BANNED":
        AuthLogin.showError("username", msg);
        break;
      case "RATE_LIMITED":
        AuthLogin.showError(
          "username",
          msg || "Too many attempts. Please wait a moment.",
        );
        break;
      case "MISSING_FIELD":
      case "INVALID_INPUT":
        AuthLogin.showError("username", msg);
        break;
      default:
        AuthLogin.showError("username", msg);
    }
  },

  // ── URL error param (server-side redirect fallback) ───────────────────────

  _checkUrlError() {
    const error = new URLSearchParams(window.location.search).get("error");
    const map = {
      invalid_credentials: "Invalid username or password",
      invalid_input: "Please check your input",
      invalid_request: "Invalid request. Please try again.",
      rate_limited: "Too many attempts. Please wait a moment.",
      banned: "Your account has been suspended.",
    };
    const msg = map[error];
    if (msg) this.showError("username", msg);
  },

  // ── Client-side validation ────────────────────────────────────────────────

  validate(username, password) {
    let valid = true;

    if (!username) {
      this.showError("username", "Username is required");
      valid = false;
    }

    if (!password) {
      this.showError("password", "Password is required");
      valid = false;
    }

    return valid;
  },

  // ── Shared UI helpers (also used by auth.register.js) ────────────────────

  /**
   * Show an error message below a field and mark the input red.
   * The error clears automatically on the next input event.
   * @param {string} fieldId  The input element's id (error el is `${fieldId}Error`)
   * @param {string} message
   */
  showError(fieldId, message) {
    const errorEl = document.getElementById(`${fieldId}Error`);
    const inputEl = document.getElementById(fieldId);

    if (errorEl && message.trim()) {
      errorEl.textContent = message;
      errorEl.style.display = "block";
    }

    if (inputEl) {
      inputEl.classList.add("error");
      inputEl.addEventListener(
        "input",
        () => {
          inputEl.classList.remove("error");
          if (errorEl) {
            errorEl.textContent = "";
            errorEl.style.display = "none";
          }
        },
        { once: true },
      );
    }
  },

  /** Clear all visible form errors and error styling on the page. */
  clearErrors() {
    document.querySelectorAll(".form-error").forEach((el) => {
      el.textContent = "";
      el.style.display = "none";
    });
    document
      .querySelectorAll(".form-input")
      .forEach((el) => el.classList.remove("error"));
  },

  // ── Loading state ─────────────────────────────────────────────────────────

  _setLoading(btn, isLoading) {
    if (!btn) return;
    btn.disabled = isLoading;
    btn.textContent = isLoading ? "Signing in…" : "Sign In";
  },
};
