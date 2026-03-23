/**
 * Admin — Config Editor
 * Fetches /admin/api/config, renders a fully-generated editable form from the
 * returned JSON object, and PATCHes changes back field-by-field or as a whole.
 *
 * The schema that drives the UI is defined in CONFIG_SCHEMA below.  Each entry
 * maps a dot-path (matching the JSON key returned by the server) to a
 * descriptor that controls how the field is rendered and validated.
 *
 * Depends on: Utils, AdminState, AdminUI
 */

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------
// Each key is the dot-path into the config JSON (e.g. "server.bind").
// Descriptor fields:
//   label       — Human-readable field name
//   section     — Groups fields into panels (must match a key in CONFIG_SECTIONS)
//   type        — "text" | "number" | "boolean" | "password" | "array"
//   description — Shown as helper text beneath the input
//   min         — (number) minimum value
//   placeholder — Input placeholder
//   readOnly    — If true, rendered as a read-only badge (never sent in PATCH)
//   dangerous   — Adds a ⚠ warning badge; requires explicit save confirmation
// ---------------------------------------------------------------------------

const CONFIG_SCHEMA = {
  // ── Server ──────────────────────────────────────────────────────────────────
  "server.bind": {
    label: "Bind Address",
    section: "server",
    type: "text",
    placeholder: "0.0.0.0",
    description: "IP address the server listens on. Use 0.0.0.0 for all interfaces.",
    dangerous: true,
  },
  "server.port_client": {
    label: "Client Port",
    section: "server",
    type: "number",
    min: 1,
    placeholder: "1337",
    description: "Port for the user-facing HTTP server.",
    dangerous: true,
  },
  "server.port_admin": {
    label: "Admin Port",
    section: "server",
    type: "number",
    min: 1,
    placeholder: "1338",
    description: "Port for this admin panel.",
    dangerous: true,
  },
  "server.max_connections": {
    label: "Max Connections",
    section: "server",
    type: "number",
    min: 1,
    placeholder: "1000",
    description: "Hard cap on simultaneous TCP connections.",
  },

  // ── Auth ────────────────────────────────────────────────────────────────────
  "auth.token_expiry_minutes": {
    label: "Token Expiry (minutes)",
    section: "auth",
    type: "number",
    min: 1,
    placeholder: "60",
    description: "How long JWT sessions remain valid before the user must re-authenticate.",
  },
  "auth.email_required": {
    label: "Email Required",
    section: "auth",
    type: "boolean",
    description: "When enabled, users must supply a valid email address at registration.",
  },
  "auth.strict_ip_binding": {
    label: "Strict IP Binding",
    section: "auth",
    type: "boolean",
    description:
      "Reject requests whose IP doesn't match the session IP. Disable for mobile / VPN users.",
  },
  "auth.jwt_secret": {
    label: "JWT Secret",
    section: "auth",
    type: "password",
    placeholder: "••••••••",
    description:
      "HMAC key used to sign tokens. Prefer the JWT_SECRET env-var. Min 32 chars. Changing this invalidates all sessions.",
    dangerous: true,
  },
  "auth.cors_origins": {
    label: "CORS Origins",
    section: "auth",
    type: "array",
    placeholder: "https://example.com",
    description:
      "Allowed origins for cross-origin requests (release builds only). One entry per line.",
  },

  // ── Paths ───────────────────────────────────────────────────────────────────
  "paths.web_dir": {
    label: "Web Directory",
    section: "paths",
    type: "text",
    placeholder: "./web",
    description: "Filesystem path to the directory serving static files.",
    dangerous: true,
  },
  "paths.uploads_dir": {
    label: "Uploads Directory",
    section: "paths",
    type: "text",
    placeholder: "./uploads",
    description: "Directory where user-uploaded files are stored.",
    dangerous: true,
  },
  "paths.icons": {
    label: "Icons Path",
    section: "paths",
    type: "text",
    placeholder: "./static/icons",
    description: "Path to the bundled icon set.",
  },
  "paths.blocked_paths": {
    label: "Blocked Paths",
    section: "paths",
    type: "array",
    placeholder: "/admin/secret",
    description:
      "URL paths that the server refuses unconditionally. One entry per line.",
  },
};

// Section metadata — order + display properties
const CONFIG_SECTIONS = {
  server: { icon: "⚙", title: "Server", description: "Network binding and connection limits" },
  auth:   { icon: "🔑", title: "Auth & Security", description: "JWT, CORS, IP binding, and session settings" },
  paths:  { icon: "📁", title: "Paths", description: "Filesystem paths for web assets and uploads" },
};

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

