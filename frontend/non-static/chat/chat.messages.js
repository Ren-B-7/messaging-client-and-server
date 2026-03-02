/**
 * Chat — Messages
 * Renders the message list, handles sending, and integrates with ChatSSE
 * for real-time delivery.  HTTP polling is no longer used — all live updates
 * arrive through the SSE stream managed by chat.sse.js.
 *
 * Depends on: Utils, ChatState, ChatSSE
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
      id:           msg.id,
      text:         msg.content,
      content:      msg.content,
      timestamp:    msg.sent_at * 1000,   // seconds → ms
      isSent:       myId !== null && msg.sender_id === myId,
      sender_id:    msg.sender_id,
      delivered_at: msg.delivered_at,
      read_at:      msg.read_at,
      message_type: msg.message_type,
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

    if (!messages || !messages.length) {
      container.innerHTML = `
        <div class="text-center" style="padding: var(--space-8); color: var(--fg-tertiary);">
          <p>No messages yet</p>
          <p style="font-size: var(--text-sm); margin-top: var(--space-2);">
            Send a message to start the conversation
          </p>
        </div>`;
      return;
    }

    // column-reverse renders the last DOM child at the top visually,
    // so we reverse the array so the newest message ends up at the top.
    const reversed = [...messages].reverse();
    container.innerHTML = reversed.map(msg => this._renderItem(msg)).join('');
    // column-reverse means scrollTop=0 is the newest (top) — reset to top on load
    container.scrollTop = 0;
  },

  /** @returns {string} HTML for a single message bubble. */
  _renderItem({ id, text, content, timestamp, isSent, sender_id, read_at }) {
    const messageText = text || content || '';
    const time        = typeof timestamp === 'number' ? new Date(timestamp) : new Date();
    const myId        = this._currentUserId();
    const sentByMe    = isSent || (myId !== null && sender_id === myId);

    const readTick = sentByMe && read_at
      ? `<span class="message-read-tick" title="Read">✓✓</span>`
      : sentByMe
        ? `<span class="message-read-tick message-read-tick--sent" title="Sent">✓</span>`
        : '';

    return `
      <div class="message ${sentByMe ? 'sent' : 'received'}" data-msg-id="${id ?? ''}">
        <div class="message-bubble">${Utils.escapeHtml(messageText)}</div>
        <div class="message-meta">
          <span class="message-time">${Utils.formatTime(time)}</span>
          ${readTick}
        </div>
      </div>`;
  },

  /**
   * Update read-receipt ticks on an already-rendered message without a
   * full re-render.  Called by ChatSSE when a message_read event arrives.
   *
   * @param {string} chatId
   * @param {number} messageId
   * @param {number} readerId  — unused visually but available for future use
   */
  renderReadReceipts(chatId, messageId, readerId) {
    // Update the in-memory message
    const msgs = ChatState.getMessages(chatId);
    const msg  = msgs.find(m => m.id === messageId);
    if (msg && !msg.read_at) {
      msg.read_at = Math.floor(Date.now() / 1000);
    }

    // Patch the DOM node if it exists (avoid full re-render for one tick change)
    const el = document.querySelector(`[data-msg-id="${messageId}"]`);
    if (!el) return;

    const tick = el.querySelector('.message-read-tick');
    if (tick) {
      tick.textContent = '✓✓';
      tick.title       = 'Read';
      tick.classList.remove('message-read-tick--sent');
    }
  },

  /**
   * Append a single new message bubble without touching existing DOM.
   * Called by ChatSSE for live incoming messages — avoids a full re-render.
   * @param {object} msg  — same shape as objects stored in ChatState.messages
   */
  renderOne(msg) {
    const container = document.getElementById('messagesContainer');
    if (!container) return;

    // If the empty-state placeholder is showing, clear it first
    const placeholder = container.querySelector('.text-center');
    if (placeholder) container.innerHTML = '';

    const html = this._renderItem(msg);
    const el   = document.createElement('div');
    el.innerHTML = html.trim();
    const node = el.firstElementChild;
    if (node) {
      // column-reverse: prepending puts the new message at the visual top
      container.prepend(node);
      // Auto-scroll to top (newest) only if the user is already there (within 120px)
      if (container.scrollTop < 120) {
        container.scrollTop = 0;
      }
    }
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

    setTimeout(() => banner.remove(), 4_000);
  },

  // ── Sending ──────────────────────────────────────────────────────────────

  async send() {
    const input   = document.getElementById('messageInput');
    const text    = input?.value.trim();

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

    // Clear input immediately for snappy UX
    input.value        = '';
    input.style.height = 'auto';

    // Stop any pending typing indicator
    ChatSSE.sendTyping(false);

    try {
      const response = await fetch('/api/messages/send', {
        method:  'POST',
        headers: { 'Content-Type': 'application/json' },
        body:    JSON.stringify({
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

      // Optimistic local update with the server-assigned ID
      const message = {
        id:        messageData.message_id || Utils.generateId(),
        text,
        content:   text,
        timestamp: (messageData.sent_at || Math.floor(Date.now() / 1000)) * 1000,
        isSent:    true,
        sender_id: this._currentUserId(),
      };

      // Add only if SSE hasn't already delivered it (race condition guard)
      const existing = ChatState.getMessages(chatId);
      if (!existing.some(m => m.id === message.id)) {
        ChatState.addMessage(chatId, message);
        this.renderOne(message);   // append the bubble without wiping the DOM
      }

      const conv = ChatState.findConversation(chatId) ?? ChatState.findGroup(chatId);
      if (conv) {
        conv.lastMessage = text;
        conv.timestamp   = Date.now();
      }

      try { ChatState.save(); } catch (e) {
        console.warn('[messages] Could not persist to localStorage:', e);
      }

      ChatConversations.render();  // sidebar preview update only

      console.info('[messages] Sent:', messageData.message_id);

    } catch (err) {
      // Put the text back so the user doesn't lose it
      if (input) input.value = text;
      console.error('[messages] Send failed:', err);
      this._showSendError(err.message || 'Failed to send message. Please try again.');
      this.render(ChatState.getMessages(chatId));
    } finally {
      if (sendBtn) {
        sendBtn.disabled = (input?.value.trim() ?? '') === '';
      }
    }
  },

  // ── Input setup ──────────────────────────────────────────────────────────

  setupInput() {
    const input   = document.getElementById('messageInput');
    const sendBtn = document.getElementById('sendBtn');
    if (!input || !sendBtn) return;

    // Auto-resize textarea
    input.addEventListener('input', () => {
      input.style.height = 'auto';
      input.style.height = Math.min(input.scrollHeight, 120) + 'px';

      const hasText = input.value.trim().length > 0;
      sendBtn.disabled = !hasText;

      // Typing signal driven purely by whether the box has text.
      // No timers, no key tracking — if box has text we're typing, if empty we're not.
      if (ChatState.currentConversation) {
        ChatSSE.sendTyping(hasText);
      }
    });

    // Send on Enter (Shift+Enter inserts newline)
    input.addEventListener('keydown', e => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        ChatMessages.send();
      }
    });

    sendBtn.addEventListener('click', () => ChatMessages.send());
  },

  /**
   * Load messages for a chat from the backend over HTTP (one-shot, pre-SSE).
   * Once the SSE stream connects it replays history automatically, so this
   * is a fallback shown while the SSE handshake is in progress.
   *
   * @param {string|number} chatId
   */
  async loadMessages(chatId) {
    // Immediately show whatever we have cached while waiting for SSE history
    const cached = ChatState.getMessages(String(chatId));
    if (cached.length) {
      this.render(cached);
    }

    // Connect (or switch) the SSE stream — it will replay history
    ChatSSE.connect(chatId);
  },
};
