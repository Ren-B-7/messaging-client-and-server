/**
 * Chat — Conversations
 * Renders the sidebar DM and Groups tabs, handles opening a conversation,
 * search/filter, refreshing from the API, and creating new DMs / groups
 * via modals.
 *
 * Depends on: Utils, ChatState, ChatUI, ChatMessages, ChatSSE
 */

const ChatConversations = {
  activeTab : 'dm', // 'dm' | 'groups'

  // ─── Tab switching ────────────────────────────────────────────────────────

  setupTabs() {
    document.querySelectorAll('.panel-tab').forEach(btn => {
      btn.addEventListener('click', () => {
        this.activeTab = btn.dataset.tab;

        document.querySelectorAll('.panel-tab')
            .forEach(b => b.classList.remove('active'));
        btn.classList.add('active');

        const newBtn = document.getElementById('newChatBtn');
        if (newBtn) {
          newBtn.title = this.activeTab === 'dm' ? 'New Message' : 'New Group';
        }

        this.refresh();
      });
    });
  },

  // ─── Rendering ───────────────────────────────────────────────────────────

  render() {
    if (this.activeTab === 'dm') {
      this._renderList(ChatState.conversations, 'dm');
    } else {
      this._renderList(ChatState.groups, 'groups');
    }
  },

  _renderList(items, type) {
    const list = document.getElementById('conversationsList');
    if (!list)
      return;

    if (!items.length) {
      const [title, sub] = type === 'dm'
        ? ['No conversations yet', 'Start a new chat to begin messaging']
        : ['No groups yet', 'Create a group to get started'];
      list.innerHTML = `
        <div class="text-center" style="padding: var(--space-8); color: var(--fg-tertiary);">
          <p>${title}</p>
          <p style="font-size: var(--text-sm); margin-top: var(--space-2);">${
          sub}</p>
        </div>`;
      return;
    }

    list.innerHTML = items.map(item => this._renderItem(item, type)).join('');

    list.querySelectorAll('.conversation-item').forEach(el => {
      el.addEventListener('click',
                          () => this.open(el.dataset.id, el.dataset.type));
    });
  },

  _renderItem(conv, type) {
    const {id, name, lastMessage, timestamp, unreadCount, avatarUrl} = conv;
    const timeStr = Utils.formatRelativeTime(new Date(timestamp));
    const isActive = ChatState.currentConversation?.id === id;
    const prefix = type === 'groups' ? '# ' : '';

    const avatarHtml =
        (type === 'dm' && avatarUrl)
            ? `<img class="avatar avatar-sm avatar-img" src="${
                  avatarUrl}" alt="${
                  Utils.escapeHtml(
                      name)}" onerror="this.style.display='none';this.nextElementSibling.style.display='flex'">` +
                  `<div class="avatar avatar-sm" style="display:none">${
                      Utils.getInitials(name)}</div>`
            : `<div class="avatar avatar-sm">${Utils.getInitials(name)}</div>`;

    return `
      <div class="conversation-item ${isActive ? 'active' : ''} ${
        unreadCount > 0 ? 'unread' : ''}"
           data-id="${id}" data-type="${type}">
        <div class="avatar-wrapper">${avatarHtml}</div>
        <div class="conversation-content">
          <div class="conversation-name">${prefix}${
        Utils.escapeHtml(name)}</div>
          <div class="conversation-preview">${
        Utils.escapeHtml(lastMessage || 'No messages yet')}</div>
        </div>
        ${
        unreadCount > 0 ? `<div class="unread-count">${unreadCount}</div>`
                        : `<div class="conversation-time">${timeStr}</div>`}
      </div>`;
  },

  // ─── Open / mark read ────────────────────────────────────────────────────

  /**
   * Select and display a conversation or group by id.
   * Tears down any existing SSE connection and opens a new one scoped to
   * the selected chat.
   *
   * @param {string}          id
   * @param {'dm'|'groups'}   type
   */
  open(id, type = this.activeTab) {
    const items =
        type === 'groups' ? ChatState.groups : ChatState.conversations;
    const conv = items.find(c => c.id === id);
    if (!conv)
      return;

    ChatState.currentConversation = conv;
    ChatState.currentConversationType = type;
    ChatState.save();

    document.querySelectorAll('.conversation-item')
        .forEach(
            el => { el.classList.toggle('active', el.dataset.id === id); });

    // Clear any leftover typing banner from the previous chat
    document.getElementById('typingIndicator')?.remove();
    ChatUI._typingUsers?.clear();

    ChatUI.hideEmptyState();
    ChatUI.updateHeader(conv, type);

    // loadMessages now connects the SSE stream (replays history automatically)
    ChatMessages.loadMessages(id);

    this._markAsRead(id, type);
  },

  _markAsRead(id, type) {
    const items =
        type === 'groups' ? ChatState.groups : ChatState.conversations;
    const conv = items.find(c => c.id === id);
    if (conv && conv.unreadCount > 0) {
      conv.unreadCount = 0;
      ChatState.save();
      this.render();
    }
  },

  // ─── Refresh from API ────────────────────────────────────────────────────

  async refresh() {
    const btn = document.getElementById('refreshConvsBtn');
    if (btn) {
      btn.disabled = true;
      btn.textContent = '…';
    }

    try {
      if (this.activeTab === 'dm') {
        await this._fetchChats();
      } else {
        await this._fetchGroups();
      }
    } catch (e) {
      console.error('[chat] refresh failed:', e);
      this.render();
    } finally {
      if (btn) {
        btn.disabled = false;
        btn.textContent = '↺';
      }
    }
  },

  async _fetchChats() {
    const res = await fetch('/api/chats');
    if (!res.ok)
      throw new Error(`HTTP ${res.status}`);
    const data = await res.json();

    const chats = data.data?.chats ?? data.chats ?? [];
    ChatState.conversations = chats.filter(c => c.chat_type === 'direct')
                                  .map(c => ({
                                         id : String(c.chat_id || c.id),
                                         name : c.name ?? 'Unnamed chat',
                                         lastMessage : c.last_message ?? '',
                                         timestamp : c.created_at ?? Date.now(),
                                         unreadCount : c.unread_count ?? 0,
                                         isOnline : c.is_online ?? false,
                                         avatarUrl : c.avatar_url ?? null,
                                       }));
    ChatState.save();
    this.render();
  },

  async _fetchGroups() {
    const res = await fetch('/api/chats');
    if (!res.ok)
      throw new Error(`HTTP ${res.status}`);
    const data = await res.json();

    const chats = data.data?.chats ?? data.chats ?? [];
    ChatState.groups = chats.filter(c => c.chat_type === 'group')
                           .map(g => ({
                                  id : String(g.chat_id || g.id),
                                  name : g.name ?? 'Unnamed group',
                                  lastMessage : g.last_message ?? '',
                                  timestamp : g.created_at ?? Date.now(),
                                  unreadCount : g.unread_count ?? 0,
                                  memberCount : g.member_count ?? null,
                                }));
    ChatState.save();
    this.render();
  },

  // ─── Search ──────────────────────────────────────────────────────────────

  setupSearch() {
    const input = document.getElementById('conversationSearch');
    if (!input)
      return;

    input.addEventListener('input', Utils.debounce(e => {
      const query = e.target.value.toLowerCase();
      document.querySelectorAll('.conversation-item').forEach(el => {
        const name =
            el.querySelector('.conversation-name')?.textContent.toLowerCase() ||
            '';
        el.style.display = name.includes(query) ? 'flex' : 'none';
      });
    }, 300));
  },

  // ─── New DM / Group buttons & modals ─────────────────────────────────────

  setupNewButtons() {
    document.getElementById('newChatBtn')?.addEventListener('click', () => {
      this._openModal(this.activeTab === 'dm' ? 'new-dm-modal'
                                              : 'new-group-modal');
    });

    document.getElementById('refreshConvsBtn')
        ?.addEventListener('click', () => { this.refresh(); });

    document.getElementById('newDmSubmitBtn')
        ?.addEventListener('click', () => this._submitDm());
    document.getElementById('newGroupSubmitBtn')
        ?.addEventListener('click', () => this._submitGroup());

    document.getElementById('dmRecipientInput')
        ?.addEventListener('keydown', e => {
          if (e.key === 'Enter')
            this._submitDm();
        });
    document.getElementById('groupNameInput')
        ?.addEventListener('keydown', e => {
          if (e.key === 'Enter')
            this._submitGroup();
        });

    document.querySelectorAll('[data-close-conv-modal]').forEach(btn => {
      btn.addEventListener('click',
                           () => this._closeModal(btn.dataset.closeConvModal));
    });

    ['new-dm-modal', 'new-group-modal'].forEach(id => {
      document.getElementById(id)?.addEventListener('click', e => {
        if (e.target === e.currentTarget)
          this._closeModal(id);
      });
    });

    document.addEventListener('keydown', e => {
      if (e.key === 'Escape') {
        ['new-dm-modal', 'new-group-modal'].forEach(id => this._closeModal(id));
      }
    });
  },

  _openModal(id) {
    const modal = document.getElementById(id);
    if (!modal)
      return;
    modal.classList.add('open');
    const input = modal.querySelector('input[type="text"]');
    const err = modal.querySelector('.conv-modal-error');
    if (input) {
      input.value = '';
      input.focus();
    }
    if (err)
      err.textContent = '';
  },

  _closeModal(id) { document.getElementById(id)?.classList.remove('open'); },

  async _submitDm() {
    const input = document.getElementById('dmRecipientInput');
    const errorEl = document.getElementById('dmRecipientError');
    const username = input?.value.trim();

    if (!username) {
      if (errorEl)
        errorEl.textContent = 'Please enter a username.';
      return;
    }

    try {
      const response = await fetch('/api/chats', {
        method : 'POST',
        headers : {'Content-Type' : 'application/json'},
        body : JSON.stringify({username}),
      });

      if (!response.ok) {
        const errData = await response.json();
        throw new Error(errData.message || 'Failed to create DM');
      }

      const newChat = await response.json();
      const chatData = newChat.data || newChat;

      ChatState.addConversation({
        id : String(chatData.id || chatData.chat_id),
        name : chatData.name || username,
        lastMessage : '',
        timestamp : chatData.created_at || Date.now(),
        unreadCount : 0,
        isOnline : false,
        avatarUrl : chatData.avatar_url ?? null,
      });
      ChatState.save();

      this._closeModal('new-dm-modal');
      this._switchTab('dm');
      this.open(String(chatData.id || chatData.chat_id), 'dm');
    } catch (err) {
      if (errorEl)
        errorEl.textContent = err.message;
      console.error('[conversations] Create DM error:', err);
    }
  },

  async _submitGroup() {
    const input = document.getElementById('groupNameInput');
    const errorEl = document.getElementById('groupNameError');
    const name = input?.value.trim();

    if (!name) {
      if (errorEl)
        errorEl.textContent = 'Please enter a group name.';
      return;
    }

    try {
      const response = await fetch('/api/groups', {
        method : 'POST',
        headers : {'Content-Type' : 'application/json'},
        body : JSON.stringify({name}),
      });

      if (!response.ok) {
        const errData = await response.json();
        throw new Error(errData.message || 'Failed to create group');
      }

      const newGroup = await response.json();
      const groupData = newGroup.data || newGroup;

      ChatState.addGroup({
        id : String(groupData.chat_id || groupData.group_id || groupData.id),
        name : groupData.name || name,
        lastMessage : '',
        timestamp : groupData.created_at || Date.now(),
        unreadCount : 0,
        memberCount : 1,
      });
      ChatState.save();

      this._closeModal('new-group-modal');
      this._switchTab('groups');
      this.open(String(groupData.chat_id || groupData.group_id || groupData.id),
                'groups');
    } catch (err) {
      if (errorEl)
        errorEl.textContent = err.message;
      console.error('[conversations] Create Group error:', err);
    }
  },

  _switchTab(tab) {
    this.activeTab = tab;
    document.querySelectorAll('.panel-tab').forEach(btn => {
      btn.classList.toggle('active', btn.dataset.tab === tab);
    });
    this.render();
  },
};
