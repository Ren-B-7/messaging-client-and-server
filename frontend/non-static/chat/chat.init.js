/**
 * Chat — Initialiser
 * Loads user data from storage, boots all chat sub-modules in the correct
 * order, fetches conversations from the API on first load, and renders
 * the initial state.
 *
 * Load order (all deferred):
 *   theme.manager.js → platform.config.js → utils.js
 *   → chat.state.js → chat.ui.js → chat.messages.js
 *   → chat.conversations.js → chat.init.js
 */

document.addEventListener('DOMContentLoaded', async () => {
  // ── Theme ──────────────────────────────────────────────────────────────────
  themeManager.init(['base', 'chat']);

  document.getElementById('themeToggle')?.addEventListener('click', () => {
    themeManager.toggle();
  });

  // ── User data ──────────────────────────────────────────────────────────────
  // Read from storage — never rely on a bare `user` global.
  const user = Utils.getStorage('user') || {};

  const initialsEl = document.getElementById('userInitials');
  if (initialsEl) initialsEl.textContent = Utils.getInitials(user.name || user.email || '?');

  // ── Load persisted state ───────────────────────────────────────────────────
  ChatState.load();

  // ── Boot sub-modules ───────────────────────────────────────────────────────
  ChatMessages.setupInput();
  ChatConversations.setupTabs();
  ChatConversations.setupSearch();
  ChatConversations.setupNewButtons();
  ChatUI.setupActionButtons();

  // ── Fetch fresh data from API on every page load ───────────────────────────
  // Always hits /api/messages on load so the DM list is current.
  // Groups are fetched lazily when the groups tab is first clicked.
  await ChatConversations.refresh();

  // ── Restore previously open conversation (if any) ─────────────────────────
  if (ChatState.currentConversation) {
    ChatConversations.open(
      ChatState.currentConversation.id,
      ChatState.currentConversationType,
    );
  }
});
