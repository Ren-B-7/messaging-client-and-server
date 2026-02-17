/**
 * Settings — Tab Navigation
 * Manages the sidebar nav links and settings tab panel switching.
 * Also handles direct navigation via URL hash (e.g. /settings#privacy).
 * Depends on: (none — pure DOM)
 */

const SettingsNav = {
  setup() {
    const navItems = document.querySelectorAll('.settings-nav-item');
    const tabs     = document.querySelectorAll('.settings-tab');

    navItems.forEach(item => {
      item.addEventListener('click', e => {
        e.preventDefault();
        const tabId = item.getAttribute('data-tab');

        navItems.forEach(n => n.classList.remove('active'));
        tabs.forEach(t => t.classList.remove('active'));

        item.classList.add('active');
        document.getElementById(tabId)?.classList.add('active');

        const hash = item.getAttribute('href');
        if (hash) window.location.hash = hash;
      });
    });

    // Honour a hash already in the URL on page load.
    const hash = window.location.hash.substring(1);
    if (hash) {
      document.querySelector(`[href="#${hash}"]`)?.click();
    }
  },
};
