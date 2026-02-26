/**
 * Chat — Conversations
 * Renders the sidebar DM and Groups tabs, handles opening a conversation,
 * search/filter, refreshing from the API, and creating new DMs / groups
 * via modals.
 *
 * Depends on: Utils, ChatState, ChatUI, ChatMessages
 */

const ChatConversations = {
  activeTab: 'dm',   // 'dm' | 'groups'

  // ─── Tab switching ────────────────────────────────────────────────────────

  setupTabs() {
    document.querySelectorAll('.panel-tab').forEach(btn => {
      btn.addEventListener('click', () => {
        this.activeTab = btn.dataset.tab;

        // Update active tab styling.
        document.querySelectorAll('.panel-tab').forEach(b => b.classList.remove('active'));
        btn.classList.add('active');

        // Keep the + button title in sync so it's clear what it will open.
        const newBtn = document.getElementById('newChatBtn');
        if (newBtn) {
          newBtn.title = this.activeTab === 'dm' ? 'New Message' : 'New Group';
        }

        // Fetch the relevant data for the newly selected tab, then render.
        this.refresh();
      });
    });
  },

  // ─── Rendering ───────────────────────────────────────────────────────────

  /** Render the sidebar list for the active tab. */
  render() {
    if (this.activeTab === 'dm') {
      this._renderList(ChatState.conversations, 'dm');
    } else {
      this._renderList(ChatState.groups, 'groups');
    }
  },

  _renderList(items, type) {
    const list = document.getElementById('conversationsList');
    if (!list) return;

    if (!items.length) {
      const [title, sub] = type === 'dm'
        ? ['No conversations yet', 'Start a new chat to begin messaging']
        : ['No groups yet',        'Create a group to get started'];
      list.innerHTML = `
        <div class="text-center" style="padding: var(--space-8); color: var(--fg-tertiary);">
          <p>${title}</p>
          <p style="font-size: var(--text-sm); margin-top: var(--space-2);">${sub}</p>
        </div>`;
      return;
    }

    list.innerHTML = items.map(item => this._renderItem(item, type)).join('');

    list.querySelectorAll('.conversation-item').forEach(el => {
      el.addEventListener('click', () => this.open(el.dataset.id, el.dataset.type));
    });
  },

  /** @returns {string} HTML for one sidebar row. */
  _renderItem(conv, type) {
    const { id, name, lastMessage, timestamp, unreadCount } = conv;
    const timeStr  = Utils.formatRelativeTime(new Date(timestamp));
    const isActive = ChatState.currentConversation?.id === id;
    const prefix   = type === 'groups' ? '# ' : '';   // visual group indicator

    return `
      <div class="conversation-item ${isActive ? 'active' : ''} ${unreadCount > 0 ? 'unread' : ''}"
           data-id="${id}" data-type="${type}">
        <div class="avatar avatar-sm">${Utils.getInitials(name)}</div>
        <div class="conversation-content">
          <div class="conversation-name">${prefix}${Utils.escapeHtml(name)}</div>
          <div class="conversation-preview">${Utils.escapeHtml(lastMessage || 'No messages yet')}</div>
        </div>
        ${unreadCount > 0
          ? `<div class="unread-count">${unreadCount}</div>`
          : `<div class="conversation-time">${timeStr}</div>`
        }
      </div>`;
  },

  // ─── Open / mark read ────────────────────────────────────────────────────

  /**
   * Select and display a conversation or group by id.
   * @param {string} id
   * @param {'dm'|'groups'} type
   */
  open(id, type = this.activeTab) {
    const items = type === 'groups' ? ChatState.groups : ChatState.conversations;
    const conv  = items.find(c => c.id === id);
    if (!conv) return;

    ChatState.currentConversation     = conv;
    ChatState.currentConversationType = type;
    ChatState.save();

    document.querySelectorAll('.conversation-item').forEach(el => {
      el.classList.toggle('active', el.dataset.id === id);
    });

    ChatUI.hideEmptyState();
    ChatUI.updateHeader(conv, type);
    ChatMessages.render(ChatState.getMessages(id));
    this._markAsRead(id, type);
  },

  _markAsRead(id, type) {
    const items = type === 'groups' ? ChatState.groups : ChatState.conversations;
    const conv  = items.find(c => c.id === id);
    if (conv && conv.unreadCount > 0) {
      conv.unreadCount = 0;
      ChatState.save();
      this.render();
    }
  },

  // ─── Refresh from API ────────────────────────────────────────────────────

  /**
   * Fetch the active tab's data from the server and re-render.
   *   DM tab     → GET /api/messages
   *   Groups tab → GET /api/chats
   *
   * Called on page load (DM tab) and whenever the user clicks Refresh or
   * switches tabs.
   */
  async refresh() {
    const btn = document.getElementById('refreshConvsBtn');
    if (btn) { btn.disabled = true; btn.textContent = '…'; }

    try {
      if (this.activeTab === 'dm') {
        await this._fetchMessages();
      } else {
        await this._fetchChats();
      }
    } catch (e) {
      console.error('[chat] refresh failed:', e);
      // Still render whatever is cached in state so the UI is not blank.
      this.render();
    } finally {
      if (btn) { btn.disabled = false; btn.textContent = '↺'; }
    }
  },

  async _fetchMessages() {
    const res = await fetch('/api/messages');
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = await res.json();

    const convs = data.data?.conversations ?? data.conversations ?? [];
    ChatState.conversations = convs.map(c => ({
      id:          String(c.id),
      name:        c.name         ?? c.username ?? 'Unknown',
      lastMessage: c.last_message ?? '',
      timestamp:   c.timestamp    ?? Date.now(),
      unreadCount: c.unread_count ?? 0,
      isOnline:    c.is_online    ?? false,
    }));
    ChatState.save();
    this.render();
  },

  async _fetchChats() {
    const res = await fetch('/api/chats');
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = await res.json();

    const groups = data.data?.groups ?? data.groups ?? [];
    ChatState.groups = groups.map(g => ({
      id:          String(g.id),
      name:        g.name         ?? 'Unnamed group',
      lastMessage: g.last_message ?? '',
      timestamp:   g.timestamp    ?? Date.now(),
      unreadCount: g.unread_count ?? 0,
      memberCount: g.member_count ?? null,
    }));
    ChatState.save();
    this.render();
  },

  // ─── Search ──────────────────────────────────────────────────────────────

  setupSearch() {
    const input = document.getElementById('conversationSearch');
    if (!input) return;

    input.addEventListener('input', Utils.debounce(e => {
      const query = e.target.value.toLowerCase();
      document.querySelectorAll('.conversation-item').forEach(el => {
        const name = el.querySelector('.conversation-name')?.textContent.toLowerCase() || '';
        el.style.display = name.includes(query) ? 'flex' : 'none';
      });
    }, 300));
  },

  // ─── New DM / Group buttons & modals ─────────────────────────────────────

  setupNewButtons() {
    // + button is context-sensitive: opens DM or group modal based on active tab.
    document.getElementById('newChatBtn')?.addEventListener('click', () => {
      this._openModal(this.activeTab === 'dm' ? 'new-dm-modal' : 'new-group-modal');
    });

    // Refresh button.
    document.getElementById('refreshConvsBtn')?.addEventListener('click', () => {
      this.refresh();
    });

    // Modal submit buttons.
    document.getElementById('newDmSubmitBtn')?.addEventListener('click',    () => this._submitDm());
    document.getElementById('newGroupSubmitBtn')?.addEventListener('click', () => this._submitGroup());

    // Enter key inside modal inputs triggers submit.
    document.getElementById('dmRecipientInput')?.addEventListener('keydown', e => {
      if (e.key === 'Enter') this._submitDm();
    });
    document.getElementById('groupNameInput')?.addEventListener('keydown', e => {
      if (e.key === 'Enter') this._submitGroup();
    });

    // Close buttons (data-close-conv-modal attribute).
    document.querySelectorAll('[data-close-conv-modal]').forEach(btn => {
      btn.addEventListener('click', () => this._closeModal(btn.dataset.closeConvModal));
    });

    // Clicking the backdrop (outside the modal card) also closes.
    ['new-dm-modal', 'new-group-modal'].forEach(id => {
      document.getElementById(id)?.addEventListener('click', e => {
        if (e.target === e.currentTarget) this._closeModal(id);
      });
    });

    // Escape key closes any open modal.
    document.addEventListener('keydown', e => {
      if (e.key === 'Escape') {
        ['new-dm-modal', 'new-group-modal'].forEach(id => this._closeModal(id));
      }
    });
  },

  _openModal(id) {
    const modal = document.getElementById(id);
    if (!modal) return;
    modal.classList.add('open');
    // Reset state from any previous open.
    const input = modal.querySelector('input[type="text"]');
    const err   = modal.querySelector('.conv-modal-error');
    if (input) { input.value = ''; input.focus(); }
    if (err)   err.textContent = '';
  },

  _closeModal(id) {
    document.getElementById(id)?.classList.remove('open');
  },

  _submitDm() {
    const input   = document.getElementById('dmRecipientInput');
    const errorEl = document.getElementById('dmRecipientError');
    const name    = input?.value.trim();

    if (!name) {
      if (errorEl) errorEl.textContent = 'Please enter a name or username.';
      return;
    }
    if (errorEl) errorEl.textContent = '';

    const conv = {
      id:          Utils.generateId(),
      name,
      lastMessage: '',
      timestamp:   Date.now(),
      unreadCount: 0,
      isOnline:    false,
    };

    ChatState.addConversation(conv);
    ChatState.save();
    this._closeModal('new-dm-modal');

    // Switch to DM tab and open the new conversation.
    this._switchTab('dm');
    this.open(conv.id, 'dm');
  },

  _submitGroup() {
    const input   = document.getElementById('groupNameInput');
    const errorEl = document.getElementById('groupNameError');
    const name    = input?.value.trim();

    if (!name) {
      if (errorEl) errorEl.textContent = 'Please enter a group name.';
      return;
    }
    if (errorEl) errorEl.textContent = '';

    const group = {
      id:          Utils.generateId(),
      name,
      lastMessage: '',
      timestamp:   Date.now(),
      unreadCount: 0,
      memberCount: 1,
    };

    ChatState.addGroup(group);
    ChatState.save();
    this._closeModal('new-group-modal');

    // Switch to Groups tab and open the new group.
    this._switchTab('groups');
    this.open(group.id, 'groups');
  },

  /**
   * Programmatically switch the active tab without triggering a refresh.
   * Used after creating a new conversation/group locally.
   * @param {'dm'|'groups'} tab
   */
  _switchTab(tab) {
    this.activeTab = tab;
    document.querySelectorAll('.panel-tab').forEach(btn => {
      btn.classList.toggle('active', btn.dataset.tab === tab);
    });
    this.render();
  },
};
