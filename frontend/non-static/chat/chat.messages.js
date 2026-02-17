/**
 * Chat — Messages
 * Renders the message list, handles sending, and simulates incoming replies.
 * Depends on: Utils, ChatState
 */

const ChatMessages = {
  // ─── Rendering ────────────────────────────────────────────────────────────

  /**
   * Render all messages for the active conversation into #messagesContainer.
   * @param {object[]} messages
   */
  render(messages) {
    const container = document.getElementById('messagesContainer');
    if (!container) return;

    if (messages.length === 0) {
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
  _renderItem(message) {
    const { text, timestamp, isSent } = message;
    const timeStr = Utils.formatTime(new Date(timestamp));
    return `
      <div class="message ${isSent ? 'sent' : 'received'}">
        <div class="message-bubble">${Utils.escapeHtml(text)}</div>
        <div class="message-time">${timeStr}</div>
      </div>`;
  },

  // ─── Sending ──────────────────────────────────────────────────────────────

  /** Read the textarea, build a message, persist it, and update the UI. */
  send() {
    const input = document.getElementById('messageInput');
    const text  = input?.value.trim();
    if (!text || !ChatState.currentConversation) return;

    const message = {
      id:        Utils.generateId(),
      text,
      timestamp: Date.now(),
      isSent:    true,
    };

    const convId = ChatState.currentConversation.id;
    ChatState.addMessage(convId, message);

    // Update the conversation's last-message preview.
    const conv = ChatState.findConversation(convId);
    if (conv) {
      conv.lastMessage = text;
      conv.timestamp   = Date.now();
    }

    ChatState.save();
    this.render(ChatState.getMessages(convId));
    ChatConversations.render();

    // Reset input.
    input.value              = '';
    input.style.height       = 'auto';
    const sendBtn            = document.getElementById('sendBtn');
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

    const message = {
      id:        Utils.generateId(),
      text:      replies[Math.floor(Math.random() * replies.length)],
      timestamp: Date.now(),
      isSent:    false,
    };

    const convId = ChatState.currentConversation.id;
    ChatState.addMessage(convId, message);

    const conv = ChatState.findConversation(convId);
    if (conv) {
      conv.lastMessage = message.text;
      conv.timestamp   = Date.now();
    }

    ChatState.save();
    this.render(ChatState.getMessages(convId));
    ChatConversations.render();
  },

  // ─── Input setup ──────────────────────────────────────────────────────────

  /** Wire the textarea auto-resize, Enter-to-send, and send button. */
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
