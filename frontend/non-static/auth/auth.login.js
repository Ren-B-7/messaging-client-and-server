const AuthLogin = {
  setup() {
    const form = document.getElementById("loginForm");
    if (!form) return;

    AuthPassword.setupToggles();

    form.addEventListener("submit", async (e) => {
      e.preventDefault();

      const username = document.getElementById("username")?.value.trim();
      const password = document.getElementById("password")?.value;

      AuthLogin.clearErrors();

      if (!AuthLogin.validate(username, password)) return;

      const submitBtn = form.querySelector('button[type="submit"]');
      AuthLogin._setLoading(submitBtn, true);

      try {
        const response = await fetch("/api/login", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ username: username, password }),
        });

        // Server sent a redirect (302) — fetch followed it, navigate the browser there
        if (response.redirected) {
          localStorage.setItem("allowed", true);
          window.location.href = response.url;
          return;
        }

        const data = await response.json();

        if (data.status === "error") {
          AuthLogin._handleServerError(data);
          return;
        }

        // Success response with explicit redirect field
        if (data.redirect) {
          window.location.href = data.redirect;
        }
      } catch (err) {
        // Network failure or non-JSON response
        AuthLogin.showError(
          "username",
          "Could not reach the server. Please try again.",
        );
      } finally {
        AuthLogin._setLoading(submitBtn, false);
      }
    });

    this._checkUrlError();
  },

  _handleServerError(data) {
    switch (data.code) {
      case "INVALID_CREDENTIALS":
        AuthLogin.showError("username", data.message);
        AuthLogin.showError("password", " "); // mark field red without duplicate text
        break;
      case "USER_BANNED":
        AuthLogin.showError("username", data.message);
        break;
      case "MISSING_FIELD":
      case "INVALID_INPUT":
        AuthLogin.showError("username", data.message);
        break;
      default:
        AuthLogin.showError(
          "username",
          data.message ?? "An unexpected error occurred.",
        );
    }
  },

  // ─── URL error param (fallback for non-JS redirects) ────────────────────────

  _checkUrlError() {
    const error = new URLSearchParams(window.location.search).get("error");
    if (error === "invalid_credentials") {
      this.showError("username", "Invalid username or password");
    } else if (error === "invalid_input") {
      this.showError("username", "Please check your input");
    } else if (error === "invalid_request") {
      this.showError("username", "Invalid request. Please try again.");
    }
  },

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

  // ─── UI helpers ─────────────────────────────────────────────────────────────

  _setLoading(btn, isLoading) {
    if (!btn) return;
    btn.disabled = isLoading;
    btn.textContent = isLoading ? "Signing in..." : "Sign in";
  },

  /**
   * Display an error message below a field and mark it invalid.
   * Automatically clears on next input.
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
          if (errorEl) errorEl.style.display = "none";
        },
        { once: true },
      );
    }
  },

  /** Clear all visible form errors on the page. */
  clearErrors() {
    document.querySelectorAll(".form-error").forEach((el) => {
      el.textContent = "";
      el.style.display = "none";
    });
    document
      .querySelectorAll(".form-input")
      .forEach((el) => el.classList.remove("error"));
  },
};
