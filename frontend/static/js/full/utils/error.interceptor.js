/**
 * error.interceptor.js
 *
 * Intercepts fetch/XHR errors, unhandled JS exceptions, and unhandled promise
 * rejections, then redirects to /error.
 */

const ERROR_PAGE = "/error";

/**
 * Write payload to sessionStorage and navigate to the error page.
 * @param {Object} payload
 */
function redirect(payload) {
    try {
        sessionStorage.setItem("__error_payload__", JSON.stringify(payload || {}));
    } catch (_) {
        try {
            const encoded = btoa(JSON.stringify(payload || {}));
            location.replace(`${ERROR_PAGE}?e=${encodeURIComponent(encoded)}`);
            return;
        } catch (_2) {
            /* ignore */
        }
    }
    location.replace(ERROR_PAGE);
}

/**
 * Build a payload from a fetch/XHR Response object.
 * @param {Response} response
 * @param {Object} [extra]
 */
function fromResponse(response, extra) {
    const base = Object.assign({ code: response.status }, extra || {});
    const ct = response.headers?.get("content-type") || "";

    if (ct.includes("application/json")) {
        response
            .clone()
            .json()
            .then((body) => {
                redirect(
                    Object.assign({}, base, {
                        subtitle: body.message || body.error || body.detail || base.subtitle,
                        hint: body.hint || body.description || base.hint,
                        title: body.title || body.title,
                    })
                );
            })
            .catch(() => redirect(base));
    } else {
        redirect(base);
    }
}

/**
 * Build a payload from a plain object.
 * @param {Object} obj
 * @param {Object} [extra]
 */
function fromJSON(obj = {}, extra) {
    redirect(
        Object.assign(
            {
                code: obj.status || obj.code,
                title: obj.title,
                subtitle: obj.message || obj.error || obj.subtitle || obj.detail,
                hint: obj.hint || obj.description,
                icon: obj.icon,
                primary: obj.primary,
                returnTo: obj.returnTo,
            },
            extra || {}
        )
    );
}

// Global listeners
function isErrorPage() {
    return location.pathname === ERROR_PAGE || location.pathname === `${ERROR_PAGE}.html`;
}

function isDevToolsNoise(event) {
    const msg = (event.message || "").toLowerCase();
    const filename = event.filename || "";
    const reason = event.reason;
    const reasonMsg =
        reason instanceof Error
            ? (reason.message || "").toLowerCase()
            : typeof reason === "string"
              ? reason.toLowerCase()
              : "";

    if (/^(chrome|moz|safari)-extension:\/\//.test(filename)) return true;
    if (filename.startsWith("devtools://")) return true;
    if (filename.endsWith(".map") || reasonMsg.includes(".map")) return true;
    if (msg.includes("resizeobserver loop") || reasonMsg.includes("resizeobserver loop"))
        return true;
    if (msg.includes("hot") && msg.includes("undefined")) return true;
    if (!filename && event.lineno === 0 && event.colno === 0) return true;

    return false;
}

window.addEventListener("error", (event) => {
    if (isErrorPage() || isDevToolsNoise(event)) return;
    if (!event.message || event.message === "Script error.") return;

    redirect({
        code: "JS_ERROR",
        title: "Unexpected Error",
        subtitle: "An unhandled error occurred in the application.",
        hint: event.message || "Please refresh and try again.",
    });
});

window.addEventListener("unhandledrejection", (event) => {
    if (isErrorPage() || isDevToolsNoise(event)) return;

    const reason = event.reason;
    if (reason?.name === "AbortError") return;

    if (reason instanceof TypeError) {
        const tmsg = (reason.message || "").toLowerCase();
        if (tmsg.includes("fetch") && tmsg.includes(".map")) return;
        if (tmsg === "failed to fetch" || tmsg.includes("networkerror")) return;
    }

    const hint =
        reason instanceof Error
            ? reason.message
            : typeof reason === "string"
              ? reason
              : "Please refresh and try again.";

    redirect({
        code: "UNHANDLED_REJECTION",
        title: "Unexpected Error",
        subtitle: "An unhandled promise rejection occurred.",
        hint,
    });
});

export const ErrorInterceptor = { redirect, fromResponse, fromJSON };
window.ErrorInterceptor = ErrorInterceptor;
