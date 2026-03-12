/**
 * error.interceptor.js
 *
 * Intercepts fetch/XHR errors, unhandled JS exceptions, and unhandled promise
 * rejections, then redirects to /error with a structured JSON payload stored
 * in sessionStorage under "__error_payload__".
 *
 * The payload schema accepted by error.html:
 *
 *   {
 *     code:     number | string   — HTTP status or short error code
 *     title:    string            — Heading shown to the user
 *     subtitle: string            — Descriptive sentence
 *     hint:     string            — Secondary help text / next steps
 *     icon:     string            — Emoji icon (falls back to defaults per code)
 *     primary:  { label, href? }  — Primary CTA button (defaults to "Go Home → /")
 *     returnTo: string            — URL for the "Go Back" button
 *   }
 *
 * Usage — manual redirect:
 *
 *   ErrorInterceptor.redirect({
 *     code: 403,
 *     title: "Forbidden",
 *     subtitle: "You don't have access to this page.",
 *     primary: { label: "Go to Dashboard", href: "/chat" }
 *   });
 *
 * Usage — from a fetch response:
 *
 *   const res = await fetch("/api/data");
 *   if (!res.ok) ErrorInterceptor.fromResponse(res);
 *
 * Usage — from a raw JSON object (e.g. an API error body):
 *
 *   ErrorInterceptor.fromJSON({ status: 429, message: "Rate limited." });
 *
 * Drop-in notes:
 *   - Remove the <template id="__error_template__"> blocks from every page HTML.
 *   - Remove the inline <script defer src="static/js/utils/error.interceptor.js">
 *     from the bottom of each page body — keep it in <head> instead so
 *     global error listeners are registered as early as possible.
 *   - The CSS import (error.interceptor.css) is still used by error.html
 *     directly, so keep it in your stylesheet pipeline.
 */

