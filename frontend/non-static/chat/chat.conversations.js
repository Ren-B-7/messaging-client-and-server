/**
 * Chat — Conversations
 * Renders the sidebar conversation list, handles opening a conversation,
 * search/filter, creating new conversations, and marking as read.
 * Depends on: Utils, ChatState, ChatUI, ChatMessages
 */

const ChatConversations = {
  // ─── Rendering ────────────────────────────────────────────────────────────

  /** Render the full sidebar list from ChatState.conversations. */
  render() {
    const list = document.getElementById('conversationsList');
    if (!list) return;

    if (ChatState.conversations.length === 0) {
      list.innerHTML = `
        <div class="text-center" style="padding: var(--space-8); color: var(--fg-tertiary);">
          <p>No conversations yet</p>
          <p style="font-size: var(--text-sm); margin-top: var(--space-2);">
            Start a new chat to begin messaging
          </p>
        </div>`;
      return;
    }

    list.innerHTML = ChatState.conversations
      .map(conv => this._renderItem(conv))
      .join('');

    list.querySelectorAll('.conversation-item').forEach(item => {
      item.addEventListener('click', () => this.open(item.dataset.conversationId));
    });
  },

  /** @returns {string} HTML for one sidebar row. */
  _renderItem(conv) {
    const { id, name, avatar, lastMessage, timestamp, unreadCount } = conv;
    const timeStr  = Utils.formatRelativeTime(new Date(timestamp));
    const isActive = ChatState.currentConversation?.id === id;

    return `
      <div class="conversation-item ${isActive ? 'active' : ''} ${unreadCount > 0 ? 'unread' : ''}"
           data-conversation-id="${id}">
        <div class="avatar avatar-sm">${avatar || Utils.getInitials(name)}</div>
        <div class="conversation-content">
          <div class="conversation-name">${Utils.escapeHtml(name)}</div>
          <div class="conversation-preview">${Utils.escapeHtml(lastMessage || 'No messages yet')}</div>
        </div>
        ${unreadCount > 0
          ? `<div class="unread-count">${unreadCount}</div>`
          : `<div class="conversation-time">${timeStr}</div>`
        }
      </div>`;
  },

  // ─── Open / mark read ─────────────────────────────────────────────────────

  /**
   * Select and display a conversation by id.
   * @param {string} conversationId
   */
  open(conversationId) {
    const conv = ChatState.findConversation(conversationId);
    if (!conv) return;

    ChatState.currentConversation = conv;

    // Update active highlight in sidebar.
    document.querySelectorAll('.conversation-item').forEach(item => {
      item.classList.toggle('active', item.dataset.conversationId === conversationId);
    });

    ChatUI.hideEmptyState();
    ChatUI.updateHeader(conv);
    ChatMessages.render(ChatState.getMessages(conversationId));
    this._markAsRead(conversationId);
  },

  _markAsRead(conversationId) {
    const conv = ChatState.findConversation(conversationId);
    if (conv && conv.unreadCount > 0) {
      conv.unreadCount = 0;
      ChatState.save();
      this.render();
    }
  },

  // ─── Search ───────────────────────────────────────────────────────────────

  /** Attach a debounced input handler to #conversationSearch. */
  setupSearch() {
    const input = document.getElementById('conversationSearch');
    if (!input) return;

    input.addEventListener('input', Utils.debounce(e => {
      const query = e.target.value.toLowerCase();
      document.querySelectorAll('.conversation-item').forEach(item => {
        const name    = item.querySelector('.conversation-name')?.textContent.toLowerCase() || '';
        item.style.display = name.includes(query) ? 'flex' : 'none';
      });
    }, 300));
  },

  // ─── New conversation ─────────────────────────────────────────────────────

  /** Attach click handler to #newChatBtn. */
  setupNewChatButton() {
    document.getElementById('newChatBtn')?.addEventListener('click', () => {
      const name = prompt('Enter contact name:');
      if (name?.trim()) this._create(name.trim());
    });
  },

  _create(name) {
    const conv = {
      id:          Utils.generateId(),
      name,
      avatar:      Utils.getInitials(name),
      lastMessage: '',
      timestamp:   Date.now(),
      unreadCount: 0,
      isOnline:    false,
    };

    ChatState.addConversation(conv);
    ChatState.save();
    this.render();
    this.open(conv.id);
  },
};
