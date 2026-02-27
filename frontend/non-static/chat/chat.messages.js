/**
 * Chat — Messages
 * Renders the message list, handles sending, and simulates incoming replies.
 * Depends on: Utils, ChatState
 */

const ChatMessages = {

  // ── Helpers ──────────────────────────────────────────────────────────────

  /** Return the logged-in user's numeric ID, or null if unavailable. */
  _currentUserId() {
    return ChatState.currentUser?.id ?? null;
  },

  /** Map a raw API message object to the local format. */
  _fromApi(msg) {
    const myId = this._currentUserId();
    return {
      id:        msg.id,
      text:      msg.content,
      content:   msg.content,
      timestamp: msg.sent_at * 1000,          // seconds → ms
      isSent:    myId !== null && msg.sender_id === myId,
      sender_id: msg.sender_id,
    };
  },

  // ── Rendering ────────────────────────────────────────────────────────────

  /**
   * Render all messages for the active conversation into #messagesContainer.
   * @param {object[]} messages
   */
  render(messages) {
    const container = document.getElementById('messagesContainer');
    if (!container) return;

    if (!messages.length) {
      container.innerHTML = `
        <div class="text-center" style="padding: var(--space-8); color: var(--fg-tertiary);">
          <p>No messages yet</p>
          <p style="font-size: var(--text-sm); margin-top: var(--space-2);">
            Send a message to start the conversation
          </p>
        </div>`;
      return;
    }

    container.innerHTML = messages.map(msg => this._renderItem(msg)).join('');
    container.scrollTop = container.scrollHeight;
  },

  /** @returns {string} HTML for a single message bubble. */
  _renderItem({ id, text, content, timestamp, isSent, sender_id }) {
    const messageText = text || content || '';
    const time = typeof timestamp === 'number' ? new Date(timestamp) : new Date();

    // isSent drives the visual side (right = sent, left = received).
    // Fall back to checking sender_id if isSent was not pre-computed.
    const myId = this._currentUserId();
    const sentByMe = isSent || (myId !== null && sender_id === myId);

    return `
      <div class="message ${sentByMe ? 'sent' : 'received'}">
        <div class="message-bubble">${Utils.escapeHtml(messageText)}</div>
        <div class="message-time">${Utils.formatTime(time)}</div>
      </div>`;
  },

  /** Show a temporary inline error banner inside the message area. */
  _showSendError(message) {
    const container = document.getElementById('messagesContainer');
    if (!container) return;

    const banner = document.createElement('div');
    banner.className   = 'message-send-error';
    banner.textContent = message;
    container.appendChild(banner);
    container.scrollTop = container.scrollHeight;

    setTimeout(() => banner.remove(), 4000);
  },

  // ── Sending ──────────────────────────────────────────────────────────────

  async send() {
    const input = document.getElementById('messageInput');
    const text  = input?.value.trim();

    if (!text) return;

    if (!ChatState.currentConversation) {
      this._showSendError('No conversation selected. Please select one first.');
      return;
    }

    if (text.length > 10_000) {
      this._showSendError('Message is too long (max 10,000 characters).');
      return;
    }

    const chatId  = String(ChatState.currentConversation.id);
    const sendBtn = document.getElementById('sendBtn');
    if (sendBtn) sendBtn.disabled = true;

    try {
      const response = await fetch('/api/messages/send', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          chat_id:      parseInt(chatId),
          content:      text,
          message_type: 'text',
        }),
      });

      if (!response.ok) {
        const errData = await response.json();
        throw new Error(errData.message || 'Failed to send message');
      }

      const result      = await response.json();
      const messageData = result.data || result;

      // Optimistic local update — we know this one is ours.
      const message = {
        id:        messageData.message_id || Utils.generateId(),
        text,
        timestamp: (messageData.sent_at || Math.floor(Date.now() / 1000)) * 1000,
        isSent:    true,
        sender_id: this._currentUserId(),
      };

      ChatState.addMessage(chatId, message);

      const conv = ChatState.findConversation(chatId) ?? ChatState.findGroup(chatId);
      if (conv) {
        conv.lastMessage = text;
        conv.timestamp   = Date.now();
      }

      try { ChatState.save(); } catch (e) {
        console.warn('[messages] Could not persist to localStorage:', e);
      }

      this.render(ChatState.getMessages(chatId));
      ChatConversations.render();

      input.value        = '';
      input.style.height = 'auto';

      console.info('[messages] Sent:', messageData.message_id);

      // Re-sync from server shortly after so other clients' messages appear.
      setTimeout(() => this._refreshMessages(chatId), 500);

    } catch (err) {
      console.error('[messages] Send failed:', err);
      this._showSendError(err.message || 'Failed to send message. Please try again.');
      this.render(ChatState.getMessages(chatId));
    } finally {
      if (sendBtn) sendBtn.disabled = text.trim() === '';
    }
  },

  /** Pull the latest messages from the server and update local state. */
  async _refreshMessages(chatId) {
    try {
      const response = await fetch(`/api/messages?chat_id=${chatId}&limit=50`);
      if (!response.ok) return;

      const data     = await response.json();
      const messages = data.data?.messages ?? data.messages ?? [];

      ChatState.messages[chatId] = messages.map(m => this._fromApi(m));

      try { ChatState.save(); } catch (_) {}
      this.render(ChatState.getMessages(chatId));

    } catch (err) {
      console.warn('[messages] Refresh failed:', err);
    }
  },

  // ── Input setup ──────────────────────────────────────────────────────────

  setupInput() {
    const input   = document.getElementById('messageInput');
    const sendBtn = document.getElementById('sendBtn');
    if (!input || !sendBtn) return;

    input.addEventListener('input', () => {
      input.style.height = 'auto';
      input.style.height = Math.min(input.scrollHeight, 120) + 'px';
      sendBtn.disabled   = input.value.trim() === '';
    });

    input.addEventListener('keydown', e => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        ChatMessages.send();
      }
    });

    sendBtn.addEventListener('click', () => ChatMessages.send());
  },

  /** Load messages for a chat from the backend. */
  async loadMessages(chatId) {
    try {
      const response = await fetch(`/api/messages?chat_id=${chatId}&limit=50`);

      if (!response.ok) {
        console.warn('[messages] Load failed:', response.status);
        return;
      }

      const data     = await response.json();
      const messages = data.data?.messages ?? data.messages ?? [];

      ChatState.messages[String(chatId)] = messages.map(m => this._fromApi(m));

      try { ChatState.save(); } catch (_) {}
      this.render(ChatState.getMessages(String(chatId)));

    } catch (err) {
      console.error('[messages] Error loading messages:', err);
    }
  },
};