const AdminConfig = {
  /** Raw config fetched from the server */
  _raw: null,
  /** Pending edits — dot-path → new value */
  _dirty: {},

  // ── Public ──────────────────────────────────────────────────────────────────

  async load() {
    const container = document.getElementById("config-container");
    if (!container) return;

    container.innerHTML = this._spinnerHtml();

    try {
      const res = await fetch("/admin/api/config");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      this._raw = await res.json();
      this._dirty = {};
      this._render(container);
      AdminUI.logAction("info", "Config loaded from server");
    } catch (e) {
      container.innerHTML = `
        <div class="empty-state">
          <span class="empty-icon">⚠</span>
          Failed to load config — ${Utils.escapeHtml(e.message)}
        </div>`;
      AdminUI.logAction("error", `Config load failed: ${e.message}`);
      console.error("[admin] config load:", e);
    }
  },

  async saveAll() {
    if (!Object.keys(this._dirty).length) {
      AdminUI.toast("No changes to save", "info");
      return;
    }

    const hasDangerous = Object.keys(this._dirty).some(
      (k) => CONFIG_SCHEMA[k]?.dangerous
    );

    if (hasDangerous) {
      const ok = confirm(
        "⚠ You are about to save one or more dangerous settings (marked ⚠).\n\n" +
        "Changing bind address, ports, or the JWT secret may disconnect active users " +
        "or require a server restart.\n\nContinue?"
      );
      if (!ok) return;
    }

    const patch = this._buildPatch();
    AdminUI.setLoading("config-save-btn", true);

    try {
      const res = await fetch("/admin/api/config", {
        method: "PATCH",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(patch),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();

      if (data.status === "success") {
        AdminUI.toast("Configuration saved", "success");
        AdminUI.logAction("info", `Config updated — ${Object.keys(this._dirty).join(", ")}`);
        this._dirty = {};
        this._updateDirtyBanner();
        // Reload to reflect any server-side normalisation
        await this.load();
      } else {
        AdminUI.toast(data.message || "Save failed", "error");
      }
    } catch (e) {
      AdminUI.toast("Save failed — see console", "error");
      AdminUI.logAction("error", `Config save error: ${e.message}`);
      console.error("[admin] config save:", e);
    }

    AdminUI.setLoading("config-save-btn", false);
  },

  discard() {
    if (!Object.keys(this._dirty).length) return;
    this._dirty = {};
    const container = document.getElementById("config-container");
    if (container) this._render(container);
    AdminUI.toast("Changes discarded", "info");
  },

  // ── Render ──────────────────────────────────────────────────────────────────

  _render(container) {
    if (!this._raw) return;

    // Group schema keys by section, preserving CONFIG_SECTIONS order
    const bySection = {};
    for (const sectionKey of Object.keys(CONFIG_SECTIONS)) {
      bySection[sectionKey] = [];
    }
    for (const [path, desc] of Object.entries(CONFIG_SCHEMA)) {
      if (bySection[desc.section]) bySection[desc.section].push(path);
    }

    const panels = Object.entries(CONFIG_SECTIONS)
      .map(([sectionKey, meta]) => {
        const paths = bySection[sectionKey] || [];
        if (!paths.length) return "";
        const rows = paths.map((p) => this._fieldHtml(p)).join("");
        return `
          <div class="panel config-panel">
            <div class="panel-head">
              <span class="panel-title">${meta.icon} ${Utils.escapeHtml(meta.title)}</span>
              <span class="panel-subtitle">${Utils.escapeHtml(meta.description)}</span>
            </div>
            <div class="config-fields">${rows}</div>
          </div>`;
      })
      .join("");

    container.innerHTML = `
      <div class="config-dirty-banner" id="config-dirty-banner" style="display:none">
        <span>⚠ You have unsaved changes.</span>
        <div class="config-dirty-actions">
          <button class="btn btn-secondary btn-sm" onclick="AdminConfig.discard()">Discard</button>
          <button class="btn btn-primary btn-sm" id="config-save-btn" onclick="AdminConfig.saveAll()">
            Save Changes
          </button>
        </div>
      </div>
      ${panels}
      <div class="config-footer">
        <button class="btn btn-secondary btn-sm" onclick="AdminConfig.discard()">Discard All</button>
        <button class="btn btn-primary" id="config-save-btn" onclick="AdminConfig.saveAll()">
          💾 Save All Changes
        </button>
      </div>`;

    // Attach change listeners
    container.querySelectorAll("[data-config-path]").forEach((el) => {
      const path = el.dataset.configPath;
      const desc = CONFIG_SCHEMA[path];
      if (!desc) return;

      const event = desc.type === "boolean" ? "change" : "input";
      el.addEventListener(event, () => {
        this._onFieldChange(path, el, desc);
      });
    });
  },

  _fieldHtml(path) {
    const desc = CONFIG_SCHEMA[path];
    if (!desc) return "";

    const value = this._getNestedValue(this._raw, path);
    const isDirty = path in this._dirty;

    const dangerBadge = desc.dangerous
      ? `<span class="badge badge-amber config-badge-danger" title="Dangerous setting — changes may require restart or disconnect users">⚠ dangerous</span>`
      : "";

    const input = this._inputHtml(path, desc, value);

    return `
      <div class="config-field ${isDirty ? "is-dirty" : ""}" id="config-field-${CSS.escape(path)}">
        <div class="config-field-label">
          <label class="form-label" for="config-input-${CSS.escape(path)}">
            ${Utils.escapeHtml(desc.label)}
          </label>
          ${dangerBadge}
        </div>
        ${input}
        ${desc.description ? `<div class="config-field-hint">${Utils.escapeHtml(desc.description)}</div>` : ""}
        <div class="config-field-dirty-marker" title="Unsaved change">● modified</div>
      </div>`;
  },

  _inputHtml(path, desc, value) {
    const id = `config-input-${CSS.escape(path)}`;
    const attr = `id="${id}" data-config-path="${Utils.escapeHtml(path)}"`;

    switch (desc.type) {
      case "boolean":
        return `
          <label class="config-toggle">
            <input type="checkbox" ${attr} class="config-checkbox"
              ${value ? "checked" : ""} />
            <span class="config-toggle-label">${value ? "Enabled" : "Disabled"}</span>
          </label>`;

      case "password":
        return `
          <div class="password-input-wrapper">
            <input type="password" ${attr} class="form-input"
              value="${Utils.escapeHtml(value ?? "")}"
              placeholder="${Utils.escapeHtml(desc.placeholder ?? "")}"
              autocomplete="new-password" />
            <button type="button" class="password-toggle"
              onclick="this.previousElementSibling.type = this.previousElementSibling.type === 'password' ? 'text' : 'password'"
              aria-label="Toggle visibility">
              <img src="static/icons/icons/eye.svg" alt="" width="20" height="20" />
            </button>
          </div>`;

      case "number":
        return `
          <input type="number" ${attr} class="form-input"
            value="${value ?? ""}"
            placeholder="${Utils.escapeHtml(desc.placeholder ?? "")}"
            min="${desc.min ?? ""}" />`;

      case "array": {
        const lines = Array.isArray(value) ? value.join("\n") : (value ?? "");
        return `
          <textarea ${attr} class="form-input config-textarea"
            rows="4"
            placeholder="${Utils.escapeHtml(desc.placeholder ?? "")}"
            spellcheck="false">${Utils.escapeHtml(lines)}</textarea>`;
      }

      default: // "text"
        return `
          <input type="text" ${attr} class="form-input"
            value="${Utils.escapeHtml(value ?? "")}"
            placeholder="${Utils.escapeHtml(desc.placeholder ?? "")}" />`;
    }
  },

  // ── Change tracking ──────────────────────────────────────────────────────────

  _onFieldChange(path, el, desc) {
    let value;
    if (desc.type === "boolean") {
      value = el.checked;
      // Update sibling label text
      const label = el.closest(".config-toggle")?.querySelector(".config-toggle-label");
      if (label) label.textContent = value ? "Enabled" : "Disabled";
    } else if (desc.type === "array") {
      value = el.value.split("\n").map((s) => s.trim()).filter(Boolean);
    } else if (desc.type === "number") {
      value = el.value === "" ? null : Number(el.value);
    } else {
      value = el.value;
    }

    const original = this._getNestedValue(this._raw, path);
    const changed = JSON.stringify(value) !== JSON.stringify(original);

    if (changed) {
      this._dirty[path] = value;
    } else {
      delete this._dirty[path];
    }

    // Toggle dirty class on the field wrapper
    const fieldEl = document.getElementById(`config-field-${CSS.escape(path)}`);
    if (fieldEl) fieldEl.classList.toggle("is-dirty", changed);

    this._updateDirtyBanner();
  },

  _updateDirtyBanner() {
    const count = Object.keys(this._dirty).length;
    const banner = document.getElementById("config-dirty-banner");
    if (banner) banner.style.display = count > 0 ? "flex" : "none";

    // Update the footer save button state
    document.querySelectorAll("#config-save-btn").forEach((btn) => {
      btn.disabled = count === 0;
      btn.textContent = count > 0 ? `💾 Save ${count} Change${count > 1 ? "s" : ""}` : "💾 Save All Changes";
    });
  },

  // ── Helpers ──────────────────────────────────────────────────────────────────

  /** Read a value from a nested object using a dot-path key. */
  _getNestedValue(obj, path) {
    return path.split(".").reduce((o, k) => (o != null ? o[k] : undefined), obj);
  },

  /**
   * Build the PATCH body — a nested object mirroring AppConfig,
   * containing only the changed fields.
   */
  _buildPatch() {
    const patch = {};
    for (const [path, value] of Object.entries(this._dirty)) {
      const parts = path.split(".");
      let cursor = patch;
      for (let i = 0; i < parts.length - 1; i++) {
        cursor[parts[i]] ??= {};
        cursor = cursor[parts[i]];
      }
      cursor[parts[parts.length - 1]] = value;
    }
    return patch;
  },

  _spinnerHtml() {
    return `
      <div class="empty-state">
        <span class="spinner"></span>
        Loading configuration…
      </div>`;
  },
};
