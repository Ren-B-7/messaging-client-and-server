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

  // ── Fetch current user from server ─────────────────────────────────────────
  // localStorage may be empty (e.g. after a hard refresh or on a new device),
  // so always resolve identity from the API using the session cookie.
  try {
    const res = await fetch('/api/profile');
    if (res.ok) {
      const data = await res.json();
      const profile = data.data ?? data;
      ChatState.currentUser = {
        id:       Number(profile.user_id),   // API returns user_id not id
        username: profile.username ?? '',
        email:    profile.email ?? '',
        isAdmin:  profile.is_admin ?? false,
      };
      // Persist so other pages can read it without another API call.
      Utils.setStorage('user', ChatState.currentUser);
      console.info('[init] Current user:', ChatState.currentUser);
    } else {
      console.warn('[init] Could not fetch profile, sent messages may render incorrectly');
    }
  } catch (e) {
    console.error('[init] Profile fetch failed:', e);
  }

  // ── Update avatar initials now that we have a username ─────────────────────
  const initialsEl = document.getElementById('userInitials');
  if (initialsEl && ChatState.currentUser) {
    initialsEl.textContent = Utils.getInitials(ChatState.currentUser.username);
  }

  // ── Load persisted state ───────────────────────────────────────────────────
  ChatState.load();

  // ── Boot sub-modules ───────────────────────────────────────────────────────
  ChatMessages.setupInput();
  ChatConversations.setupTabs();
  ChatConversations.setupSearch();
  ChatConversations.setupNewButtons();
  ChatUI.setupActionButtons();

  // ── Fetch fresh data from API on every page load ───────────────────────────
  await ChatConversations.refresh();

  // ── Restore previously open conversation (if any) ─────────────────────────
  if (ChatState.currentConversation) {
    ChatConversations.open(
      ChatState.currentConversation.id,
      ChatState.currentConversationType,
    );
  }

  // ── Clear transient data when the user leaves ──────────────────────────────
  window.addEventListener('beforeunload', () => {
    // Clear transient chat data — always re-fetched from the server on next load.
    // Keep 'user' so other pages can read identity without an extra API call.
    Utils.removeStorage('messages');
    Utils.removeStorage('conversations');
    Utils.removeStorage('groups');
  });
});
