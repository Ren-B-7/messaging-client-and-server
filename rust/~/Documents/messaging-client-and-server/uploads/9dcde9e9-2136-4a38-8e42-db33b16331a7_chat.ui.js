/**
 * Chat — UI & Header
 * Manages the empty-state / active-chat view toggle, chat header rendering,
 * action button handlers, group settings (rename, delete, member management),
 * and the typing indicator.
 *
 * Depends on: Utils, ChatState, ChatConversations
 */

const ChatUI = {
  // ── Empty / active state ─────────────────────────────────────────────────

  showEmptyState() {
    const empty = document.getElementById("emptyChatState");
    const active = document.getElementById("activeChatView");
    if (empty) empty.style.display = "flex";
    if (active) active.style.display = "none";
  },

  hideEmptyState() {
    const empty = document.getElementById("emptyChatState");
    const active = document.getElementById("activeChatView");
    if (empty) empty.style.display = "none";
    if (active) active.style.display = "flex";
  },

  // ── Chat header ──────────────────────────────────────────────────────────

  updateHeader(conversation, type = "dm") {
    const nameEl = document.getElementById("chatName");
    const avatarEl = document.getElementById("chatAvatar");
    const dotEl = document.getElementById("statusDot");
    const textEl = document.getElementById("statusText");
    const addMemberBtn = document.getElementById("addMemberBtn");
    const groupSettingsBtn = document.getElementById("groupSettingsBtn");

    if (nameEl) nameEl.textContent = Utils.escapeHtml(conversation.name);
    if (avatarEl) avatarEl.textContent = Utils.getInitials(conversation.name);

    if (type === "groups") {
      if (dotEl) dotEl.className = "status-dot";
      if (textEl)
        textEl.textContent = conversation.memberCount
          ? `${conversation.memberCount} members`
          : "Group";
      if (textEl) textEl.style.color = "";
      if (addMemberBtn) addMemberBtn.style.display = "";
      if (groupSettingsBtn) groupSettingsBtn.style.display = "";
    } else {
      if (addMemberBtn) addMemberBtn.style.display = "none";
      if (groupSettingsBtn) groupSettingsBtn.style.display = "none";
      if (textEl) textEl.style.color = "";
    }
  },

  // ── Typing indicator ─────────────────────────────────────────────────────

  _typingUsers: new Set(),

  showTyping(userId) {
    this._typingUsers.add(userId);
    this._renderTyping();
  },

  hideTyping(userId) {
    this._typingUsers.delete(userId);
    this._renderTyping();
  },

  _renderTyping() {
    let banner = document.getElementById("typingIndicator");
    if (this._typingUsers.size === 0) {
      if (banner) banner.remove();
      return;
    }
    if (!banner) {
      banner = document.createElement("div");
      banner.id = "typingIndicator";
      banner.className = "typing-indicator";
      const bottomBar = document.querySelector(".chat-bottom-bar");
      const inputArea = document.getElementById("messageInputArea");
      if (bottomBar && inputArea) {
        bottomBar.insertBefore(banner, inputArea);
      } else if (inputArea) {
        inputArea.parentNode.insertBefore(banner, inputArea);
      } else {
        document.getElementById("messagesContainer")?.appendChild(banner);
      }
    }
    const count = this._typingUsers.size;
    banner.innerHTML = `
      <span class="typing-dots"><span></span><span></span><span></span></span>
      <span class="typing-text">${count === 1 ? "Someone is typing" : `${count} people are typing`}&hellip;</span>`;
  },

  // ── Action buttons setup ─────────────────────────────────────────────────

  setupActionButtons() {
    // File upload + files modal wired in chat.files.js
    ChatFiles.setupUpload();

    // Quick-add button in header
    document.getElementById("addMemberBtn")?.addEventListener("click", () => {
      const conv = ChatState.currentConversation;
      if (!conv) return;
      const nameEl = document.getElementById("addMemberGroupName");
      if (nameEl) nameEl.textContent = conv.name;
      this._openAddMemberModal();
    });

    document.getElementById("addMemberSubmitBtn")?.addEventListener("click", () => this._submitAddMember());
    document.getElementById("addMemberInput")?.addEventListener("keydown", (e) => { if (e.key === "Enter") this._submitAddMember(); });
    document.querySelectorAll('[data-close-conv-modal="add-member-modal"]').forEach((btn) => btn.addEventListener("click", () => this._closeAddMemberModal()));
    document.getElementById("add-member-modal")?.addEventListener("click", (e) => { if (e.target === e.currentTarget) this._closeAddMemberModal(); });

    // Group settings button
    document.getElementById("groupSettingsBtn")?.addEventListener("click", () => this._openGroupSettings());

    // Group settings modal
    document.querySelectorAll('[data-close-conv-modal="group-settings-modal"]').forEach((btn) => btn.addEventListener("click", () => this._closeModal("group-settings-modal")));
    document.getElementById("group-settings-modal")?.addEventListener("click", (e) => { if (e.target === e.currentTarget) this._closeModal("group-settings-modal"); });

    // Rename
    document.getElementById("groupRenameBtn")?.addEventListener("click", () => this._submitRename());
    document.getElementById("groupRenameInput")?.addEventListener("keydown", (e) => { if (e.key === "Enter") this._submitRename(); });

    // Delete flow
    document.getElementById("groupDeleteBtn")?.addEventListener("click", () => {
      const conv = ChatState.currentConversation;
      if (!conv) return;
      const nameEl = document.getElementById("groupDeleteName");
      if (nameEl) nameEl.textContent = conv.name;
      const errEl = document.getElementById("groupDeleteError");
      if (errEl) errEl.textContent = "";
      this._openModal("group-delete-confirm-modal");
    });

    document.querySelectorAll('[data-close-conv-modal="group-delete-confirm-modal"]').forEach((btn) => btn.addEventListener("click", () => this._closeModal("group-delete-confirm-modal")));
    document.getElementById("group-delete-confirm-modal")?.addEventListener("click", (e) => { if (e.target === e.currentTarget) this._closeModal("group-delete-confirm-modal"); });
    document.getElementById("groupDeleteConfirmBtn")?.addEventListener("click", () => this._submitDeleteGroup());

    // Add member inside settings
    document.getElementById("groupAddMemberBtn")?.addEventListener("click", () => this._submitAddMemberFromSettings());
    document.getElementById("groupAddMemberInput")?.addEventListener("keydown", (e) => { if (e.key === "Enter") this._submitAddMemberFromSettings(); });
    document.getElementById("groupAddMemberInput")?.addEventListener("input", Utils.debounce((e) => this._searchUsers(e.target.value), 300));

    // Close search results when clicking outside
    document.addEventListener("click", (e) => {
      const wrapper = document.querySelector(".group-search-wrapper");
      if (wrapper && !wrapper.contains(e.target)) {
        const results = document.getElementById("groupSearchResults");
        if (results) { results.style.display = "none"; results.innerHTML = ""; }
      }
    });

    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        this._closeAddMemberModal();
        this._closeModal("group-settings-modal");
        this._closeModal("group-delete-confirm-modal");
      }
    });
  },

  // ── Group settings modal ─────────────────────────────────────────────────

  async _openGroupSettings() {
    const conv = ChatState.currentConversation;
    if (!conv) return;

    const renameInput = document.getElementById("groupRenameInput");
    if (renameInput) renameInput.value = conv.name;

    ["groupRenameError", "groupAddMemberError"].forEach((id) => {
      const el = document.getElementById(id);
      if (el) { el.textContent = ""; el.style.color = ""; }
    });

    const addInput = document.getElementById("groupAddMemberInput");
    if (addInput) addInput.value = "";

    const searchResults = document.getElementById("groupSearchResults");
    if (searchResults) { searchResults.style.display = "none"; searchResults.innerHTML = ""; }

    this._openModal("group-settings-modal");
    await this._loadMembers(conv.id);
  },

  async _loadMembers(chatId) {
    const listEl = document.getElementById("groupMembersList");
    const countEl = document.getElementById("groupMemberCount");
    if (!listEl) return;

    listEl.innerHTML = `<p style="color:var(--fg-tertiary);font-size:var(--text-sm)">Loading…</p>`;

    try {
      const res = await fetch(`/api/groups/${encodeURIComponent(chatId)}/members`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      const members = data.data?.members ?? data.members ?? [];

      if (countEl) countEl.textContent = `(${members.length})`;

      if (!members.length) {
        listEl.innerHTML = `<p style="color:var(--fg-tertiary);font-size:var(--text-sm)">No members found.</p>`;
        return;
      }

      const myId = ChatState.currentUser?.id ?? null;

      listEl.innerHTML = members.map((m) => {
        const isMe = m.user_id === myId;
        const displayName = m.username || `User ${m.user_id}`;
        return `
          <div class="group-member-row" data-user-id="${m.user_id}">
            <div class="avatar avatar-sm">${Utils.getInitials(displayName)}</div>
            <div class="group-member-info">
              <span class="group-member-name">${Utils.escapeHtml(displayName)}</span>
              ${m.role && m.role !== 'member' ? `<span class="group-member-role">${Utils.escapeHtml(m.role)}</span>` : ''}
              ${isMe ? `<span class="group-member-you">you</span>` : ''}
            </div>
            ${!isMe ? `
              <button class="group-member-remove-btn" data-user-id="${m.user_id}" title="Remove member">
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
                  <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
                </svg>
              </button>` : ''}
          </div>`;
      }).join('');

      listEl.querySelectorAll('.group-member-remove-btn').forEach((btn) => {
        btn.addEventListener('click', () => this._removeMember(chatId, parseInt(btn.dataset.userId, 10)));
      });

    } catch (e) {
      listEl.innerHTML = `<p style="color:var(--danger);font-size:var(--text-sm)">Failed to load members.</p>`;
      console.error('[group-settings] Load members error:', e);
    }
  },

  async _removeMember(chatId, targetUserId) {
    try {
      const res = await fetch(`/api/groups/${encodeURIComponent(chatId)}/members`, {
        method: 'DELETE',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ user_id: targetUserId }),
      });
      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.message || 'Failed to remove member');
      }

      const group = ChatState.groups.find((g) => g.id === String(chatId));
      if (group && group.memberCount > 0) {
        group.memberCount--;
        ChatState.currentConversation = group;
        ChatState.save();
        this.updateHeader(group, 'groups');
        ChatConversations.render();
      }

      await this._loadMembers(chatId);
    } catch (e) {
      console.error('[group-settings] Remove member error:', e);
      alert(e.message || 'Failed to remove member.');
    }
  },

  // ── Rename ───────────────────────────────────────────────────────────────

  async _submitRename() {
    const input = document.getElementById("groupRenameInput");
    const errEl = document.getElementById("groupRenameError");
    const newName = input?.value.trim();
    const conv = ChatState.currentConversation;

    if (errEl) { errEl.textContent = ""; errEl.style.color = ""; }
    if (!newName) { if (errEl) errEl.textContent = "Please enter a group name."; return; }
    if (!conv) return;

    const btn = document.getElementById("groupRenameBtn");
    if (btn) btn.disabled = true;

    try {
      const res = await fetch(`/api/groups/${encodeURIComponent(conv.id)}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: newName }),
      });

      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.message || 'Failed to rename group');
      }

      const group = ChatState.groups.find((g) => g.id === conv.id);
      if (group) {
        group.name = newName;
        ChatState.currentConversation = group;
        ChatState.save();
        this.updateHeader(group, 'groups');
        ChatConversations.render();
      }

      if (errEl) {
        errEl.style.color = 'var(--success)';
        errEl.textContent = 'Renamed successfully.';
        setTimeout(() => { errEl.textContent = ''; errEl.style.color = ''; }, 2000);
      }
    } catch (e) {
      if (errEl) errEl.textContent = e.message || 'Failed to rename.';
      console.error('[group-settings] Rename error:', e);
    } finally {
      if (btn) btn.disabled = false;
    }
  },

  // ── Delete group ─────────────────────────────────────────────────────────

  async _submitDeleteGroup() {
    const conv = ChatState.currentConversation;
    const errEl = document.getElementById("groupDeleteError");
    if (!conv) return;

    const btn = document.getElementById("groupDeleteConfirmBtn");
    if (btn) btn.disabled = true;
    if (errEl) errEl.textContent = "";

    try {
      const res = await fetch(`/api/groups/${encodeURIComponent(conv.id)}`, { method: 'DELETE' });
      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.message || 'Failed to delete group');
      }

      ChatState.groups = ChatState.groups.filter((g) => g.id !== conv.id);
      ChatState.currentConversation = null;
      ChatState.currentConversationType = 'groups';
      ChatState.save();

      this._closeModal("group-delete-confirm-modal");
      this._closeModal("group-settings-modal");
      this.showEmptyState();
      ChatConversations.render();

    } catch (e) {
      if (errEl) errEl.textContent = e.message || 'Failed to delete group.';
      console.error('[group-settings] Delete error:', e);
    } finally {
      if (btn) btn.disabled = false;
    }
  },

  // ── Add member from settings panel ───────────────────────────────────────

  async _submitAddMemberFromSettings() {
    const input = document.getElementById("groupAddMemberInput");
    const errEl = document.getElementById("groupAddMemberError");
    const username = input?.value.trim();
    const conv = ChatState.currentConversation;

    if (errEl) { errEl.textContent = ""; errEl.style.color = ""; }
    if (!username) { if (errEl) errEl.textContent = "Please enter a username."; return; }
    if (!conv) return;

    const btn = document.getElementById("groupAddMemberBtn");
    if (btn) btn.disabled = true;

    try {
      const res = await fetch(`/api/groups/${encodeURIComponent(conv.id)}/members`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username }),
      });

      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.message || 'Failed to add member');
      }

      const group = ChatState.groups.find((g) => g.id === conv.id);
      if (group) {
        group.memberCount = (group.memberCount ?? 1) + 1;
        ChatState.currentConversation = group;
        ChatState.save();
        this.updateHeader(group, 'groups');
        ChatConversations.render();
      }

      if (input) input.value = "";
      const searchResults = document.getElementById("groupSearchResults");
      if (searchResults) { searchResults.style.display = "none"; searchResults.innerHTML = ""; }

      await this._loadMembers(conv.id);

      if (errEl) {
        errEl.style.color = 'var(--success)';
        errEl.textContent = `${username} added successfully.`;
        setTimeout(() => { errEl.textContent = ''; errEl.style.color = ''; }, 2500);
      }
    } catch (e) {
      if (errEl) errEl.textContent = e.message || 'Failed to add member.';
      console.error('[group-settings] Add member error:', e);
    } finally {
      if (btn) btn.disabled = false;
    }
  },

  // ── User search autocomplete ──────────────────────────────────────────────

  async _searchUsers(query) {
    const resultsEl = document.getElementById("groupSearchResults");
    if (!resultsEl) return;

    const q = query.trim();
    if (q.length < 2) {
      resultsEl.style.display = "none";
      resultsEl.innerHTML = "";
      return;
    }

    try {
      const res = await fetch(`/api/users/search?q=${encodeURIComponent(q)}`);
      if (!res.ok) return;
      const data = await res.json();
      const users = data.data?.users ?? data.users ?? [];

      if (!users.length) {
        resultsEl.style.display = "none";
        return;
      }

      resultsEl.innerHTML = users.map((u) => `
        <div class="group-search-result-item" data-username="${Utils.escapeHtml(u.username)}">
          <div class="avatar avatar-sm">${Utils.getInitials(u.username)}</div>
          <span>${Utils.escapeHtml(u.username)}</span>
        </div>`).join('');

      resultsEl.querySelectorAll('.group-search-result-item').forEach((item) => {
        item.addEventListener('click', () => {
          const input = document.getElementById("groupAddMemberInput");
          if (input) input.value = item.dataset.username;
          resultsEl.style.display = "none";
          resultsEl.innerHTML = "";
        });
      });

      resultsEl.style.display = "block";
    } catch (e) {
      console.warn('[group-settings] User search error:', e);
    }
  },

  // ── Legacy quick-add modal (header button) ───────────────────────────────

  _openAddMemberModal() {
    const modal = document.getElementById("add-member-modal");
    if (!modal) return;
    modal.classList.add("open");
    const input = document.getElementById("addMemberInput");
    const err = document.getElementById("addMemberError");
    if (input) { input.value = ""; input.focus(); }
    if (err) err.textContent = "";
  },

  _closeAddMemberModal() {
    document.getElementById("add-member-modal")?.classList.remove("open");
  },

  async _submitAddMember() {
    const input = document.getElementById("addMemberInput");
    const errorEl = document.getElementById("addMemberError");
    const name = input?.value.trim();
    const conv = ChatState.currentConversation;

    if (!name) { if (errorEl) errorEl.textContent = "Please enter a name or username."; return; }
    if (!conv) { if (errorEl) errorEl.textContent = "No active group selected."; return; }

    try {
      const addRes = await fetch(`/api/groups/${encodeURIComponent(conv.id)}/members`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username: name }),
      });

      if (!addRes.ok) {
        const errData = await addRes.json();
        throw new Error(errData.message || "Failed to add member");
      }

      const group = ChatState.groups.find((g) => g.id === conv.id);
      if (group) {
        group.memberCount = (group.memberCount ?? 1) + 1;
        ChatState.currentConversation = group;
        ChatState.save();
        this.updateHeader(group, "groups");
        ChatConversations.render();
      }

      this._closeAddMemberModal();
    } catch (err) {
      if (errorEl) errorEl.textContent = err.message || "Failed to add member.";
    }
  },

  // ── Generic modal helpers ─────────────────────────────────────────────────

  _openModal(id) {
    const modal = document.getElementById(id);
    if (!modal) return;
    modal.classList.add("open");
    const input = modal.querySelector('input[type="text"]');
    if (input) input.focus();
  },

  _closeModal(id) {
    document.getElementById(id)?.classList.remove("open");
  },
};
