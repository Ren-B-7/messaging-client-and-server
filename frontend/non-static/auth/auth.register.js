/**
 * Auth — Registration Flow
 *
 * Submits via fetch so JSON error responses from the server can be caught
 * and displayed inline. On success follows the redirect URL in the payload.
 *
 * Expected server responses:
 *   Error:   { "status": "error",   "code": "USERNAME_TAKEN", "message": "..." }
 *   Success: { "status": "success", ..., "redirect": "/chat" }
 *
 * Multi-step registration form with validation.
 * Depends on: Utils, AuthLogin (showError / clearErrors), AuthPassword
 */

const AuthRegister = {
  currentStep: 0,
  formData: {},

  /**
   * Boot the registration form if it exists on this page.
   */
  setup() {
    const form = document.getElementById("registerForm");
    if (!form) return;

    AuthPassword.setupToggles();
    AuthPassword.setupStrengthMeter();

    this._setupStepNavigation();

    form.addEventListener("submit", (e) => {
      e.preventDefault();
      this._handleRegistration();
    });

    this._checkUrlError();
  },

  // ─── URL error param (fallback for non-JS redirects) ────────────────────────

  _checkUrlError() {
    const error = new URLSearchParams(window.location.search).get("error");
    if (error === "username_taken") {
      AuthLogin.showError(
        "username",
        "This username is already taken. Please choose another.",
      );
    } else if (error === "email_taken") {
      AuthLogin.showError(
        "regEmail",
        "This email is already registered. Please sign in instead.",
      );
    } else if (error === "validation_failed") {
      AuthLogin.showError("regEmail", "Please check your input and try again.");
    } else if (error === "registration_failed") {
      AuthLogin.showError("regEmail", "Registration failed. Please try again.");
    }
  },

  // ─── Step navigation ─────────────────────────────────────────────────────────

  _setupStepNavigation() {
    document.getElementById("nextBtn1")?.addEventListener("click", () => {
      if (this._validateStep1()) this._goToStep(1);
    });

    document.getElementById("nextBtn2")?.addEventListener("click", () => {
      if (this._validateStep2()) {
        this._updateReview();
        this._goToStep(2);
      }
    });

    document
      .getElementById("prevBtn2")
      ?.addEventListener("click", () => this._goToStep(0));
    document
      .getElementById("prevBtn3")
      ?.addEventListener("click", () => this._goToStep(1));
  },

  _goToStep(n) {
    document.querySelectorAll(".step").forEach((el, i) => {
      el.classList.remove("active", "completed");
      if (i < n) el.classList.add("completed");
      else if (i === n) el.classList.add("active");
    });

    document.querySelectorAll(".form-step").forEach((el, i) => {
      el.classList.toggle("active", i === n);
    });

    this.currentStep = n;
  },

  // ─── Validation ──────────────────────────────────────────────────────────────

  _validateStep1() {
    const email = document.getElementById("regEmail")?.value.trim();
    const password = document.getElementById("regPassword")?.value;
    const confirm = document.getElementById("regConfirmPassword")?.value;

    AuthLogin.clearErrors();
    let valid = true;

    if (!email) {
      AuthLogin.showError("regEmail", "Email is required");
      valid = false;
    } else if (!Utils.isValidEmail(email)) {
      AuthLogin.showError("regEmail", "Please enter a valid email");
      valid = false;
    }

    if (!password) {
      AuthLogin.showError("regPassword", "Password is required");
      valid = false;
    } else if (password.length < 8) {
      AuthLogin.showError(
        "regPassword",
        "Password must be at least 8 characters",
      );
      valid = false;
    }

    if (password !== confirm) {
      AuthLogin.showError("regConfirmPassword", "Passwords do not match");
      valid = false;
    }

    if (valid) {
      this.formData.email = email;
      this.formData.password = password;
      this.formData.confirm_password = confirm;
    }

    return valid;
  },

  _validateStep2() {
    const name = document.getElementById("fullName")?.value.trim();
    const username = document.getElementById("username")?.value.trim();

    AuthLogin.clearErrors();
    let valid = true;

    if (!name) {
      AuthLogin.showError("fullName", "Full name is required");
      valid = false;
    }

    if (!username) {
      AuthLogin.showError("username", "Username is required");
      valid = false;
    } else if (username.length < 3) {
      AuthLogin.showError("username", "Username must be at least 3 characters");
      valid = false;
    }

    if (valid) {
      this.formData.fullName = name;
      this.formData.username = username;
    }

    return valid;
  },

  // ─── Review panel ────────────────────────────────────────────────────────────

  _updateReview() {
    document.getElementById("reviewEmail").textContent =
      this.formData.email || "-";
    document.getElementById("reviewName").textContent =
      this.formData.fullName || "-";
    document.getElementById("reviewUsername").textContent =
      this.formData.username || "-";
  },

  // ─── Submission ──────────────────────────────────────────────────────────────

  async _handleRegistration() {
    const termsCheckbox = document.getElementById("termsCheckbox");
    if (!termsCheckbox?.checked) {
      AuthLogin.showError(
        "termsCheckbox",
        "Please accept the Terms of Service and Privacy Policy",
      );
      return;
    }

    const submitBtn = document.getElementById("submitBtn");
    this._setLoading(submitBtn, true);

    const payload = {
      email: this.formData.email,
      password: this.formData.password,
      confirm_password: this.formData.confirm_password,
      username: this.formData.username,
      full_name: this.formData.fullName,
    };

    try {
      const response = await fetch("/api/register", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });

      if (response.redirected) {
        window.location.href = response.url;
        return;
      }

      const data = await response.json();

      if (data.status === "error") {
        this._handleServerError(data);
        return;
      }

      // Success — follow the redirect the server provided
      window.location.href = data.redirect ?? "/chat";
    } catch (err) {
      // Network failure or non-JSON response
      AuthLogin.showError(
        "regEmail",
        "Could not reach the server. Please try again.",
      );
    } finally {
      this._setLoading(submitBtn, false);
    }
  },

  // ─── Server error → field mapping ────────────────────────────────────────────

  _handleServerError(data) {
    switch (data.code) {
      case "USERNAME_TAKEN":
        // Step back so the user can see and fix the username field
        this._goToStep(1);
        AuthLogin.showError("username", data.message);
        break;
      case "EMAIL_TAKEN":
        this._goToStep(0);
        AuthLogin.showError("regEmail", data.message);
        break;
      case "INVALID_USERNAME":
        this._goToStep(1);
        AuthLogin.showError("username", data.message);
        break;
      case "INVALID_PASSWORD":
        this._goToStep(0);
        AuthLogin.showError("regPassword", data.message);
        break;
      case "WEAK_PASSWORD":
        this._goToStep(0);
        AuthLogin.showError("regPassword", data.message);
        break;
      case "INVALID_EMAIL":
        this._goToStep(0);
        AuthLogin.showError("regEmail", data.message);
        break;
      case "EMAIL_REQUIRED":
        this._goToStep(0);
        AuthLogin.showError("regEmail", data.message);
        break;
      case "MISSING_FIELD":
        // message names the field — surface it at the top of step 1 as a fallback
        this._goToStep(0);
        AuthLogin.showError("regEmail", data.message);
        break;
      default:
        // Unknown error — show on step 1 so it's visible
        this._goToStep(0);
        AuthLogin.showError(
          "regEmail",
          data.message ?? "An unexpected error occurred.",
        );
    }
  },

  // ─── UI helpers ──────────────────────────────────────────────────────────────

  _setLoading(btn, isLoading) {
    if (!btn) return;
    btn.disabled = isLoading;
    btn.textContent = isLoading ? "Creating account..." : "Create account";
  },
};
