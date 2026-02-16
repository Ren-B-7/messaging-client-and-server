/**
 * Authentication Module
 * Handles login and registration functionality
 */

const AuthModule = {
  currentStep: 0,
  formData: {},

  /**
   * Initialize authentication module
   */
  init() {
    this.setupPasswordToggles();
    this.setupLoginForm();
    this.setupRegisterForm();
  },

  /**
   * Setup password visibility toggles
   */
  setupPasswordToggles() {
    const passwordToggles = document.querySelectorAll('.password-toggle');

    passwordToggles.forEach((toggle) => {
      toggle.addEventListener('click', () => {
        const wrapper = toggle.closest('.password-input-wrapper');
        const input = wrapper?.querySelector('input');

        if (!input) return;

        const isPassword = input.type === 'password';
        input.type = isPassword ? 'text' : 'password';

        // Update icon
        const img = toggle.querySelector('img');
        if (img) {
          if (isPassword) {
            img.src = '/static/icons/eye-off.svg';
          } else {
            img.src = '/static/icons/eye.svg';
          }
        }
      });
    });
  },

  /**
   * Setup login form
   */
  setupLoginForm() {
    const loginForm = document.getElementById('loginForm');
    if (!loginForm) return;

    loginForm.addEventListener('submit', async (e) => {
      e.preventDefault();

      const email = document.getElementById('email')?.value.trim();
      const password = document.getElementById('password')?.value;
      const remember = document.querySelector('input[name="remember"]')?.checked;

      // Clear previous errors
      this.clearErrors();

      // Validate
      if (!this.validateLogin(email, password)) return;

      // Simulate API call
      try {
        await this.performLogin(email, password, remember);
      } catch (error) {
        this.showError('email', 'Invalid credentials. Please try again.');
      }
    });
  },

  /**
   * Validate login form
   */
  validateLogin(email, password) {
    let isValid = true;

    if (!email) {
      this.showError('email', 'Email is required');
      isValid = false;
    } else if (!Utils.isValidEmail(email)) {
      this.showError('email', 'Please enter a valid email');
      isValid = false;
    }

    if (!password) {
      this.showError('password', 'Password is required');
      isValid = false;
    }

    return isValid;
  },

  /**
   * Perform login (simulated)
   */
  async performLogin(email, password, remember) {
    // Simulate API delay
    await new Promise((resolve) => setTimeout(resolve, 1000));

    // Store user data
    const userData = {
      email,
      name: email.split('@')[0],
      loggedIn: true,
      loginTime: Date.now(),
    };

    Utils.setStorage('user', userData);
    if (remember) {
      Utils.setStorage('rememberMe', true);
    }

    // Redirect to chat
    window.location.href = '/chat';
  },

  /**
   * Setup register form
   */
  setupRegisterForm() {
    const registerForm = document.getElementById('registerForm');
    if (!registerForm) return;

    // Step navigation
    this.setupStepNavigation();

    // Password strength indicator
    this.setupPasswordStrength();

    // Avatar upload
    this.setupAvatarUpload();

    // Form submission
    registerForm.addEventListener('submit', async (e) => {
      e.preventDefault();
      await this.handleRegistration();
    });
  },

  /**
   * Setup step navigation for registration
   */
  setupStepNavigation() {
    // Next buttons
    document.getElementById('nextBtn1')?.addEventListener('click', () => {
      if (this.validateStep1()) {
        this.goToStep(1);
      }
    });

    document.getElementById('nextBtn2')?.addEventListener('click', () => {
      if (this.validateStep2()) {
        this.updateReview();
        this.goToStep(2);
      }
    });

    // Previous buttons
    document.getElementById('prevBtn2')?.addEventListener('click', () => {
      this.goToStep(0);
    });

    document.getElementById('prevBtn3')?.addEventListener('click', () => {
      this.goToStep(1);
    });
  },

  /**
   * Navigate to specific step
   */
  goToStep(stepIndex) {
    const steps = document.querySelectorAll('.step');
    const formSteps = document.querySelectorAll('.form-step');

    // Update progress indicators
    steps.forEach((step, index) => {
      step.classList.remove('active', 'completed');
      if (index < stepIndex) {
        step.classList.add('completed');
      } else if (index === stepIndex) {
        step.classList.add('active');
      }
    });

    // Update form steps
    formSteps.forEach((formStep, index) => {
      formStep.classList.toggle('active', index === stepIndex);
    });

    this.currentStep = stepIndex;
  },

  /**
   * Validate step 1 (Account details)
   */
  validateStep1() {
    const email = document.getElementById('regEmail')?.value.trim();
    const password = document.getElementById('regPassword')?.value;
    const confirmPassword = document.getElementById('regConfirmPassword')?.value;

    this.clearErrors();
    let isValid = true;

    if (!email) {
      this.showError('regEmail', 'Email is required');
      isValid = false;
    } else if (!Utils.isValidEmail(email)) {
      this.showError('regEmail', 'Please enter a valid email');
      isValid = false;
    }

    if (!password) {
      this.showError('regPassword', 'Password is required');
      isValid = false;
    } else if (password.length < 8) {
      this.showError('regPassword', 'Password must be at least 8 characters');
      isValid = false;
    }

    if (password !== confirmPassword) {
      this.showError('confirmPassword', 'Passwords do not match');
      isValid = false;
    }

    if (isValid) {
      this.formData.email = email;
      this.formData.password = password;
    }

    return isValid;
  },

  /**
   * Validate step 2 (Profile info)
   */
  validateStep2() {
    const fullName = document.getElementById('fullName')?.value.trim();
    const username = document.getElementById('username')?.value.trim();

    this.clearErrors();
    let isValid = true;

    if (!fullName) {
      this.showError('fullName', 'Full name is required');
      isValid = false;
    }

    if (!username) {
      this.showError('username', 'Username is required');
      isValid = false;
    } else if (username.length < 3) {
      this.showError('username', 'Username must be at least 3 characters');
      isValid = false;
    }

    if (isValid) {
      this.formData.fullName = fullName;
      this.formData.username = username;
    }

    return isValid;
  },

  /**
   * Update review section
   */
  updateReview() {
    document.getElementById('reviewEmail').textContent = this.formData.email || '-';
    document.getElementById('reviewName').textContent = this.formData.fullName || '-';
    document.getElementById('reviewUsername').textContent = this.formData.username || '-';
  },

  /**
   * Handle registration
   */
  async handleRegistration() {
    const termsCheckbox = document.getElementById('termsCheckbox');
    
    if (!termsCheckbox?.checked) {
      alert('Please accept the Terms of Service and Privacy Policy');
      return;
    }

    try {
      // Simulate API call
      await new Promise((resolve) => setTimeout(resolve, 1500));

      // Store user data
      const userData = {
        email: this.formData.email,
        name: this.formData.fullName,
        username: this.formData.username,
        loggedIn: true,
        registeredAt: Date.now(),
      };

      Utils.setStorage('user', userData);

      // Show success and redirect
      alert('Registration successful! Welcome to Chat!');
      window.location.href = '/chat';
    } catch (error) {
      alert('Registration failed. Please try again.');
    }
  },

  /**
   * Setup password strength indicator
   */
  setupPasswordStrength() {
    const passwordInput = document.getElementById('regPassword');
    if (!passwordInput) return;

    passwordInput.addEventListener('input', (e) => {
      const strength = this.calculatePasswordStrength(e.target.value);
      this.updatePasswordStrengthUI(strength);
    });
  },

  /**
   * Calculate password strength
   */
  calculatePasswordStrength(password) {
    let strength = 0;

    if (password.length >= 8) strength++;
    if (/[a-z]/.test(password) && /[A-Z]/.test(password)) strength++;
    if (/[0-9]/.test(password)) strength++;
    if (/[^a-zA-Z0-9]/.test(password)) strength++;

    if (strength === 0 || strength === 1) return 'weak';
    if (strength === 2 || strength === 3) return 'fair';
    return 'strong';
  },

  /**
   * Update password strength UI
   */
  updatePasswordStrengthUI(strength) {
    const fill = document.getElementById('strengthFill');
    const text = document.getElementById('strengthText');

    if (!fill || !text) return;

    fill.className = `strength-fill ${strength}`;
    
    const labels = { 
      weak: 'Weak', 
      fair: 'Fair', 
      strong: 'Strong' 
    };
    text.textContent = `Password strength: ${labels[strength]}`;
  },

  /**
   * Setup avatar upload
   */
  setupAvatarUpload() {
    const avatarInput = document.getElementById('avatarInput');
    const avatarPreview = document.getElementById('avatarPreview');

    if (!avatarInput || !avatarPreview) return;

    avatarPreview.addEventListener('click', () => avatarInput.click());

    avatarInput.addEventListener('change', (e) => {
      const file = e.target.files[0];
      if (file) {
        const reader = new FileReader();
        reader.onload = (event) => {
          const img = document.createElement('img');
          img.src = event.target.result;
          avatarPreview.innerHTML = '';
          avatarPreview.appendChild(img);
          this.formData.avatar = event.target.result;
        };
        reader.readAsDataURL(file);
      }
    });
  },

  /**
   * Show error message
   */
  showError(fieldId, message) {
    const errorElement = document.getElementById(`${fieldId}Error`);
    const inputElement = document.getElementById(fieldId);

    if (errorElement) {
      errorElement.textContent = message;
      errorElement.style.display = 'block';
    }

    if (inputElement) {
      inputElement.classList.add('error');
      inputElement.addEventListener('input', () => {
        inputElement.classList.remove('error');
        if (errorElement) errorElement.style.display = 'none';
      }, { once: true });
    }
  },

  /**
   * Clear all error messages
   */
  clearErrors() {
    document.querySelectorAll('.form-error').forEach((el) => {
      el.textContent = '';
      el.style.display = 'none';
    });
    document.querySelectorAll('.form-input').forEach((el) => {
      el.classList.remove('error');
    });
  },
};

// Initialize on DOM ready
document.addEventListener('DOMContentLoaded', () => {
  AuthModule.init();
});