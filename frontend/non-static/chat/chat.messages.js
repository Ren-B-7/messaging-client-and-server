/**
 * Chat — Messages
 * Renders the message list, handles sending, and simulates incoming replies.
 * Depends on: Utils, ChatState
 */

const ChatMessages = {

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
  _renderItem({ text, timestamp, isSent }) {
    return `
      <div class="message ${isSent ? 'sent' : 'received'}">
        <div class="message-bubble">${Utils.escapeHtml(text)}</div>
        <div class="message-time">${Utils.formatTime(new Date(timestamp))}</div>
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

    // Auto-dismiss after 4 s.
    setTimeout(() => banner.remove(), 4000);
  },

  // ── Sending ──────────────────────────────────────────────────────────────

  send() {
    const input = document.getElementById('messageInput');
    const text  = input?.value.trim();

    if (!text) return;

    if (!ChatState.currentConversation) {
      this._showSendError('No conversation selected. Please select one first.');
      return;
    }

    // Basic length guard.
    if (text.length > 4000) {
      this._showSendError('Message is too long (max 4,000 characters).');
      return;
    }

    const convId  = ChatState.currentConversation.id;
    const message = {
      id:        Utils.generateId(),
      text,
      timestamp: Date.now(),
      isSent:    true,
    };

    ChatState.addMessage(convId, message);

    // Update conversation preview in the sidebar.
    const conv = ChatState.findConversation(convId)
               ?? ChatState.findGroup?.(convId);
    if (conv) {
      conv.lastMessage = text;
      conv.timestamp   = Date.now();
    }

    try {
      ChatState.save();
    } catch (e) {
      console.warn('[messages] Could not persist message to localStorage:', e);
    }

    this.render(ChatState.getMessages(convId));
    ChatConversations.render();

    // Reset the input field.
    input.value        = '';
    input.style.height = 'auto';
    const sendBtn = document.getElementById('sendBtn');
    if (sendBtn) sendBtn.disabled = true;

    // Demo: simulate a reply after 1 s.
    setTimeout(() => this._simulateReply(), 1000);
  },

  _simulateReply() {
    if (!ChatState.currentConversation) return;

    const replies = [
      'Thanks for your message!',
      'Got it, will get back to you soon.',
      'Sounds good!',
      'Let me think about that.',
      'Absolutely!',
    ];

    const convId  = ChatState.currentConversation.id;
    const message = {
      id:        Utils.generateId(),
      text:      replies[Math.floor(Math.random() * replies.length)],
      timestamp: Date.now(),
      isSent:    false,
    };

    ChatState.addMessage(convId, message);

    const conv = ChatState.findConversation(convId)
               ?? ChatState.findGroup?.(convId);
    if (conv) {
      conv.lastMessage = message.text;
      conv.timestamp   = Date.now();
    }

    try {
      ChatState.save();
    } catch (e) {
      console.warn('[messages] Could not persist reply:', e);
    }

    this.render(ChatState.getMessages(convId));
    ChatConversations.render();
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
};
