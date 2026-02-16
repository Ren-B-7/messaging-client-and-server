/**
 * Platform Configuration
 * Detects and exposes platform information
 */

const PlatformConfig = {
  // Platform detection
  platform: (() => {
    const userAgent = navigator.userAgent.toLowerCase();
    const platform = navigator.platform.toLowerCase();

    if (userAgent.includes("android")) return "android";
    if (
      userAgent.includes("iphone") ||
      userAgent.includes("ipad") ||
      userAgent.includes("ipod")
    )
      return "ios";
    if (userAgent.includes("windows")) return "windows";
    if (userAgent.includes("mac")) return "mac";
    if (userAgent.includes("linux")) return "linux";

    return "web";
  })(),

  // Capabilities
  capabilities: {
    touch: "ontouchstart" in window || navigator.maxTouchPoints > 0,
    webp: document.createElement("canvas").toDataURL("image/webp").indexOf("data:image/webp") === 0,
    localStorage: (() => {
      try {
        localStorage.setItem("test", "test");
        localStorage.removeItem("test");
        return true;
      } catch (e) {
        return false;
      }
    })(),
  },

  // Feature detection
  features: {
    pwa: window.matchMedia("(display-mode: standalone)").matches,
    standalone: window.navigator.standalone === true,
  },
};

// Make available globally
window.PlatformConfig = PlatformConfig;