(function (global) {
  "use strict";

  /** Path of the dedicated error page. Override if your app serves it elsewhere. */
  var ERROR_PAGE = "/error";

  /**
   * Write payload to sessionStorage and navigate to the error page.
   * sessionStorage is used so the payload never appears in the URL or
   * browser history, and it is consumed once on arrival.
   *
   * @param {Object} payload
   */
  function redirect(payload) {
    try {
      sessionStorage.setItem("__error_payload__", JSON.stringify(payload || {}));
    } catch (_) {
      // sessionStorage unavailable (e.g. private mode quota exceeded):
      // fall back to base64 query param so the page still works.
      try {
        var encoded = btoa(JSON.stringify(payload || {}));
        location.replace(ERROR_PAGE + "?e=" + encodeURIComponent(encoded));
        return;
      } catch (_2) { /* give up encoding, just navigate bare */ }
    }
    location.replace(ERROR_PAGE);
  }

  /**
   * Build a payload from a fetch/XHR Response object.
   * Attempts to read a JSON body for richer error details.
   *
   * @param {Response} response
   * @param {Object}   [extra]   — merge any extra payload fields
   */
  function fromResponse(response, extra) {
    var base = Object.assign({ code: response.status }, extra || {});

    // Try to pull structured data from a JSON error body
    var ct = (response.headers && response.headers.get("content-type")) || "";
    if (ct.indexOf("application/json") !== -1) {
      response
        .clone()
        .json()
        .then(function (body) {
          // Common API error shapes: { message, error, detail, hint }
          redirect(Object.assign({}, base, {
            subtitle: body.message || body.error || body.detail || base.subtitle,
            hint:     body.hint    || body.description             || base.hint,
            title:    body.title                                    || base.title,
          }));
        })
        .catch(function () {
          redirect(base);
        });
    } else {
      redirect(base);
    }
  }

  /**
   * Build a payload from a plain object — useful for API responses that have
   * already been parsed, or for constructing errors from non-HTTP sources.
   *
   * Recognised keys: status/code, message/error/subtitle, hint, title, icon,
   *                  primary, returnTo.
   *
   * @param {Object} obj
   * @param {Object} [extra]
   */
  function fromJSON(obj, extra) {
    obj = obj || {};
    redirect(Object.assign({
      code:     obj.status   || obj.code,
      title:    obj.title,
      subtitle: obj.message  || obj.error   || obj.subtitle || obj.detail,
      hint:     obj.hint     || obj.description,
      icon:     obj.icon,
      primary:  obj.primary,
      returnTo: obj.returnTo,
    }, extra || {}));
  }

  // ── Global error listeners ──────────────────────────────────────────────────
  //
  // These only fire for truly unhandled errors. Errors that your application
  // code handles explicitly (try/catch, .catch()) are unaffected.

  /**
   * Guard: only redirect if we are not already on the error page.
   * Prevents infinite redirect loops when error.html itself throws.
   */
  function isErrorPage() {
    return location.pathname === ERROR_PAGE ||
           location.pathname === ERROR_PAGE + ".html";
  }

  /**
   * Returns true for errors that originate from browser internals, DevTools,
   * extensions, or injected scripts — none of which are application faults.
   *
   * DevTools opens a separate JS context but some of its activity (source-map
   * fetch failures, extension content-scripts, HMR websocket noise) can
   * surface as unhandled errors or rejections in the page context.
   *
   * Patterns covered:
   *   - No filename / lineno 0            → injected or synthetic error
   *   - chrome-extension:// / moz-extension:// filename → browser extension
   *   - devtools://                        → DevTools internal script
   *   - <anonymous> with no useful stack   → eval / DevTools console snippet
   *   - "ResizeObserver loop" warning      → benign browser-level warning
   *   - NetworkError on *.map requests     → source map fetch (DevTools opens map files)
   *   - "Cannot read properties of undefined (reading 'hot')" → HMR noise
   */
  function isDevToolsNoise(event) {
    var msg      = (event.message || "").toLowerCase();
    var filename = (event.filename || "");
    var reason   = event.reason;
    var reasonMsg = reason instanceof Error
      ? (reason.message || "").toLowerCase()
      : (typeof reason === "string" ? reason.toLowerCase() : "");

    // Extension scripts
    if (/^(chrome|moz|safari)-extension:\/\//.test(filename)) return true;
    // DevTools internal
    if (filename.startsWith("devtools://")) return true;
    // Source map fetch failures triggered when DevTools is open
    if (filename.endsWith(".map") || reasonMsg.includes(".map")) return true;
    // ResizeObserver loop limit — harmless browser internals
    if (msg.includes("resizeobserver loop")) return true;
    if (reasonMsg.includes("resizeobserver loop")) return true;
    // HMR / webpack hot reload noise
    if (msg.includes("hot") && msg.includes("undefined")) return true;
    // Errors with no filename at all and no line number are almost always
    // injected by DevTools or extensions (not real app errors)
    if (!filename && event.lineno === 0 && event.colno === 0) return true;

    return false;
  }

  // Unhandled synchronous JS errors
  global.addEventListener("error", function (event) {
    if (isErrorPage()) return;
    if (isDevToolsNoise(event)) return;

    // Ignore cross-origin script errors (no useful info available)
    if (!event.message || event.message === "Script error.") return;

    redirect({
      code:     "JS_ERROR",
      title:    "Unexpected Error",
      subtitle: "An unhandled error occurred in the application.",
      hint:     event.message || "Please refresh and try again.",
    });
  });

  // Unhandled promise rejections
  global.addEventListener("unhandledrejection", function (event) {
    if (isErrorPage()) return;
    if (isDevToolsNoise(event)) return;

    var reason = event.reason;

    // AbortError is thrown when fetch() is cancelled (e.g. navigating away,
    // or DevTools network throttling). Never redirect for these.
    if (reason && reason.name === "AbortError") return;

    // Ignore failed fetches for source maps — DevTools triggers these silently
    if (reason instanceof TypeError) {
      var tmsg = (reason.message || "").toLowerCase();
      if (tmsg.includes("fetch") && tmsg.includes(".map")) return;
      // "Failed to fetch" with no further context is nearly always a benign
      // network hiccup or a DevTools-initiated request — skip it.
      if (tmsg === "failed to fetch" || tmsg === "networkerror when attempting to fetch resource.") return;
    }

    var hint = (reason instanceof Error)
      ? reason.message
      : (typeof reason === "string" ? reason : "Please refresh and try again.");

    redirect({
      code:     "UNHANDLED_REJECTION",
      title:    "Unexpected Error",
      subtitle: "An unhandled promise rejection occurred.",
      hint:     hint,
    });
  });

  // ── Public API ──────────────────────────────────────────────────────────────

  global.ErrorInterceptor = {
    redirect:      redirect,
    fromResponse:  fromResponse,
    fromJSON:      fromJSON,
  };

}(window));
