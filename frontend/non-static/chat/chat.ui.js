/**
 * Chat — UI & Header
 * Manages the empty-state / active-chat view toggle, chat header rendering,
 * action button handlers, the Add Member flow, and the typing indicator.
 *
 * Depends on: Utils, ChatState, ChatConversations
 */

const ChatUI = {

  // ── Empty / active state ─────────────────────────────────────────────────

  showEmptyState() {
    const empty  = document.getElementById('emptyChatState');
    const active = document.getElementById('activeChatView');
    if (empty)  empty.style.display  = 'flex';
    if (active) active.style.display = 'none';
  },

  hideEmptyState() {
    const empty  = document.getElementById('emptyChatState');
    const active = document.getElementById('activeChatView');
    if (empty)  empty.style.display  = 'none';
    if (active) active.style.display = 'flex';
  },

  // ── Chat header ──────────────────────────────────────────────────────────

  /**
   * Populate the chat header with the active conversation's details.
   * @param {object}         conversation
   * @param {'dm'|'groups'}  type
   */
  updateHeader(conversation, type = 'dm') {
    const nameEl       = document.getElementById('chatName');
    const avatarEl     = document.getElementById('chatAvatar');
    const dotEl        = document.getElementById('statusDot');
    const textEl       = document.getElementById('statusText');
    const addMemberBtn = document.getElementById('addMemberBtn');

    if (nameEl)   nameEl.textContent   = Utils.escapeHtml(conversation.name);
    if (avatarEl) avatarEl.textContent = Utils.getInitials(conversation.name);

    if (type === 'groups') {
      if (dotEl)  dotEl.className    = 'status-dot';
      if (textEl) textEl.textContent = conversation.memberCount
        ? `${conversation.memberCount} members`
        : 'Group';
      if (addMemberBtn) addMemberBtn.style.display = '';
    } else {
      if (addMemberBtn) addMemberBtn.style.display = 'none';

      if (conversation.isOnline) {
        if (dotEl)  dotEl.className    = 'status-dot status-online';
        if (textEl) textEl.textContent = 'Online';
      } else {
        if (dotEl)  dotEl.className    = 'status-dot status-offline';
        if (textEl) textEl.textContent = 'Offline';
      }
    }
  },

  // ── Typing indicator ─────────────────────────────────────────────────────

  // Map of userId → display name (we only have the ID from SSE, so we show
  // a generic label unless the name resolves from state).
  _typingUsers: new Set(),

  /**
   * Show "Someone is typing…" below the message list.
   * Multiple concurrent typers are collapsed into a single banner.
   * @param {number} userId
   */
  showTyping(userId) {
    this._typingUsers.add(userId);
    this._renderTyping();
  },

  /**
   * Remove a user from the typing set and update the banner.
   * @param {number} userId
   */
  hideTyping(userId) {
    this._typingUsers.delete(userId);
    this._renderTyping();
  },

  _renderTyping() {
    let banner = document.getElementById('typingIndicator');

    if (this._typingUsers.size === 0) {
      if (banner) banner.remove();
      return;
    }

    if (!banner) {
      banner = document.createElement('div');
      banner.id        = 'typingIndicator';
      banner.className = 'typing-indicator';

      // Insert at the top of .chat-bottom-bar, above the input area
      const bottomBar = document.querySelector('.chat-bottom-bar');
      const inputArea = document.getElementById('messageInputArea');
      if (bottomBar && inputArea) {
        bottomBar.insertBefore(banner, inputArea);
      } else if (inputArea) {
        inputArea.parentNode.insertBefore(banner, inputArea);
      } else {
        document.getElementById('messagesContainer')?.appendChild(banner);
      }
    }

    const count = this._typingUsers.size;
    banner.innerHTML = `
      <span class="typing-dots">
        <span></span><span></span><span></span>
      </span>
      <span class="typing-text">
        ${count === 1 ? 'Someone is typing' : `${count} people are typing`}&hellip;
      </span>`;
  },

  // ── Action buttons ───────────────────────────────────────────────────────

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

    document.getElementById('addMemberBtn')?.addEventListener('click', () => {
      const conv = ChatState.currentConversation;
      if (!conv) return;
      const nameEl = document.getElementById('addMemberGroupName');
      if (nameEl) nameEl.textContent = conv.name;
      this._openAddMemberModal();
    });

    document.getElementById('addMemberSubmitBtn')?.addEventListener('click', () => {
      this._submitAddMember();
    });

    document.getElementById('addMemberInput')?.addEventListener('keydown', e => {
      if (e.key === 'Enter') this._submitAddMember();
    });

    document.querySelectorAll('[data-close-conv-modal="add-member-modal"]').forEach(btn => {
      btn.addEventListener('click', () => this._closeAddMemberModal());
    });

    document.getElementById('add-member-modal')?.addEventListener('click', e => {
      if (e.target === e.currentTarget) this._closeAddMemberModal();
    });

    document.addEventListener('keydown', e => {
      if (e.key === 'Escape') this._closeAddMemberModal();
    });
  },

  // ── Add Member modal ─────────────────────────────────────────────────────

  _openAddMemberModal() {
    const modal = document.getElementById('add-member-modal');
    if (!modal) return;
    modal.classList.add('open');
    const input = document.getElementById('addMemberInput');
    const err   = document.getElementById('addMemberError');
    if (input) { input.value = ''; input.focus(); }
    if (err)   err.textContent = '';
  },

  _closeAddMemberModal() {
    document.getElementById('add-member-modal')?.classList.remove('open');
  },

  async _submitAddMember() {
    const input   = document.getElementById('addMemberInput');
    const errorEl = document.getElementById('addMemberError');
    const name    = input?.value.trim();
    const conv    = ChatState.currentConversation;

    if (!name) {
      if (errorEl) errorEl.textContent = 'Please enter a name or username.';
      return;
    }
    if (!conv) {
      if (errorEl) errorEl.textContent = 'No active group selected.';
      return;
    }

    try {
      // Look up user_id by username first
      const lookupRes = await fetch(`/api/chats`, {
        method:  'POST',
        headers: { 'Content-Type': 'application/json' },
        body:    JSON.stringify({ username: name }),
      });

      // Then add to the group by user_id
      const userData  = await lookupRes.json();
      const targetId  = userData?.data?.chat_id ?? null;

      const addRes = await fetch(`/api/groups/${encodeURIComponent(conv.id)}/members`, {
        method:  'POST',
        headers: { 'Content-Type': 'application/json' },
        body:    JSON.stringify({ username: name }),
      });

      if (!addRes.ok) {
        const errData = await addRes.json();
        throw new Error(errData.message || 'Failed to add member');
      }

      const group = ChatState.groups.find(g => g.id === conv.id);
      if (group) {
        group.memberCount = (group.memberCount ?? 1) + 1;
        ChatState.currentConversation = group;
        ChatState.save();
        this.updateHeader(group, 'groups');
        ChatConversations.render();
      }

      this._closeAddMemberModal();
    } catch (err) {
      if (errorEl) errorEl.textContent = err.message || 'Failed to add member.';
    }
  },
};
