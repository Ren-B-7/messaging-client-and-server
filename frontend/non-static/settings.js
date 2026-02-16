/**
 * Settings Module
 * Handles application settings and preferences
 */

const SettingsModule = {
  originalFormData: {},

  /**
   * Initialize settings module
   */
  init() {
    this.checkAuthentication();
    this.loadUserData();
    this.setupTabNavigation();
    this.setupProfileForm();
    this.setupThemeSelection();
    this.setupToggleSwitches();
    this.setupAccountActions();
  },

  /**
   * Check if user is authenticated
   */
  checkAuthentication() {
    const user = Utils.getStorage('user');
    if (!user || !user.loggedIn) {
      window.location.href = '/';
      return;
    }
  },

  /**
   * Load user data and populate forms
   */
  loadUserData() {
    const user = Utils.getStorage('user');
    if (!user) return;

    // Update profile section
    const fullName = user.name || '';
    const names = fullName.split(' ');
    
    document.getElementById('profileName').textContent = fullName || 'User';
    document.getElementById('profileEmail').textContent = user.email || '';
    document.getElementById('profileAvatar').textContent = Utils.getInitials(fullName);
    document.getElementById('userInitials').textContent = Utils.getInitials(fullName);

    // Populate form fields
    if (names.length > 0) {
      document.getElementById('firstName').value = names[0] || '';
      document.getElementById('lastName').value = names.slice(1).join(' ') || '';
    }
    document.getElementById('username').value = user.username || '';
    document.getElementById('accountEmail').value = user.email || '';

    // Store original data for change detection
    this.originalFormData = {
      firstName: names[0] || '',
      lastName: names.slice(1).join(' ') || '',
      username: user.username || '',
    };

    // Set current theme
    const currentTheme = themeManager.getTheme();
    const themeRadio = document.getElementById(`${currentTheme}Theme`);
    if (themeRadio) {
      themeRadio.checked = true;
    }

    // Load preferences
    this.loadPreferences();
  },

  /**
   * Load user preferences from storage
   */
  loadPreferences() {
    const prefs = Utils.getStorage('preferences') || {};

    // Set notification preferences
    const pushNotifications = document.getElementById('pushNotifications');
    if (pushNotifications) {
      pushNotifications.checked = prefs.pushNotifications !== false;
    }

    const notificationSound = document.getElementById('notificationSound');
    if (notificationSound) {
      notificationSound.checked = prefs.notificationSound !== false;
    }

    // Set privacy preferences
    const showLastSeen = document.getElementById('showLastSeen');
    if (showLastSeen) {
      showLastSeen.checked = prefs.showLastSeen !== false;
    }

    const showProfilePhoto = document.getElementById('showProfilePhoto');
    if (showProfilePhoto) {
      showProfilePhoto.checked = prefs.showProfilePhoto !== false;
    }
  },

  /**
   * Setup tab navigation
   */
  setupTabNavigation() {
    const navItems = document.querySelectorAll('.settings-nav-item');
    const tabs = document.querySelectorAll('.settings-tab');

    navItems.forEach((item) => {
      item.addEventListener('click', (e) => {
        e.preventDefault();
        const tabId = item.getAttribute('data-tab');

        // Update active states
        navItems.forEach((nav) => nav.classList.remove('active'));
        tabs.forEach((tab) => tab.classList.remove('active'));

        item.classList.add('active');
        const targetTab = document.getElementById(tabId);
        if (targetTab) {
          targetTab.classList.add('active');
        }

        // Update URL hash
        const href = item.getAttribute('href');
        if (href) {
          window.location.hash = href;
        }
      });
    });

    // Handle direct navigation via hash
    const hash = window.location.hash.substring(1);
    if (hash) {
      const targetNav = document.querySelector(`[href="#${hash}"]`);
      if (targetNav) {
        targetNav.click();
      }
    }
  },

  /**
   * Setup profile form
   */
  setupProfileForm() {
    const form = document.getElementById('profileForm');
    if (!form) return;

    const inputs = form.querySelectorAll('input');
    const saveBtn = document.getElementById('saveProfileBtn');
    const cancelBtn = document.getElementById('cancelProfileBtn');

    // Detect changes
    inputs.forEach((input) => {
      input.addEventListener('input', () => {
        this.checkFormChanges(saveBtn);
      });
    });

    // Cancel button
    cancelBtn?.addEventListener('click', () => {
      this.resetProfileForm();
      if (saveBtn) saveBtn.disabled = true;
    });

    // Save button
    form.addEventListener('submit', (e) => {
      e.preventDefault();
      this.saveProfile();
    });

    // Avatar edit
    document.getElementById('avatarEditBtn')?.addEventListener('click', () => {
      alert('Avatar upload feature - Coming soon!');
    });
  },

  /**
   * Check if form has changes
   */
  checkFormChanges(saveBtn) {
    const firstName = document.getElementById('firstName')?.value || '';
    const lastName = document.getElementById('lastName')?.value || '';
    const username = document.getElementById('username')?.value || '';

    const hasChanges =
      firstName !== this.originalFormData.firstName ||
      lastName !== this.originalFormData.lastName ||
      username !== this.originalFormData.username;

    if (saveBtn) {
      saveBtn.disabled = !hasChanges;
    }
  },

  /**
   * Reset profile form to original values
   */
  resetProfileForm() {
    document.getElementById('firstName').value = this.originalFormData.firstName;
    document.getElementById('lastName').value = this.originalFormData.lastName;
    document.getElementById('username').value = this.originalFormData.username;
  },

  /**
   * Save profile changes
   */
  async saveProfile() {
    const firstName = document.getElementById('firstName')?.value.trim();
    const lastName = document.getElementById('lastName')?.value.trim();
    const username = document.getElementById('username')?.value.trim();

    if (!firstName) {
      alert('First name is required');
      return;
    }

    // Get current user data
    const user = Utils.getStorage('user') || {};

    // Update user data
    user.name = `${firstName} ${lastName}`.trim();
    user.username = username;

    // Save to storage
    Utils.setStorage('user', user);

    // Update UI
    document.getElementById('profileName').textContent = user.name;
    document.getElementById('profileAvatar').textContent = Utils.getInitials(user.name);
    document.getElementById('userInitials').textContent = Utils.getInitials(user.name);

    // Update original data
    this.originalFormData = {
      firstName,
      lastName,
      username,
    };

    // Show success message
    alert('Profile updated successfully!');
    document.getElementById('saveProfileBtn').disabled = true;
  },

  /**
   * Setup theme selection
   */
  setupThemeSelection() {
    const themeInputs = document.querySelectorAll('input[name="theme"]');

    themeInputs.forEach((input) => {
      input.addEventListener('change', (e) => {
        if (e.target.checked) {
          themeManager.setTheme(e.target.value, ['base', 'chat', 'settings']);
        }
      });
    });
  },

  /**
   * Setup toggle switches
   */
  setupToggleSwitches() {
    // Push notifications
    document.getElementById('pushNotifications')?.addEventListener('change', (e) => {
      this.updatePreference('pushNotifications', e.target.checked);
      
      if (e.target.checked && window.PlatformConfig?.hasFeature('pushNotifications')) {
        // Request notification permission
        if ('Notification' in window && Notification.permission === 'default') {
          Notification.requestPermission();
        }
      }
    });

    // Notification sound
    document.getElementById('notificationSound')?.addEventListener('change', (e) => {
      this.updatePreference('notificationSound', e.target.checked);
    });

    // Last seen
    document.getElementById('showLastSeen')?.addEventListener('change', (e) => {
      this.updatePreference('showLastSeen', e.target.checked);
    });

    // Profile photo
    document.getElementById('showProfilePhoto')?.addEventListener('change', (e) => {
      this.updatePreference('showProfilePhoto', e.target.checked);
    });
  },

  /**
   * Update preference in storage
   */
  updatePreference(key, value) {
    const prefs = Utils.getStorage('preferences') || {};
    prefs[key] = value;
    Utils.setStorage('preferences', prefs);
  },

  /**
   * Setup account actions
   */
  setupAccountActions() {
    // Change password
    document.getElementById('changePasswordBtn')?.addEventListener('click', () => {
      const currentPassword = prompt('Enter current password:');
      if (!currentPassword) return;

      const newPassword = prompt('Enter new password:');
      if (!newPassword) return;

      if (newPassword.length < 8) {
        alert('Password must be at least 8 characters long');
        return;
      }

      const confirmPassword = prompt('Confirm new password:');
      if (newPassword !== confirmPassword) {
        alert('Passwords do not match');
        return;
      }

      // Simulate password change
      alert('Password changed successfully!');
    });

    // Delete account
    document.getElementById('deleteAccountBtn')?.addEventListener('click', () => {
      const confirmed = confirm(
        'Are you sure you want to delete your account? This action cannot be undone.'
      );

      if (!confirmed) return;

      const emailConfirm = prompt('Please type your email address to confirm:');
      const user = Utils.getStorage('user');

      if (emailConfirm !== user?.email) {
        alert('Email address does not match');
        return;
      }

      // Clear all data
      localStorage.clear();
      alert('Your account has been deleted.');
      window.location.href = '/';
    });
  },
};

// Initialize on DOM ready
document.addEventListener('DOMContentLoaded', () => {
  SettingsModule.init();
});