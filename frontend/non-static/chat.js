/**
 * Chat Module
 * Handles messaging functionality
 */

const ChatModule = {
  currentConversation: null,
  conversations: [],
  messages: {},

  /**
   * Initialize chat module
   */
  init() {
    this.checkAuthentication();
    this.loadUserData();
    this.setupMessageInput();
    this.setupConversationSearch();
    this.setupNewChatButton();
    this.setupChatActions();
    this.loadConversations();
  },

  /**
   * Check if user is authenticated
   */
  checkAuthentication() {
    const user = Utils.getStorage('user');
    if (!user || !user.loggedIn) {
      window.location.href = '/';
      return;
    }
  },

  /**
   * Load user data and update UI
   */
  loadUserData() {
    const user = Utils.getStorage('user');
    if (!user) return;

    // Update user initials in avatar
    const userInitials = document.getElementById('userInitials');
    if (userInitials) {
      userInitials.textContent = Utils.getInitials(user.name || user.email);
    }
  },

  /**
   * Load conversations from storage
   */
  loadConversations() {
    this.conversations = Utils.getStorage('conversations') || [];
    this.renderConversations();

    // If there are no conversations, show empty state
    if (this.conversations.length === 0) {
      this.showEmptyState();
    }
  },

  /**
   * Render conversations list
   */
  renderConversations() {
    const conversationsList = document.getElementById('conversationsList');
    if (!conversationsList) return;

    if (this.conversations.length === 0) {
      conversationsList.innerHTML = `
        <div class="text-center" style="padding: var(--space-8); color: var(--fg-tertiary);">
          <p>No conversations yet</p>
          <p style="font-size: var(--text-sm); margin-top: var(--space-2);">
            Start a new chat to begin messaging
          </p>
        </div>
      `;
      return;
    }

    conversationsList.innerHTML = this.conversations
      .map((conv) => this.renderConversationItem(conv))
      .join('');

    // Add click handlers
    document.querySelectorAll('.conversation-item').forEach((item) => {
      item.addEventListener('click', () => {
        const convId = item.dataset.conversationId;
        this.openConversation(convId);
      });
    });
  },

  /**
   * Render single conversation item
   */
  renderConversationItem(conversation) {
    const { id, name, avatar, lastMessage, timestamp, unreadCount, isOnline } = conversation;
    const timeStr = Utils.formatRelativeTime(new Date(timestamp));
    const isActive = this.currentConversation?.id === id;

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
      </div>
    `;
  },

  /**
   * Open a conversation
   */
  openConversation(conversationId) {
    const conversation = this.conversations.find((c) => c.id === conversationId);
    if (!conversation) return;

    this.currentConversation = conversation;

    // Update active state in sidebar
    document.querySelectorAll('.conversation-item').forEach((item) => {
      item.classList.toggle('active', item.dataset.conversationId === conversationId);
    });

    // Hide empty state and show chat view
    this.hideEmptyState();

    // Update chat header
    this.updateChatHeader(conversation);

    // Load and display messages
    this.loadMessages(conversationId);

    // Mark as read
    this.markAsRead(conversationId);
  },

  /**
   * Update chat header
   */
  updateChatHeader(conversation) {
    document.getElementById('chatName').textContent = conversation.name;
    document.getElementById('chatAvatar').textContent = 
      conversation.avatar || Utils.getInitials(conversation.name);
    
    const statusDot = document.getElementById('statusDot');
    const statusText = document.getElementById('statusText');
    
    if (conversation.isOnline) {
      statusDot.className = 'status-dot status-online';
      statusText.textContent = 'Online';
    } else {
      statusDot.className = 'status-dot status-offline';
      statusText.textContent = 'Offline';
    }
  },

  /**
   * Load messages for conversation
   */
  loadMessages(conversationId) {
    const messages = this.messages[conversationId] || [];
    this.renderMessages(messages);
  },

  /**
   * Render messages
   */
  renderMessages(messages) {
    const container = document.getElementById('messagesContainer');
    if (!container) return;

    if (messages.length === 0) {
      container.innerHTML = `
        <div class="text-center" style="padding: var(--space-8); color: var(--fg-tertiary);">
          <p>No messages yet</p>
          <p style="font-size: var(--text-sm); margin-top: var(--space-2);">
            Send a message to start the conversation
          </p>
        </div>
      `;
      return;
    }

    container.innerHTML = messages
      .map((msg) => this.renderMessage(msg))
      .join('');

    // Scroll to bottom
    container.scrollTop = container.scrollHeight;
  },

  /**
   * Render single message
   */
  renderMessage(message) {
    const { text, timestamp, isSent } = message;
    const timeStr = Utils.formatTime(new Date(timestamp));

    return `
      <div class="message ${isSent ? 'sent' : 'received'}">
        <div class="message-bubble">${Utils.escapeHtml(text)}</div>
        <div class="message-time">${timeStr}</div>
      </div>
    `;
  },

  /**
   * Setup message input
   */
  setupMessageInput() {
    const messageInput = document.getElementById('messageInput');
    const sendBtn = document.getElementById('sendBtn');

    if (!messageInput || !sendBtn) return;

    // Auto-resize textarea
    messageInput.addEventListener('input', () => {
      messageInput.style.height = 'auto';
      messageInput.style.height = Math.min(messageInput.scrollHeight, 120) + 'px';
      
      // Enable/disable send button
      sendBtn.disabled = messageInput.value.trim() === '';
    });

    // Send on Enter (Shift+Enter for new line)
    messageInput.addEventListener('keypress', (e) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        this.sendMessage();
      }
    });

    // Send button click
    sendBtn.addEventListener('click', () => this.sendMessage());
  },

  /**
   * Send message
   */
  sendMessage() {
    const messageInput = document.getElementById('messageInput');
    const text = messageInput?.value.trim();

    if (!text || !this.currentConversation) return;

    const message = {
      id: Utils.generateId(),
      text,
      timestamp: Date.now(),
      isSent: true,
    };

    // Add to messages
    if (!this.messages[this.currentConversation.id]) {
      this.messages[this.currentConversation.id] = [];
    }
    this.messages[this.currentConversation.id].push(message);

    // Update conversation last message
    const conversation = this.conversations.find(
      (c) => c.id === this.currentConversation.id
    );
    if (conversation) {
      conversation.lastMessage = text;
      conversation.timestamp = Date.now();
    }

    // Save to storage
    this.saveData();

    // Update UI
    this.renderMessages(this.messages[this.currentConversation.id]);
    this.renderConversations();

    // Clear input
    messageInput.value = '';
    messageInput.style.height = 'auto';
    document.getElementById('sendBtn').disabled = true;

    // Simulate received message (for demo)
    setTimeout(() => this.simulateReceivedMessage(), 1000);
  },

  /**
   * Simulate received message (for demo purposes)
   */
  simulateReceivedMessage() {
    if (!this.currentConversation) return;

    const responses = [
      'Thanks for your message!',
      'Got it, will get back to you soon.',
      'Sounds good!',
      'Let me think about that.',
      'Absolutely!',
    ];

    const message = {
      id: Utils.generateId(),
      text: responses[Math.floor(Math.random() * responses.length)],
      timestamp: Date.now(),
      isSent: false,
    };

    this.messages[this.currentConversation.id].push(message);

    // Update conversation
    const conversation = this.conversations.find(
      (c) => c.id === this.currentConversation.id
    );
    if (conversation) {
      conversation.lastMessage = message.text;
      conversation.timestamp = Date.now();
    }

    // Save and update UI
    this.saveData();
    this.renderMessages(this.messages[this.currentConversation.id]);
    this.renderConversations();
  },

  /**
   * Setup conversation search
   */
  setupConversationSearch() {
    const searchInput = document.getElementById('conversationSearch');
    if (!searchInput) return;

    searchInput.addEventListener(
      'input',
      Utils.debounce((e) => {
        const query = e.target.value.toLowerCase();
        this.filterConversations(query);
      }, 300)
    );
  },

  /**
   * Filter conversations by search query
   */
  filterConversations(query) {
    document.querySelectorAll('.conversation-item').forEach((item) => {
      const name = item.querySelector('.conversation-name')?.textContent.toLowerCase() || '';
      const matches = name.includes(query);
      item.style.display = matches ? 'flex' : 'none';
    });
  },

  /**
   * Setup new chat button
   */
  setupNewChatButton() {
    const newChatBtn = document.getElementById('newChatBtn');
    if (!newChatBtn) return;

    newChatBtn.addEventListener('click', () => {
      const name = prompt('Enter contact name:');
      if (name) {
        this.createNewConversation(name);
      }
    });
  },

  /**
   * Create new conversation
   */
  createNewConversation(name) {
    const conversation = {
      id: Utils.generateId(),
      name,
      avatar: Utils.getInitials(name),
      lastMessage: '',
      timestamp: Date.now(),
      unreadCount: 0,
      isOnline: false,
    };

    this.conversations.unshift(conversation);
    this.messages[conversation.id] = [];
    
    this.saveData();
    this.renderConversations();
    this.openConversation(conversation.id);
  },

  /**
   * Setup chat actions (call buttons, etc.)
   */
  setupChatActions() {
    document.getElementById('voiceCallBtn')?.addEventListener('click', () => {
      if (window.PlatformConfig?.hasFeature('voiceCall')) {
        alert('Voice call feature - Coming soon!');
      } else {
        alert('Voice calls are not supported on this platform');
      }
    });

    document.getElementById('videoCallBtn')?.addEventListener('click', () => {
      if (window.PlatformConfig?.hasFeature('videoCall')) {
        alert('Video call feature - Coming soon!');
      } else {
        alert('Video calls are not supported on this platform');
      }
    });

    document.getElementById('attachFileBtn')?.addEventListener('click', () => {
      if (window.PlatformConfig?.hasFeature('fileUpload')) {
        alert('File attachment feature - Coming soon!');
      } else {
        alert('File uploads are not supported on this platform');
      }
    });
  },

  /**
   * Mark conversation as read
   */
  markAsRead(conversationId) {
    const conversation = this.conversations.find((c) => c.id === conversationId);
    if (conversation && conversation.unreadCount > 0) {
      conversation.unreadCount = 0;
      this.saveData();
      this.renderConversations();
    }
  },

  /**
   * Show empty chat state
   */
  showEmptyState() {
    document.getElementById('emptyChatState').style.display = 'flex';
    document.getElementById('activeChatView').style.display = 'none';
  },

  /**
   * Hide empty chat state
   */
  hideEmptyState() {
    document.getElementById('emptyChatState').style.display = 'none';
    document.getElementById('activeChatView').style.display = 'flex';
  },

  /**
   * Save data to storage
   */
  saveData() {
    Utils.setStorage('conversations', this.conversations);
    Utils.setStorage('messages', this.messages);
  },
};

// Initialize on DOM ready
document.addEventListener('DOMContentLoaded', () => {
  ChatModule.init();
});