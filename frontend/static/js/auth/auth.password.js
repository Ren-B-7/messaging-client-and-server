/**
 * Auth — Password & Avatar Helpers
 * Password visibility toggle, strength meter, and avatar upload.
 * Depends on: (none — pure DOM)
 */

const AuthPassword = {
  /**
   * Attach click handlers to every .password-toggle button on the page.
   */
  setupToggles() {
    document.querySelectorAll('.password-toggle').forEach(btn => {
      btn.addEventListener('click', () => {
        const wrapper = btn.closest('.password-input-wrapper');
        const input   = wrapper?.querySelector('input');
        if (!input) return;

        const isHidden = input.type === 'password';
        input.type     = isHidden ? 'text' : 'password';

        const img = btn.querySelector('img');
        if (img) img.src = isHidden
          ? '/static/icons/icons/eye-off.svg'
          : '/static/icons/icons/eye.svg';
      });
    });
  },

  /**
   * Attach an input listener to #regPassword that drives the strength bar.
   */
  setupStrengthMeter() {
    const input = document.getElementById('regPassword');
    if (!input) return;
    input.addEventListener('input', e => {
      const level = this._calcStrength(e.target.value);
      this._renderStrength(level);
    });
  },

  /** @returns {'weak'|'fair'|'strong'} */
  _calcStrength(password) {
    let score = 0;
    if (password.length >= 8)                             score++;
    if (/[a-z]/.test(password) && /[A-Z]/.test(password)) score++;
    if (/[0-9]/.test(password))                           score++;
    if (/[^a-zA-Z0-9]/.test(password))                   score++;
    return score <= 1 ? 'weak' : score <= 3 ? 'fair' : 'strong';
  },

  _renderStrength(level) {
    const fill  = document.getElementById('strengthFill');
    const label = document.getElementById('strengthText');
    if (!fill || !label) return;
    fill.className = `strength-fill ${level}`;
    label.textContent = `Password strength: ${{ weak: 'Weak', fair: 'Fair', strong: 'Strong' }[level]}`;
  },

  /**
   * Wire up the avatar preview click → file input → FileReader preview.
   * Stores base64 result into formData.avatar via the supplied callback.
   *
   * @param {function(string): void} onLoad  Called with the base64 data URL.
   */
  setupAvatarUpload(onLoad) {
    const input   = document.getElementById('avatarInput');
    const preview = document.getElementById('avatarPreview');
    if (!input || !preview) return;

    preview.addEventListener('click', () => input.click());

    input.addEventListener('change', e => {
      const file = e.target.files[0];
      if (!file) return;

      const reader = new FileReader();
      reader.onload = ev => {
        const img = document.createElement('img');
        img.src   = ev.target.result;
        preview.innerHTML = '';
        preview.appendChild(img);
        if (typeof onLoad === 'function') onLoad(ev.target.result);
      };
      reader.readAsDataURL(file);
    });
  },
};
