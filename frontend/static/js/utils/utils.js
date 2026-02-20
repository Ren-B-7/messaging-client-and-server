/**
 * Shared Utilities
 * Common functions used across the application
 */

const Utils = {
  /**
   * Format time to HH:MM format
   * @param {Date} date - The date to format
   * @returns {string} Formatted time string
   */
  formatTime(date = new Date()) {
    const hours = String(date.getHours()).padStart(2, "0");
    const minutes = String(date.getMinutes()).padStart(2, "0");
    return `${hours}:${minutes}`;
  },

  /**
   * Format relative time (e.g., "5m ago")
   * @param {Date} date - The date to format
   * @returns {string} Relative time string
   */
  formatRelativeTime(date) {
    const now = new Date();
    const seconds = Math.floor((now - date) / 1000);

    if (seconds < 60) return "Just now";

    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m`;

    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h`;

    const days = Math.floor(hours / 24);
    if (days < 7) return `${days}d`;

    return date.toLocaleDateString();
  },

  /**
   * Debounce function to limit function calls
   * @param {Function} func - Function to debounce
   * @param {number} delay - Delay in milliseconds
   * @returns {Function} Debounced function
   */
  debounce(func, delay) {
    let timeoutId;
    return function (...args) {
      clearTimeout(timeoutId);
      timeoutId = setTimeout(() => func.apply(this, args), delay);
    };
  },

  /**
   * Simple HTML escape for security
   * @param {string} text - Text to escape
   * @returns {string} Escaped text
   */
  escapeHtml(text) {
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  },

  /**
   * Get initials from name
   * @param {string} name - Full name
   * @returns {string} Initials (max 2 characters)
   */
  getInitials(name) {
    if (!name) return "?";
    return name
      .split(" ")
      .map((n) => n[0])
      .join("")
      .toUpperCase()
      .slice(0, 2);
  },

  /**
   * Check if user is online
   * @returns {boolean} Online status
   */
  isOnline() {
    return navigator.onLine;
  },

  /**
   * Generate unique ID
   * @returns {string} Unique ID
   */
  generateId() {
    return `${Date.now()}-${Math.random().toString(36).substring(2, 11)}`;
  },

  /**
   * Store data in localStorage with error handling
   * @param {string} key - Storage key
   * @param {any} value - Value to store
   */
  setStorage(key, value) {
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch (error) {
      console.error("Error saving to localStorage:", error);
    }
  },

  /**
   * Get data from localStorage with error handling
   * @param {string} key - Storage key
   * @returns {any} Stored value or null
   */
  getStorage(key) {
    try {
      const item = localStorage.getItem(key);
      return item ? JSON.parse(item) : null;
    } catch (error) {
      console.error("Error reading from localStorage:", error);
      return null;
    }
  },

  /**
   * Remove data from localStorage
   * @param {string} key - Storage key
   */
  removeStorage(key) {
    try {
      localStorage.removeItem(key);
    } catch (error) {
      console.error("Error removing from localStorage:", error);
    }
  },

  /**
   * Show a notification (if supported)
   * @param {string} title - Notification title
   * @param {object} options - Notification options
   */
  async showNotification(title, options = {}) {
    if (!("Notification" in window)) {
      console.log("This browser does not support notifications");
      return;
    }

    if (Notification.permission === "granted") {
      new Notification(title, options);
    } else if (Notification.permission !== "denied") {
      const permission = await Notification.requestPermission();
      if (permission === "granted") {
        new Notification(title, options);
      }
    }
  },

  /**
   * Copy text to clipboard
   * @param {string} text - Text to copy
   * @returns {Promise<boolean>} Success status
   */
  async copyToClipboard(text) {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch (error) {
      console.error("Failed to copy:", error);
      return false;
    }
  },

  /**
   * Validate email address format
   * @param {string} email - Email to validate
   * @returns {boolean} Whether the email is valid
   */
  isValidEmail(email) {
    return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);
  },

  /**
   * Format file size to human readable format
   * @param {number} bytes - File size in bytes
   * @returns {string} Formatted file size
   */
  formatFileSize(bytes) {
    if (bytes === 0) return "0 Bytes";
    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round((bytes / Math.pow(k, i)) * 100) / 100 + " " + sizes[i];
  },
};

// Export for use in modules
if (typeof module !== "undefined" && module.exports) {
  module.exports = Utils;
} else {
  window.Utils = Utils;
}

