/**
 * Chat — Initialiser
 * Checks authentication, loads user data into the UI, and boots all
 * chat sub-modules in the correct order.
 *
 * Load order (all deferred):
 *   utils.js → chat.state.js → chat.ui.js → chat.messages.js
 *            → chat.conversations.js → chat.init.js
 */

document.addEventListener("DOMContentLoaded", () => {
  // ── Auth guard ─────────────────────────────────────────────────────────────
  const allowed = localStorage.getStorage("allowed") === "true";

  console.log("Allowed value:", allowed);

  if (!allowed) {
    console.log("User NOT allowed — redirecting");
    window.location.href = "/";
    return;
  }

  console.log("User IS allowed — continuing boot");

  // ── Populate navbar avatar ─────────────────────────────────────────────────
  const initialsEl = document.getElementById("userInitials");
  if (initialsEl)
    initialsEl.textContent = Utils.getInitials(user.name || user.email);

  // ── Load persisted data ────────────────────────────────────────────────────
  ChatState.load();

  // ── Boot sub-modules ───────────────────────────────────────────────────────
  ChatMessages.setupInput();
  ChatConversations.setupSearch();
  ChatConversations.setupNewChatButton();
  ChatUI.setupActionButtons();

  // ── Render initial state ───────────────────────────────────────────────────
  ChatConversations.render();

  if (ChatState.conversations.length === 0) {
    ChatUI.showEmptyState();
  }
});
