/**
 * Settings — Profile Form
 * Populates the profile tab with user data, detects unsaved changes,
 * handles save / cancel, and wires the avatar upload button.
 *
 * Avatar API:  POST /api/profile/avatar   multipart field: "avatar"
 * Profile API: PUT  /api/profile          JSON { username, email }
 *
 * Depends on: Utils
 */

const SettingsProfile = {
    _original: {},

    /**
     * Populate form fields and avatar display from user data.
     * @param {object} user  — merged object from localStorage + /api/profile
     */
    load(user) {
        // ── Profile header ───────────────────────────────────────────────────────
        const profileName = document.getElementById("profileName");
        const profileEmail = document.getElementById("profileEmail");
        const profileAvatar = document.getElementById("profileAvatar");

        const displayName = user.username || user.name || "User";
        if (profileName) profileName.textContent = displayName;
        if (profileEmail) profileEmail.textContent = user.email || "";

        // Show avatar image if one exists, otherwise fall back to initials.
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

        // ── Form fields ──────────────────────────────────────────────────────────
        // The API only stores username and email; first/last name fields are kept
        // for UI consistency but map back to username on save.
        const fullName = user.name || "";
        const names = fullName.split(" ");

        const firstName = document.getElementById("firstName");
        const lastName = document.getElementById("lastName");
        const username = document.getElementById("username");
        if (firstName) firstName.value = names[0] || "";
        if (lastName) lastName.value = names.slice(1).join(" ") || "";
        if (username) username.value = user.username || "";

        this._original = {
            firstName: names[0] || "",
            lastName: names.slice(1).join(" ") || "",
            username: user.username || "",
        };
    },

    /** Attach all event listeners for the profile tab. */
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

        // ── Avatar upload wiring ─────────────────────────────────────────────────
        const avatarEditBtn = document.getElementById("avatarEditBtn");
        const avatarFileInput = document.getElementById("avatarFileInput");

        if (avatarEditBtn && avatarFileInput) {
            // Clicking the edit button opens the file picker.
            avatarEditBtn.addEventListener("click", () => avatarFileInput.click());

            // Once the user picks a file, upload immediately.
            avatarFileInput.addEventListener("change", async () => {
                const file = avatarFileInput.files?.[0];
                avatarFileInput.value = ""; // reset so same file can be re-selected
                if (file) await this._uploadAvatar(file);
            });
        }
    },

    // ── Avatar upload ─────────────────────────────────────────────────────────

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
            // Cache-bust so the browser fetches the fresh image immediately.
            const avatarUrl =
                (data.avatar_url || `/api/avatar/${Utils.getStorage("user")?.id}`) +
                "?t=" +
                Date.now();

            // Update the profile avatar circle.
            if (avatarEl) {
                const user = Utils.getStorage("user") || {};
                const name = user.username || "User";
                avatarEl.innerHTML =
                    `<img src="${avatarUrl}" ` +
                    `style="width:100%;height:100%;object-fit:cover;border-radius:inherit" ` +
                    `alt="Profile picture" ` +
                    `onerror="this.parentElement.textContent='${Utils.getInitials(name)}'">`;
            }

            // Update the navbar chip.
            const userAvatarEl = document.getElementById("userAvatarImg");
            const initialsEl = document.getElementById("userInitials");
            if (userAvatarEl) {
                userAvatarEl.src = avatarUrl;
                userAvatarEl.style.display = "block";
                if (initialsEl) initialsEl.style.display = "none";
            }

            // Persist the new URL so the next page load picks it up from storage.
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

    // ── Profile form ──────────────────────────────────────────────────────────

    _checkChanges(saveBtn) {
        const changed =
            (document.getElementById("firstName")?.value || "") !== this._original.firstName ||
            (document.getElementById("lastName")?.value || "") !== this._original.lastName ||
            (document.getElementById("username")?.value || "") !== this._original.username;
        if (saveBtn) saveBtn.disabled = !changed;
    },

    _reset() {
        const firstName = document.getElementById("firstName");
        const lastName = document.getElementById("lastName");
        const username = document.getElementById("username");
        if (firstName) firstName.value = this._original.firstName;
        if (lastName) lastName.value = this._original.lastName;
        if (username) username.value = this._original.username;
    },

    async _save(saveBtn) {
        const firstName = document.getElementById("firstName")?.value.trim();
        const lastName = document.getElementById("lastName")?.value.trim() || "";
        const username = document.getElementById("username")?.value.trim() || "";

        if (!firstName) {
            this._feedback("First name is required.", "error");
            return;
        }

        this._setLoading(saveBtn, true);

        try {
            const res = await fetch("/api/profile", {
                method: "PUT",
                headers: { "content-type": "application/json" },
                body: JSON.stringify({ username }),
            });

            if (!res.ok) throw new Error(`HTTP ${res.status}`);
            const data = await res.json();

            if (data.status === "success") {
                const fullName = `${firstName} ${lastName}`.trim();

                const user = Utils.getStorage("user") || {};
                user.name = fullName;
                user.username = username;
                Utils.setStorage("user", user);

                const profileName = document.getElementById("profileName");
                const profileAvatar = document.getElementById("profileAvatar");
                const userInitials = document.getElementById("userInitials");
                if (profileName) profileName.textContent = fullName || username;
                // Only reset to initials if no avatar is currently shown.
                if (profileAvatar && !profileAvatar.querySelector("img")) {
                    profileAvatar.textContent = Utils.getInitials(fullName || username);
                }
                if (userInitials && !document.getElementById("userAvatarImg")?.src) {
                    userInitials.textContent = Utils.getInitials(fullName || username);
                }

                this._original = { firstName, lastName, username };
                if (saveBtn) saveBtn.disabled = true;
                this._feedback("Profile updated successfully.", "success");
            } else {
                this._feedback(data.message || "Failed to update profile.", "error");
            }
        } catch (e) {
            this._feedback("Request failed — check your connection.", "error");
            console.error("[settings] saveProfile:", e);
        }

        this._setLoading(saveBtn, false);
    },

    // ── Helpers ───────────────────────────────────────────────────────────────

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
