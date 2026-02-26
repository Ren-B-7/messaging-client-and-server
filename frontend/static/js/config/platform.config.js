/**
 * Platform Configuration
 * Detects and exposes device / browser capabilities.
 * Loaded on every page so any module can read window.PlatformConfig.
 */

const PlatformConfig = {

  /** Detected OS / platform string. */
  platform: (() => {
    const ua = navigator.userAgent.toLowerCase();
    if (ua.includes('android'))                                          return 'android';
    if (ua.includes('iphone') || ua.includes('ipad') || ua.includes('ipod')) return 'ios';
    if (ua.includes('windows'))                                          return 'windows';
    if (ua.includes('mac'))                                              return 'mac';
    if (ua.includes('linux'))                                            return 'linux';
    return 'web';
  })(),

  /** Browser capability flags. */
  capabilities: {
    touch: 'ontouchstart' in window || navigator.maxTouchPoints > 0,

    webp: document.createElement('canvas')
      .toDataURL('image/webp')
      .startsWith('data:image/webp'),

    localStorage: (() => {
      try {
        localStorage.setItem('_test', '1');
        localStorage.removeItem('_test');
        return true;
      } catch {
        return false;
      }
    })(),
  },

  /** PWA / standalone display mode flags. */
  features: {
    pwa:        window.matchMedia('(display-mode: standalone)').matches,
    standalone: window.navigator.standalone === true,
  },
};

window.PlatformConfig = PlatformConfig;
