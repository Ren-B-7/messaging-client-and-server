/**
 * Settings — Profile Form
 * Populates the profile tab with user data, detects unsaved changes,
 * handles save / cancel, and wires the avatar edit button.
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

    // Header display
    const profileName   = document.getElementById('profileName');
    const profileEmail  = document.getElementById('profileEmail');
    const profileAvatar = document.getElementById('profileAvatar');
    if (profileName)   profileName.textContent   = fullName || 'User';
    if (profileEmail)  profileEmail.textContent  = user.email || '';
    if (profileAvatar) profileAvatar.textContent = Utils.getInitials(fullName);

    // Form fields
    const firstName = document.getElementById('firstName');
    const lastName  = document.getElementById('lastName');
    const username  = document.getElementById('username');
    if (firstName) firstName.value = names[0]                   || '';
    if (lastName)  lastName.value  = names.slice(1).join(' ')   || '';
    if (username)  username.value  = user.username              || '';

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
    });

    form.addEventListener('submit', e => {
      e.preventDefault();
      this._save(saveBtn);
    });

    document.getElementById('avatarEditBtn')?.addEventListener('click', () => {
      alert('Avatar upload feature — Coming soon!');
    });
  },

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
      alert('First name is required');
      return;
    }

    const user      = Utils.getStorage('user') || {};
    user.name       = `${firstName} ${lastName}`.trim();
    user.username   = username;
    Utils.setStorage('user', user);

    // Refresh header display
    const profileName   = document.getElementById('profileName');
    const profileAvatar = document.getElementById('profileAvatar');
    const userInitials  = document.getElementById('userInitials');
    if (profileName)   profileName.textContent   = user.name;
    if (profileAvatar) profileAvatar.textContent = Utils.getInitials(user.name);
    if (userInitials)  userInitials.textContent  = Utils.getInitials(user.name);

    this._original = { firstName, lastName, username };

    alert('Profile updated successfully!');
    if (saveBtn) saveBtn.disabled = true;
  },
};
