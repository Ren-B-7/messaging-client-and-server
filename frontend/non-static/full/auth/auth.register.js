/**
 * Auth — Registration Flow
 *
 * Multi-step registration form with inline validation.
 */

import Utils from "../../../static/js/full/utils/utils.js";
import { AuthLogin } from "./auth.login.js";
import AuthPassword from "../../../static/js/full/auth/auth.password.js";

export const AuthRegister = {
    currentStep: 0,
    formData: {},

    setup() {
        const form = document.getElementById("registerForm");
        if (!form) return;

        AuthPassword.setupToggles();
        AuthPassword.setupStrengthMeter();
        this._setupStepNavigation();
        this._setupLiveValidation();

        form.addEventListener("submit", (e) => {
            e.preventDefault();
            this._handleRegistration();
        });

        this._checkUrlError();
    },

    _checkUrlError() {
        const error = new URLSearchParams(window.location.search).get("error");
        const map = {
            username_taken: [
                "regUsername",
                "This username is already taken. Please choose another.",
            ],
            email_taken: ["regEmail", "This email is already registered. Please sign in instead."],
            validation_failed: ["regEmail", "Please check your input and try again."],
            registration_failed: ["regEmail", "Registration failed. Please try again."],
        };
        const entry = map[error];
        if (entry) AuthLogin.showError(entry[0], entry[1]);
    },

    _setupLiveValidation() {
        document.getElementById("regEmail")?.addEventListener("blur", (e) => {
            const val = e.target.value.trim();
            if (val && !Utils.isValidEmail(val)) {
                AuthLogin.showError("regEmail", "Please enter a valid email address");
            }
        });

        document.getElementById("regPassword")?.addEventListener("input", (e) => {
            const val = e.target.value;
            if (val.length > 0 && val.length < 8) {
                AuthLogin.showError("regPassword", "Password must be at least 8 characters");
            } else {
                const errEl = document.getElementById("regPasswordError");
                if (errEl) {
                    errEl.textContent = "";
                    errEl.style.display = "none";
                }
                document.getElementById("regPassword")?.classList.remove("error");
            }
        });

        const checkMatch = () => {
            const pw = document.getElementById("regPassword")?.value;
            const cfm = document.getElementById("regConfirmPassword")?.value;
            if (cfm && pw !== cfm) {
                AuthLogin.showError("regConfirmPassword", "Passwords do not match");
            }
        };
        document.getElementById("regPassword")?.addEventListener("change", checkMatch);
        document.getElementById("regConfirmPassword")?.addEventListener("input", checkMatch);

        document.getElementById("regUsername")?.addEventListener("input", (e) => {
            const val = e.target.value;
            if (val && !/^[a-zA-Z0-9_]*$/.test(val)) {
                AuthLogin.showError(
                    "usernameError",
                    "Only letters, numbers and underscores are allowed"
                );
            }
        });
    },

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

        document.getElementById("prevBtn2")?.addEventListener("click", () => this._goToStep(0));
        document.getElementById("prevBtn3")?.addEventListener("click", () => this._goToStep(1));
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
            AuthLogin.showError("regEmail", "Please enter a valid email address");
            valid = false;
        }

        if (!password) {
            AuthLogin.showError("regPassword", "Password is required");
            valid = false;
        } else if (password.length < 8) {
            AuthLogin.showError("regPassword", "Password must be at least 8 characters");
            valid = false;
        } else {
            const { level } = AuthPassword._calcStrength(password);
            if (level === "weak") {
                AuthLogin.showError(
                    "regPassword",
                    "This password is too weak. Add uppercase letters, numbers, or symbols."
                );
                valid = false;
            }
        }

        if (password && confirm && password !== confirm) {
            AuthLogin.showError("regConfirmPassword", "Passwords do not match");
            valid = false;
        } else if (password && !confirm) {
            AuthLogin.showError("regConfirmPassword", "Please confirm your password");
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
        const username = document.getElementById("regUsername")?.value.trim();

        AuthLogin.clearErrors();
        let valid = true;

        if (!name) {
            AuthLogin.showError("fullName", "Full name is required");
            valid = false;
        } else if (name.length < 2) {
            AuthLogin.showError("fullName", "Name must be at least 2 characters");
            valid = false;
        }

        if (!username) {
            AuthLogin.showError("usernameError", "Username is required");
            valid = false;
        } else if (username.length < 3) {
            AuthLogin.showError("usernameError", "Username must be at least 3 characters");
            valid = false;
        } else if (username.length > 32) {
            AuthLogin.showError("usernameError", "Username must be 32 characters or fewer");
            valid = false;
        } else if (!/^[a-zA-Z0-9_]+$/.test(username)) {
            AuthLogin.showError(
                "usernameError",
                "Only letters, numbers and underscores are allowed"
            );
            valid = false;
        }

        if (valid) {
            this.formData.fullName = name;
            this.formData.username = username;
        }

        return valid;
    },

    _updateReview() {
        document.getElementById("reviewEmail").textContent = this.formData.email || "—";
        document.getElementById("reviewName").textContent = this.formData.fullName || "—";
        document.getElementById("reviewUsername").textContent = this.formData.username || "—";
    },

    async _handleRegistration() {
        if (!document.getElementById("termsCheckbox")?.checked) {
            AuthLogin.showError(
                "termsCheckbox",
                "Please accept the Terms of Service and Privacy Policy"
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
            const response = await fetch("/register", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(payload),
            });

            if (response.redirected) {
                window.location.href = response.url;
                return;
            }

            const contentType = response.headers.get("content-type") || "";
            if (!contentType.includes("application/json")) {
                throw new Error(`Unexpected response type: ${contentType}`);
            }

            const data = await response.json();
            if (data.status === "error") {
                this._handleServerError(data);
                return;
            }

            window.location.href = data.redirect ?? "/chat";
        } catch (err) {
            console.error("[register] submission error:", err);
            AuthLogin.showError(
                "regEmail",
                "Could not reach the server. Please check your connection and try again."
            );
        } finally {
            this._setLoading(submitBtn, false);
        }
    },

    _handleServerError(data) {
        const msg = data.message ?? "An unexpected error occurred. Please try again.";
        switch (data.code) {
            case "USERNAME_TAKEN":
            case "INVALID_USERNAME":
                this._goToStep(1);
                AuthLogin.showError("regUsername", msg);
                break;
            case "EMAIL_TAKEN":
            case "INVALID_EMAIL":
            case "EMAIL_REQUIRED":
                this._goToStep(0);
                AuthLogin.showError("regEmail", msg);
                break;
            case "INVALID_PASSWORD":
            case "WEAK_PASSWORD":
                this._goToStep(0);
                AuthLogin.showError("regPassword", msg);
                break;
            default:
                this._goToStep(0);
                AuthLogin.showError("regEmail", msg);
        }
    },

    _setLoading(btn, isLoading) {
        if (!btn) return;
        btn.disabled = isLoading;
        btn.textContent = isLoading ? "Creating account…" : "Create Account";
    },
};
