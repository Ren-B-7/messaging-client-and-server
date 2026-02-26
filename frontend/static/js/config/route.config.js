/**
 * Route Configuration
 * Application routing and navigation
 */

const RouteConfig = {
  // Route definitions
  routes: {
    login: "/",
    register: "/register",
    chat: "/chat",
    settings: "/settings",
    contacts: "/contacts",
    help: "/help",
    terms: "/terms",
    privacy: "/privacy",
  },

  // Auth required routes
  authRequired: ["/chat", "/settings", "/contacts"],

  // Public routes (no auth needed)
  publicRoutes: ["/", "/register", "/help", "/terms", "/privacy"],

  /**
   * Check if current route requires authentication
   */
  requiresAuth(path = window.location.pathname) {
    return this.authRequired.some((route) => path.startsWith(route));
  },

  /**
   * Navigate to route
   */
  navigate(route) {
    const path = this.routes[route] || route;
    window.location.href = path;
  },

  /**
   * Get current route name
   */
  getCurrentRoute() {
    const path = window.location.pathname;
    return (
      Object.entries(this.routes).find(([_, route]) => route === path)?.[0] ||
      "unknown"
    );
  },
};

// Make available globally
window.RouteConfig = RouteConfig;
