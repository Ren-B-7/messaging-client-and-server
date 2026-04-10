/**
 * Auth — Login
 * Handles the login form: validation, fetch, server error mapping,
 * and URL-error-param fallback for non-JS server redirects.
 */

import AuthPassword from "../../../static/js/full/auth/auth.password.js";

export const AuthLogin = {
    setup() {
        const form = document.getElementById("loginForm");
        if (!form) return;

        AuthPassword.setupToggles();

        form.addEventListener("submit", async (e) => {
            e.preventDefault();

            const username = document.getElementById("username")?.value.trim();
            const password = document.getElementById("password")?.value;
            const submitBtn = form.querySelector('button[type="submit"]');

            this.clearErrors();
            if (!this.validate(username, password)) return;

            this._setLoading(submitBtn, true);

            try {
                const response = await fetch("/login", {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ username, password }),
                });

                if (response.redirected) {
                    window.location.href = response.url;
                    return;
                }

                const contentType = response.headers.get("content-type") || "";
                if (!contentType.includes("application/json")) {
                    throw new Error(`Unexpected response (${response.status})`);
                }

                const data = await response.json();

                if (data.status === "error") {
                    this._handleServerError(data);
                    return;
                }

                if (data.redirect) {
                    window.location.href = data.redirect;
                    return;
                }

                this.showError("username", "Login succeeded but no redirect was provided.");
            } catch (err) {
                console.error("[login] error:", err);
                this.showError(
                    "username",
                    "Could not reach the server. Please check your connection and try again."
                );
            } finally {
                this._setLoading(submitBtn, false);
            }
        });

        this._checkUrlError();
    },

    _handleServerError(data) {
        const msg = data.message ?? "An unexpected error occurred.";
        switch (data.code) {
            case "INVALID_CREDENTIALS":
                this.showError("username", msg);
                this.showError("password", " ");
                break;
            case "USER_BANNED":
                this.showError("username", msg);
                break;
            case "RATE_LIMITED":
                this.showError("username", msg || "Too many attempts. Please wait a moment.");
                break;
            default:
                this.showError("username", msg);
        }
    },

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
                { once: true }
            );
        }
    },

    clearErrors() {
        document.querySelectorAll(".form-error").forEach((el) => {
            el.textContent = "";
            el.style.display = "none";
        });
        document.querySelectorAll(".form-input").forEach((el) => el.classList.remove("error"));
    },

    _setLoading(btn, isLoading) {
        if (!btn) return;
        btn.disabled = isLoading;
        btn.textContent = isLoading ? "Signing in…" : "Sign In";
    },
};
