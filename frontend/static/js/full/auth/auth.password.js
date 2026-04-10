/**
 * Auth — Password & Avatar Helpers
 * Password visibility toggle, strength meter, and avatar upload.
 */

export const AuthPassword = {
    // ── Password visibility toggle ────────────────────────────────────────────

    setupToggles() {
        document.querySelectorAll(".password-toggle").forEach((btn) => {
            btn.addEventListener("click", () => {
                const wrapper = btn.closest(".password-input-wrapper");
                const input = wrapper?.querySelector("input");
                if (!input) return;

                const isHidden = input.type === "password";
                input.type = isHidden ? "text" : "password";

                const img = btn.querySelector("img");
                if (img) {
                    img.src = isHidden
                        ? "static/icons/icons/eye-off.svg"
                        : "static/icons/icons/eye.svg";
                }
            });
        });
    },

    // ── Password strength meter ───────────────────────────────────────────────

    setupStrengthMeter() {
        const input = document.getElementById("regPassword");
        if (!input) return;

        input.addEventListener("input", (e) => {
            const { level, hints } = this._calcStrength(e.target.value);
            this._renderStrength(level, hints);
        });
    },

    _calcStrength(password) {
        if (!password) return { level: "empty", hints: [] };

        const hints = [];
        let score = 0;

        if (password.length >= 8) score++;
        else hints.push("At least 8 characters");
        if (/[a-z]/.test(password) && /[A-Z]/.test(password)) score++;
        else hints.push("Mix of upper and lower case");
        if (/[0-9]/.test(password)) score++;
        else hints.push("At least one number");
        if (/[^a-zA-Z0-9]/.test(password)) score++;
        else hints.push("At least one special character");

        const level = score <= 1 ? "weak" : score <= 3 ? "fair" : "strong";
        return { level, hints };
    },

    _renderStrength(level, hints = []) {
        const fill = document.getElementById("strengthFill");
        const label = document.getElementById("strengthText");
        const hintEl = document.getElementById("strengthHints");

        if (fill) {
            fill.className = level === "empty" ? "strength-fill" : `strength-fill ${level}`;
        }

        if (label) {
            const labels = { empty: "", weak: "Weak", fair: "Fair", strong: "Strong" };
            label.textContent = level === "empty" ? "" : `Password strength: ${labels[level]}`;
        }

        if (hintEl) {
            if (hints.length === 0 || level === "empty") {
                hintEl.innerHTML = "";
            } else {
                hintEl.innerHTML = hints
                    .map((h) => `<span class="strength-hint-item">✗ ${h}</span>`)
                    .join("");
            }
        }
    },

    // ── Avatar upload ─────────────────────────────────────────────────────────

    setupAvatarUpload(onLoad) {
        const input = document.getElementById("avatarInput");
        const preview = document.getElementById("avatarPreview");
        if (!input || !preview) return;

        preview.addEventListener("click", () => input.click());

        input.addEventListener("change", (e) => {
            const file = e.target.files[0];
            if (!file) return;

            if (!file.type.startsWith("image/")) {
                alert("Please select an image file.");
                return;
            }
            if (file.size > 5 * 1024 * 1024) {
                alert("Image must be smaller than 5 MB.");
                return;
            }

            const reader = new FileReader();
            reader.onload = (ev) => {
                const img = document.createElement("img");
                img.src = ev.target.result;
                preview.innerHTML = "";
                preview.appendChild(img);
                if (typeof onLoad === "function") onLoad(ev.target.result);
            };
            reader.onerror = () => alert("Failed to read the image file. Please try again.");
            reader.readAsDataURL(file);
        });
    },
};

export default AuthPassword;
