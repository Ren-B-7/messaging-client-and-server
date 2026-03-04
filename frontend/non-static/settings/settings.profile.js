/**
 * Settings — Profile Form
 * Populates the profile tab with user data, detects unsaved changes,
 * handles save / cancel, and wires the avatar edit button.
 * Save POSTs to /api/user/profile as JSON.
 * Depends on: Utils
 */

const SettingsProfile = {
  _original: {},

  /**
   * Populate form fields from stored user data.
   * @param {object} user
   */
  load(user) {
    const fullName = user.name || '';
    const names    = fullName.split(' ');

    const profileName   = document.getElementById('profileName');
    const profileEmail  = document.getElementById('profileEmail');
    const profileAvatar = document.getElementById('profileAvatar');
    if (profileName)   profileName.textContent   = fullName || 'User';
    if (profileEmail)  profileEmail.textContent  = user.email || '';
    if (profileAvatar) profileAvatar.textContent = Utils.getInitials(fullName);

    const firstName = document.getElementById('firstName');
    const lastName  = document.getElementById('lastName');
    const username  = document.getElementById('username');
    if (firstName) firstName.value = names[0]                 || '';
    if (lastName)  lastName.value  = names.slice(1).join(' ') || '';
    if (username)  username.value  = user.username            || '';

    this._original = {
      firstName: names[0]                 || '',
      lastName:  names.slice(1).join(' ') || '',
      username:  user.username            || '',
    };
  },

  /** Attach form submit, cancel, change-detection, and avatar edit listeners. */
  setup() {
    const form      = document.getElementById('profileForm');
    const saveBtn   = document.getElementById('saveProfileBtn');
    const cancelBtn = document.getElementById('cancelProfileBtn');
    if (!form) return;

    form.querySelectorAll('input').forEach(input => {
      input.addEventListener('input', () => this._checkChanges(saveBtn));
    });

    cancelBtn?.addEventListener('click', () => {
      this._reset();
      if (saveBtn) saveBtn.disabled = true;
      this._feedback('', '');
    });

    form.addEventListener('submit', e => {
      e.preventDefault();
      this._save(saveBtn);
    });

    document.getElementById('avatarEditBtn')?.addEventListener('click', () => {
      this._feedback('Avatar upload is not yet available.', 'error');
    });
  },

  // ── Private ───────────────────────────────────────────────────────────────

  _checkChanges(saveBtn) {
    const changed =
      (document.getElementById('firstName')?.value || '') !== this._original.firstName ||
      (document.getElementById('lastName')?.value  || '') !== this._original.lastName  ||
      (document.getElementById('username')?.value  || '') !== this._original.username;

    if (saveBtn) saveBtn.disabled = !changed;
  },

  _reset() {
    const firstName = document.getElementById('firstName');
    const lastName  = document.getElementById('lastName');
    const username  = document.getElementById('username');
    if (firstName) firstName.value = this._original.firstName;
    if (lastName)  lastName.value  = this._original.lastName;
    if (username)  username.value  = this._original.username;
  },

  async _save(saveBtn) {
    const firstName = document.getElementById('firstName')?.value.trim();
    const lastName  = document.getElementById('lastName')?.value.trim()  || '';
    const username  = document.getElementById('username')?.value.trim()  || '';

    if (!firstName) {
      this._feedback('First name is required.', 'error');
      return;
    }

    this._setLoading(saveBtn, true);

    try {
      const res = await fetch('/api/user/profile', {
        method:  'PUT',
        headers: { 'content-type': 'application/json' },
        body:    JSON.stringify({ first_name: firstName, last_name: lastName, username }),
      });

      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();

      if (data.status === 'success') {
        const fullName = `${firstName} ${lastName}`.trim();

        // Sync local storage so other pages stay consistent.
        const user    = Utils.getStorage('user') || {};
        user.name     = fullName;
        user.username = username;
        Utils.setStorage('user', user);

        // Refresh header display.
        const profileName   = document.getElementById('profileName');
        const profileAvatar = document.getElementById('profileAvatar');
        const userInitials  = document.getElementById('userInitials');
        if (profileName)   profileName.textContent   = fullName;
        if (profileAvatar) profileAvatar.textContent = Utils.getInitials(fullName);
        if (userInitials)  userInitials.textContent  = Utils.getInitials(fullName);

        this._original = { firstName, lastName, username };
        if (saveBtn) saveBtn.disabled = true;
        this._feedback('Profile updated successfully.', 'success');
      } else {
        this._feedback(data.message || 'Failed to update profile.', 'error');
      }
    } catch (e) {
      this._feedback('Request failed — check your connection.', 'error');
      console.error('[settings] saveProfile:', e);
    }

    this._setLoading(saveBtn, false);
  },

  // ── Helpers ───────────────────────────────────────────────────────────────

  _feedback(message, type) {
    const el = document.getElementById('profile-feedback');
    if (!el) return;
    el.textContent   = message;
    el.className     = `form-feedback ${type}`;
    el.style.display = message ? 'block' : 'none';
    if (type === 'success') setTimeout(() => { el.style.display = 'none'; }, 4000);
  },

  _setLoading(btn, loading) {
    if (!btn) return;
    btn.disabled = loading;
    if (loading) {
      btn._html     = btn.innerHTML;
      btn.innerHTML = 'Saving…';
    } else if (btn._html) {
      btn.innerHTML = btn._html;
      delete btn._html;
    }
  },
};
