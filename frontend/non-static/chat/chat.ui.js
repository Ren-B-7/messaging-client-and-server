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
   * @param {'dm'|'groups'} type
   */
  updateHeader(conversation, type = 'dm') {
    const nameEl   = document.getElementById('chatName');
    const avatarEl = document.getElementById('chatAvatar');
    const dotEl    = document.getElementById('statusDot');
    const textEl   = document.getElementById('statusText');

    if (nameEl)   nameEl.textContent   = conversation.name;
    if (avatarEl) avatarEl.textContent = Utils.getInitials(conversation.name);

    if (type === 'groups') {
      // Groups have no individual online status — show member count if available.
      if (dotEl)  dotEl.className    = 'status-dot';
      if (textEl) textEl.textContent = conversation.memberCount
        ? `${conversation.memberCount} members`
        : 'Group';
      return;
    }

    // DM: show real online status.
    if (conversation.isOnline) {
      if (dotEl)  dotEl.className    = 'status-dot status-online';
      if (textEl) textEl.textContent = 'Online';
    } else {
      if (dotEl)  dotEl.className    = 'status-dot status-offline';
      if (textEl) textEl.textContent = 'Offline';
    }
  },

  // ─── Action buttons ───────────────────────────────────────────────────────

  /**
   * Wire voice-call, video-call, and attach-file buttons.
   *
   * Bug fixed: PlatformConfig has no hasFeature() method — it exposes
   * `capabilities` and `features` objects. All three buttons now show the
   * "Coming soon" message unconditionally (the feature doesn't exist yet
   * regardless of platform), which is better than silently falling through
   * to the "not supported" branch on every platform.
   */
  setupActionButtons() {
    document.getElementById('voiceCallBtn')?.addEventListener('click', () => {
      alert('Voice calls — Coming soon!');
    });

    document.getElementById('videoCallBtn')?.addEventListener('click', () => {
      alert('Video calls — Coming soon!');
    });

    document.getElementById('attachFileBtn')?.addEventListener('click', () => {
      alert('File attachments — Coming soon!');
    });
  },
};
