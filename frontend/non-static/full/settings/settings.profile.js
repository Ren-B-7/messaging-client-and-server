/**
 * Settings — Profile Form
 */

import Utils from "../../../static/js/full/utils/utils.js";

export const SettingsProfile = {
    _original: {},

    load(user) {
        const profileName = document.getElementById("profileName");
        const profileEmail = document.getElementById("profileEmail");
        const profileAvatar = document.getElementById("profileAvatar");

        const displayName = user.firstName
            ? `${user.firstName} ${user.lastName || ""}`.trim()
            : user.username || "User";
        if (profileName) profileName.textContent = displayName;
        if (profileEmail) profileEmail.textContent = user.email || "";

        if (profileAvatar) {
            if (user.avatarUrl) {
                profileAvatar.innerHTML =
                    `<img src="${user.avatarUrl}" ` +
                    `style="width:100%;height:100%;object-fit:cover;border-radius:inherit" ` +
                    `alt="Profile picture" ` +
                    `onerror="this.parentElement.textContent='${Utils.getInitials(displayName)}'">`;
            } else {
                profileAvatar.textContent = Utils.getInitials(displayName);
            }
        }

        const firstName = document.getElementById("firstName");
        const lastName = document.getElementById("lastName");
        const username = document.getElementById("username");
        const emailEl = document.getElementById("email");
        if (firstName) firstName.value = user.firstName || "";
        if (lastName) lastName.value = user.lastName || "";
        if (username) username.value = user.username;
        if (emailEl) emailEl.value = user.email || "";

        this._original = {
            firstName: user.firstName || "",
            lastName: user.lastName || "",
            username: user.username || "",
            email: user.email || "",
        };
    },

    setup() {
        const form = document.getElementById("profileForm");
        const saveBtn = document.getElementById("saveProfileBtn");
        const cancelBtn = document.getElementById("cancelProfileBtn");

        if (form) {
            form.querySelectorAll("input").forEach((input) => {
                input.addEventListener("input", () => this._checkChanges(saveBtn));
            });

            cancelBtn?.addEventListener("click", () => {
                this._reset();
                if (saveBtn) saveBtn.disabled = true;
                this._feedback("", "");
            });

            form.addEventListener("submit", (e) => {
                e.preventDefault();
                this._save(saveBtn);
            });
        }

        const avatarEditBtn = document.getElementById("avatarEditBtn");
        const avatarFileInput = document.getElementById("avatarFileInput");

        if (avatarEditBtn && avatarFileInput) {
            avatarEditBtn.addEventListener("click", () => avatarFileInput.click());

            avatarFileInput.addEventListener("change", async () => {
                const file = avatarFileInput.files?.[0];
                avatarFileInput.value = "";
                if (file) await this._uploadAvatar(file);
            });
        }
    },

    async _uploadAvatar(file) {
        const statusEl = document.getElementById("avatarUploadStatus");
        const avatarEl = document.getElementById("profileAvatar");

        const setStatus = (text, color) => {
            if (!statusEl) return;
            statusEl.textContent = text;
            statusEl.style.color = color || "";
            statusEl.style.display = text ? "block" : "none";
        };

        setStatus("Uploading…", "");

        const formData = new FormData();
        formData.append("avatar", file);

        try {
            const res = await fetch("/api/profile/avatar", {
                method: "POST",
                body: formData,
            });

            if (!res.ok) {
                const err = await res.json().catch(() => ({}));
                throw new Error(err.message || `Upload failed (HTTP ${res.status})`);
            }

            const data = await res.json();
            const avatarUrl =
                (data.avatar_url || `/api/avatar/${Utils.getStorage("user")?.id}`) +
                "?t=" +
                Date.now();

            if (avatarEl) {
                const user = Utils.getStorage("user") || {};
                const name = user.username || "User";
                avatarEl.innerHTML =
                    `<img src="${avatarUrl}" ` +
                    `style="width:100%;height:100%;object-fit:cover;border-radius:inherit" ` +
                    `alt="Profile picture" ` +
                    `onerror="this.parentElement.textContent='${Utils.getInitials(name)}'">`;
            }

            const userAvatarEl = document.getElementById("userAvatarImg");
            const initialsEl = document.getElementById("userInitials");
            if (userAvatarEl) {
                userAvatarEl.src = avatarUrl;
                userAvatarEl.style.display = "block";
                if (initialsEl) initialsEl.style.display = "none";
            }

            const user = Utils.getStorage("user") || {};
            user.avatarUrl = data.avatar_url || `/api/avatar/${user.id}`;
            Utils.setStorage("user", user);

            setStatus("✓ Photo updated", "var(--success, #22c55e)");
            setTimeout(() => setStatus("", ""), 3000);
        } catch (e) {
            setStatus(`✕ ${e.message}`, "var(--danger, #ef4444)");
            setTimeout(() => setStatus("", ""), 5000);
            console.error("[settings] Avatar upload error:", e);
        }
    },

    _checkChanges(saveBtn) {
        const changed =
            (document.getElementById("firstName")?.value || "") !== this._original.firstName ||
            (document.getElementById("lastName")?.value || "") !== this._original.lastName ||
            (document.getElementById("username")?.value || "") !== this._original.username ||
            (document.getElementById("email")?.value || "") !== this._original.email;
        if (saveBtn) saveBtn.disabled = !changed;
    },

    _reset() {
        const firstName = document.getElementById("firstName");
        const lastName = document.getElementById("lastName");
        const username = document.getElementById("username");
        const emailEl = document.getElementById("email");
        if (firstName) firstName.value = this._original.firstName;
        if (lastName) lastName.value = this._original.lastName;
        if (username) username.value = this._original.username;
        if (emailEl) emailEl.value = this._original.email;
    },

    async _save(saveBtn) {
        const firstName = document.getElementById("firstName")?.value.trim() || "";
        const lastName = document.getElementById("lastName")?.value.trim() || "";
        const username = document.getElementById("username")?.value.trim() || "";
        const email = document.getElementById("email")?.value.trim() || "";

        const updates = {};
        if (firstName !== this._original.firstName || lastName !== this._original.lastName) {
            updates.name = { ...updates.name, first_name: firstName };
            updates.name = { ...updates.name, last_name: lastName };
        }
        if (username !== this._original.username) updates.username = username;
        if (email !== this._original.email) updates.email = email;

        if (Object.keys(updates).length === 0) return;

        this._setLoading(saveBtn, true);

        try {
            const res = await fetch("/api/profile", {
                method: "PUT",
                headers: { "content-type": "application/json" },
                body: JSON.stringify(updates),
            });
            if (!res.ok) throw new Error(`HTTP ${res.status}`);
            const data = await res.json();

            if (data.status === "success") {
                const fullName = `${firstName} ${lastName}`.trim();

                const user = Utils.getStorage("user") || {};
                user.firstName = firstName;
                user.lastName = lastName;
                user.username = username;
                if (email) user.email = email;
                Utils.setStorage("user", user);

                const profileName = document.getElementById("profileName");
                const profileEmailEl = document.getElementById("profileEmail");
                const profileAvatar = document.getElementById("profileAvatar");
                const userInitials = document.getElementById("userInitials");
                if (profileName) profileName.textContent = fullName || username;
                if (profileEmailEl && email) profileEmailEl.textContent = email;
                if (profileAvatar && !profileAvatar.querySelector("img")) {
                    profileAvatar.textContent = Utils.getInitials(fullName || username);
                }
                if (userInitials && !document.getElementById("userAvatarImg")?.src) {
                    userInitials.textContent = Utils.getInitials(fullName || username);
                }

                this._original = { firstName, lastName, username, email };
                if (saveBtn) saveBtn.disabled = true;
                this._feedback("Profile updated successfully.", "success");
            } else {
                this._feedback(data.message || "Failed to update profile.", "error");
            }
        } catch (e) {
            this._feedback("Request failed — check your connection.", "error");
            console.error("[settings] saveProfile:", e);
        } finally {
            this._setLoading(saveBtn, false);
        }
    },

    _feedback(message, type) {
        const el = document.getElementById("profile-feedback");
        if (!el) return;
        el.textContent = message;
        el.className = `form-feedback ${type}`;
        el.style.display = message ? "block" : "none";
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
            btn.innerHTML = "Saving…";
        } else if (btn._html) {
            btn.innerHTML = btn._html;
            delete btn._html;
        }
    },
};
