/**
 * Chat — Initialiser
 * Loads user data from storage, boots all chat sub-modules in the correct
 * order, fetches conversations from the API on first load, and renders
 * the initial state.
 *
 * Load order (all deferred):
 *   theme.manager.js → platform.config.js → utils.js
 *   → chat.state.js → chat.ui.js → chat.messages.js
 *   → chat.conversations.js → chat.sse.js → chat.init.js
 */

document.addEventListener("DOMContentLoaded", async () => {
  // ── Theme ──────────────────────────────────────────────────────────────────
  themeManager.init([ "base", "chat" ]);

  const chatThemeBtn = document.getElementById("themeToggle");
  themeManager.syncIcon(chatThemeBtn);
  chatThemeBtn?.addEventListener("click", () => {
    themeManager.toggle(); // syncIcon auto-updates via themechange event
  });

  // ── Fetch current user from server ─────────────────────────────────────────
  // Always resolve identity from the API — localStorage may be stale or empty.
  try {
    const res = await fetch("/api/profile");
    if (res.ok) {
      const data = await res.json();
      const profile = data.data ?? data;
      ChatState.currentUser = {
        id : Number(profile.user_id),
        username : profile.username ?? "",
        email : profile.email ?? "",
        isAdmin : profile.is_admin ?? false,
        avatarUrl : profile.avatar_url ?? null,
      };
      Utils.setStorage("user", ChatState.currentUser);
      console.info("[init] Current user:", ChatState.currentUser);
    } else {
      console.warn(
          "[init] Could not fetch profile — sent messages may render incorrectly",
      );
    }
  } catch (e) {
    console.error("[init] Profile fetch failed:", e);
  }

  // ── Update own avatar / initials in the sidebar header ────────────────────
  const initialsEl = document.getElementById("userInitials");
  const userAvatarEl = document.getElementById("userAvatarImg");

  if (ChatState.currentUser) {
    if (ChatState.currentUser.avatarUrl && userAvatarEl) {
      userAvatarEl.src = ChatState.currentUser.avatarUrl;
      userAvatarEl.style.display = "block";
      if (initialsEl)
        initialsEl.style.display = "none";
    } else if (initialsEl) {
      initialsEl.textContent =
          Utils.getInitials(ChatState.currentUser.username);
    }

    // Populate the avatar preview on the settings/profile section if present
    const avatarPreviewEl = document.getElementById("avatarPreview");
    if (avatarPreviewEl && ChatState.currentUser.avatarUrl) {
      avatarPreviewEl.src = ChatState.currentUser.avatarUrl;
      avatarPreviewEl.style.display = "block";
    }
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

  // ── Tear down SSE and clear transient data when the user leaves
  // ─────────────
  window.addEventListener("beforeunload", () => {
    // Cleanly close the SSE stream so the server reclaims the channel promptly.
    ChatSSE.disconnect();

    // Clear per-session data that is always re-fetched from the server on
    // reload.
    Utils.removeStorage("messages");
    Utils.removeStorage("conversations");
    Utils.removeStorage("groups");
  });
});
