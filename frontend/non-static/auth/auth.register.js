/**
 * Auth — Registration Flow
 *
 * Multi-step registration form with inline validation.
 * Submits via fetch so JSON error responses from the server can be shown
 * inline. On success it follows the redirect URL in the response payload.
 *
 * Expected server responses:
 *   Error:   { "status": "error",   "code": "USERNAME_TAKEN", "message": "…" }
 *   Success: { "status": "success", "redirect": "/chat" }
 *
 * Depends on: Utils, AuthLogin (showError / clearErrors), AuthPassword
 */

const AuthRegister = {
  currentStep: 0,
  formData:    {},

  /** Boot the registration form if it exists on this page. */
  setup() {
    const form = document.getElementById('registerForm');
    if (!form) return;

    AuthPassword.setupToggles();
    AuthPassword.setupStrengthMeter();
    this._setupStepNavigation();

    form.addEventListener('submit', e => {
      e.preventDefault();
      this._handleRegistration();
    });

    this._checkUrlError();
  },

  // ── URL error param (server-side redirect fallback) ───────────────────────

  _checkUrlError() {
    const error = new URLSearchParams(window.location.search).get('error');
    const map   = {
      username_taken:      ['regUsername', 'This username is already taken. Please choose another.'],
      email_taken:         ['regEmail',    'This email is already registered. Please sign in instead.'],
      validation_failed:   ['regEmail',    'Please check your input and try again.'],
      registration_failed: ['regEmail',    'Registration failed. Please try again.'],
    };
    const entry = map[error];
    if (entry) AuthLogin.showError(entry[0], entry[1]);
  },

  // ── Step navigation ───────────────────────────────────────────────────────

  _setupStepNavigation() {
    document.getElementById('nextBtn1')?.addEventListener('click', () => {
      if (this._validateStep1()) this._goToStep(1);
    });

    document.getElementById('nextBtn2')?.addEventListener('click', () => {
      if (this._validateStep2()) {
        this._updateReview();
        this._goToStep(2);
      }
    });

    document.getElementById('prevBtn2')?.addEventListener('click', () => this._goToStep(0));
    document.getElementById('prevBtn3')?.addEventListener('click', () => this._goToStep(1));
  },

  _goToStep(n) {
    document.querySelectorAll('.step').forEach((el, i) => {
      el.classList.remove('active', 'completed');
      if      (i < n)  el.classList.add('completed');
      else if (i === n) el.classList.add('active');
    });

    document.querySelectorAll('.form-step').forEach((el, i) => {
      el.classList.toggle('active', i === n);
    });

    this.currentStep = n;
  },

  // ── Validation ────────────────────────────────────────────────────────────

  _validateStep1() {
    const email    = document.getElementById('regEmail')?.value.trim();
    const password = document.getElementById('regPassword')?.value;
    const confirm  = document.getElementById('regConfirmPassword')?.value;

    AuthLogin.clearErrors();
    let valid = true;

    if (!email) {
      AuthLogin.showError('regEmail', 'Email is required');
      valid = false;
    } else if (!Utils.isValidEmail(email)) {
      AuthLogin.showError('regEmail', 'Please enter a valid email');
      valid = false;
    }

    if (!password) {
      AuthLogin.showError('regPassword', 'Password is required');
      valid = false;
    } else if (password.length < 8) {
      AuthLogin.showError('regPassword', 'Password must be at least 8 characters');
      valid = false;
    }

    if (password !== confirm) {
      AuthLogin.showError('regConfirmPassword', 'Passwords do not match');
      valid = false;
    }

    if (valid) {
      this.formData.email            = email;
      this.formData.password         = password;
      this.formData.confirm_password = confirm;
    }

    return valid;
  },

  _validateStep2() {
    const name     = document.getElementById('fullName')?.value.trim();
    const username = document.getElementById('regUsername')?.value.trim();

    AuthLogin.clearErrors();
    let valid = true;

    if (!name) {
      AuthLogin.showError('fullName', 'Full name is required');
      valid = false;
    }

    if (!username) {
      AuthLogin.showError('usernameError', 'Username is required');
      valid = false;
    } else if (username.length < 3) {
      AuthLogin.showError('usernameError', 'Username must be at least 3 characters');
      valid = false;
    }

    if (valid) {
      this.formData.fullName = name;
      this.formData.username = username;
    }

    return valid;
  },

  // ── Review panel ──────────────────────────────────────────────────────────

  _updateReview() {
    document.getElementById('reviewEmail').textContent    = this.formData.email    || '—';
    document.getElementById('reviewName').textContent     = this.formData.fullName || '—';
    document.getElementById('reviewUsername').textContent = this.formData.username || '—';
  },

  // ── Submission ────────────────────────────────────────────────────────────

  async _handleRegistration() {
    if (!document.getElementById('termsCheckbox')?.checked) {
      AuthLogin.showError('termsCheckbox', 'Please accept the Terms of Service and Privacy Policy');
      return;
    }

    const submitBtn = document.getElementById('submitBtn');
    this._setLoading(submitBtn, true);

    const payload = {
      email:            this.formData.email,
      password:         this.formData.password,
      confirm_password: this.formData.confirm_password,
      username:         this.formData.username,
      full_name:        this.formData.fullName,
    };

    try {
      const response = await fetch('/api/register', {
        method:  'POST',
        headers: { 'Content-Type': 'application/json' },
        body:    JSON.stringify(payload),
      });

      if (response.redirected) {
        localStorage.setItem('allowed', 'true');
        window.location.href = response.url;
        return;
      }

      const data = await response.json();

      if (data.status === 'error') {
        this._handleServerError(data);
        return;
      }

      // Success — follow the redirect the server provided.
      window.location.href = data.redirect ?? '/chat';

    } catch {
      AuthLogin.showError('regEmail', 'Could not reach the server. Please try again.');
    } finally {
      this._setLoading(submitBtn, false);
    }
  },

  // ── Server error → field mapping ──────────────────────────────────────────

  _handleServerError(data) {
    const msg = data.message ?? 'An unexpected error occurred.';

    switch (data.code) {
      case 'USERNAME_TAKEN':
      case 'INVALID_USERNAME':
        this._goToStep(1);
        AuthLogin.showError('regUsername', msg);
        break;
      case 'EMAIL_TAKEN':
      case 'INVALID_EMAIL':
      case 'EMAIL_REQUIRED':
        this._goToStep(0);
        AuthLogin.showError('regEmail', msg);
        break;
      case 'INVALID_PASSWORD':
      case 'WEAK_PASSWORD':
        this._goToStep(0);
        AuthLogin.showError('regPassword', msg);
        break;
      default:
        // Unknown error — surface on step 1 so it's always visible.
        this._goToStep(0);
        AuthLogin.showError('regEmail', msg);
    }
  },

  // ── UI helpers ────────────────────────────────────────────────────────────

  _setLoading(btn, isLoading) {
    if (!btn) return;
    btn.disabled    = isLoading;
    btn.textContent = isLoading ? 'Creating account…' : 'Create Account';
  },
};
