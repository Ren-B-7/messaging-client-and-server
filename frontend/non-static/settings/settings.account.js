/**
 * Settings — Account Actions
 * Populates the account email field and handles the change-password
 * and delete-account flows.
 * Depends on: Utils
 */

const SettingsAccount = {
  /**
   * Populate the account email input.
   * @param {object} user
   */
  load(user) {
    const emailInput = document.getElementById('accountEmail');
    if (emailInput) emailInput.value = user.email || '';
  },

  /** Attach click handlers for change-password and delete-account buttons. */
  setup() {
    document.getElementById('changePasswordBtn')?.addEventListener('click', () => {
      this._changePassword();
    });

    document.getElementById('deleteAccountBtn')?.addEventListener('click', () => {
      this._deleteAccount();
    });
  },

  _changePassword() {
    const current  = prompt('Enter current password:');
    if (!current) return;

    const next = prompt('Enter new password:');
    if (!next) return;

    if (next.length < 8) {
      alert('Password must be at least 8 characters long');
      return;
    }

    const confirm = prompt('Confirm new password:');
    if (next !== confirm) {
      alert('Passwords do not match');
      return;
    }

    // TODO: replace with a real API call.
    alert('Password changed successfully!');
  },

  _deleteAccount() {
    const confirmed = confirm(
      'Are you sure you want to delete your account? This action cannot be undone.'
    );
    if (!confirmed) return;

    const user         = Utils.getStorage('user');
    const emailConfirm = prompt('Please type your email address to confirm:');

    if (emailConfirm !== user?.email) {
      alert('Email address does not match');
      return;
    }

    localStorage.clear();
    alert('Your account has been deleted.');
    window.location.href = '/';
  },
};
