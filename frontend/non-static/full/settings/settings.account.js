/**
 * Settings — Account Actions
 */

import Utils from "../../../static/js/full/utils/utils.js";

export const SettingsAccount = {
    load(user) {
        const emailInput = document.getElementById("accountEmail");
        if (emailInput) emailInput.value = user.email || "";
    },

    setup() {
        document.getElementById("changePasswordBtn")?.addEventListener("click", () => {
            this._changePassword();
        });

        document.getElementById("deleteAccountBtn")?.addEventListener("click", () => {
            this._deleteAccount();
        });
    },

    async _changePassword() {
        const current = document.getElementById("currentPassword")?.value.trim();
        const next = document.getElementById("newPassword")?.value.trim();
        const confirm = document.getElementById("confirmPassword")?.value.trim();

        if (!current || !next || !confirm) {
            this._feedback("password", "All password fields are required.", "error");
            return;
        }
        if (next.length < 8) {
            this._feedback("password", "New password must be at least 8 characters.", "error");
            return;
        }
        if (next !== confirm) {
            this._feedback("password", "Passwords do not match.", "error");
            return;
        }

        const btn = document.getElementById("changePasswordBtn");
        this._setLoading(btn, true);

        try {
            const res = await this._post("/api/settings/password", {
                current_password: current,
                new_password: next,
                confirm_password: confirm,
            });

            if (res.status === "success") {
                this._feedback("password", "Password updated successfully.", "success");
                ["currentPassword", "newPassword", "confirmPassword"].forEach((id) => {
                    const el = document.getElementById(id);
                    if (el) el.value = "";
                });
            } else {
                this._feedback("password", res.message || "Failed to update password.", "error");
            }
        } catch (e) {
            this._feedback("password", "Request failed — check your connection.", "error");
            console.error("[settings] changePassword:", e);
        }

        this._setLoading(btn, false);
    },

    async _deleteAccount() {
        const emailConfirm = document.getElementById("deleteEmailConfirm")?.value.trim();
        const user = Utils.getStorage("user") || {};

        if (!emailConfirm) {
            this._feedback("delete", "Please enter your email address to confirm.", "error");
            return;
        }
        if (emailConfirm !== user.email) {
            this._feedback("delete", "Email address does not match.", "error");
            return;
        }

        const btn = document.getElementById("deleteAccountBtn");
        this._setLoading(btn, true);

        try {
            const res = await this._delete("/api/profile/delete");

            if (res.status === "success") {
                localStorage.clear();
                window.location.href = "/";
            } else {
                this._feedback("delete", res.message || "Failed to delete account.", "error");
                this._setLoading(btn, false);
            }
        } catch (e) {
            this._feedback("delete", "Request failed — check your connection.", "error");
            console.error("[settings] deleteAccount:", e);
            this._setLoading(btn, false);
        }
    },

    _feedback(section, message, type) {
        const el = document.getElementById(`${section}-feedback`);
        if (!el) return;
        el.textContent = message;
        el.className = `form-feedback ${type}`;
        el.style.display = "block";
        if (type === "success")
            setTimeout(() => {
                el.style.display = "none";
            }, 4000);
    },

    _setLoading(btn, loading) {
        if (!btn) return;
        btn.disabled = loading;
        if (loading) {
            btn._html = btn.innerHTML;
            btn.innerHTML = "Working…";
        } else if (btn._html) {
            btn.innerHTML = btn._html;
            delete btn._html;
        }
    },

    async _post(url, body) {
        const res = await fetch(url, {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify(body),
        });
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return res.json();
    },

    async _delete(url) {
        const res = await fetch(url, { method: "DELETE" });
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return res.json();
    },
};
