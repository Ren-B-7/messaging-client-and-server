/**
 * Chat — UI & Header
 * Manages the empty-state / active-chat view toggle, chat header rendering,
 * and the call/attach action button handlers.
 * Depends on: Utils, ChatState
 */

const ChatUI = {
  // ─── Empty / active state ─────────────────────────────────────────────────

  showEmptyState() {
    document.getElementById('emptyChatState').style.display = 'flex';
    document.getElementById('activeChatView').style.display = 'none';
  },

  hideEmptyState() {
    document.getElementById('emptyChatState').style.display = 'none';
    document.getElementById('activeChatView').style.display = 'flex';
  },

  // ─── Chat header ──────────────────────────────────────────────────────────

  /**
   * Populate the chat header with the active conversation's details.
   * @param {object} conversation
   */
  updateHeader(conversation) {
    const nameEl   = document.getElementById('chatName');
    const avatarEl = document.getElementById('chatAvatar');
    const dotEl    = document.getElementById('statusDot');
    const textEl   = document.getElementById('statusText');

    if (nameEl)   nameEl.textContent   = conversation.name;
    if (avatarEl) avatarEl.textContent = conversation.avatar || Utils.getInitials(conversation.name);

    if (conversation.isOnline) {
      if (dotEl)  dotEl.className  = 'status-dot status-online';
      if (textEl) textEl.textContent = 'Online';
    } else {
      if (dotEl)  dotEl.className  = 'status-dot status-offline';
      if (textEl) textEl.textContent = 'Offline';
    }
  },

  // ─── Action buttons ───────────────────────────────────────────────────────

  /** Wire voice-call, video-call, and attach-file buttons. */
  setupActionButtons() {
    document.getElementById('voiceCallBtn')?.addEventListener('click', () => {
      window.PlatformConfig?.hasFeature('voiceCall')
        ? alert('Voice call feature — Coming soon!')
        : alert('Voice calls are not supported on this platform');
    });

    document.getElementById('videoCallBtn')?.addEventListener('click', () => {
      window.PlatformConfig?.hasFeature('videoCall')
        ? alert('Video call feature — Coming soon!')
        : alert('Video calls are not supported on this platform');
    });

    document.getElementById('attachFileBtn')?.addEventListener('click', () => {
      window.PlatformConfig?.hasFeature('fileUpload')
        ? alert('File attachment feature — Coming soon!')
        : alert('File uploads are not supported on this platform');
    });
  },
};
