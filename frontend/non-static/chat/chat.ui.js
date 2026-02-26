/**
 * Chat — UI & Header
 * Manages the empty-state / active-chat view toggle, chat header rendering,
 * and the action button handlers (voice call, video call, add member).
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
   * Also shows or hides the Add Member button depending on whether this is
   * a group conversation.
   *
   * @param {object}         conversation
   * @param {'dm'|'groups'}  type
   */
  updateHeader(conversation, type = 'dm') {
    const nameEl     = document.getElementById('chatName');
    const avatarEl   = document.getElementById('chatAvatar');
    const dotEl      = document.getElementById('statusDot');
    const textEl     = document.getElementById('statusText');
    const addMemberBtn = document.getElementById('addMemberBtn');

    if (nameEl)   nameEl.textContent   = Utils.escapeHtml(conversation.name);
    if (avatarEl) avatarEl.textContent = Utils.getInitials(conversation.name);

    if (type === 'groups') {
      // Groups: show member count, no online dot; reveal Add Member button.
      if (dotEl)  dotEl.className    = 'status-dot';
      if (textEl) textEl.textContent = conversation.memberCount
        ? `${conversation.memberCount} members`
        : 'Group';

      if (addMemberBtn) addMemberBtn.style.display = '';
    } else {
      // DM: show real online status; hide Add Member button.
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

    // Add Member button — opens the add-member modal for the current group.
    document.getElementById('addMemberBtn')?.addEventListener('click', () => {
      const conv = ChatState.currentConversation;
      if (!conv) return;

      // Populate the group name in the modal body.
      const nameEl = document.getElementById('addMemberGroupName');
      if (nameEl) nameEl.textContent = conv.name;

      this._openAddMemberModal();
    });

    // Modal wiring.
    document.getElementById('addMemberSubmitBtn')?.addEventListener('click', () => {
      this._submitAddMember();
    });

    document.getElementById('addMemberInput')?.addEventListener('keydown', e => {
      if (e.key === 'Enter') this._submitAddMember();
    });

    // Close button (data-close-conv-modal="add-member-modal").
    document.querySelectorAll('[data-close-conv-modal="add-member-modal"]').forEach(btn => {
      btn.addEventListener('click', () => this._closeAddMemberModal());
    });

    // Click backdrop to close.
    document.getElementById('add-member-modal')?.addEventListener('click', e => {
      if (e.target === e.currentTarget) this._closeAddMemberModal();
    });

    // Escape key closes it too.
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

  _submitAddMember() {
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

    // Update the in-memory group member count and persist.
    const group = ChatState.groups.find(g => g.id === conv.id);
    if (group) {
      group.memberCount = (group.memberCount ?? 1) + 1;
      ChatState.currentConversation = group;
      ChatState.save();
      // Refresh the header to reflect the new member count.
      this.updateHeader(group, 'groups');
      // Re-render the sidebar so any preview data is current.
      ChatConversations.render();
    }

    this._closeAddMemberModal();
  },
};
