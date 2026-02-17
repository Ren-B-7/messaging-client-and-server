/**
 * Chat — Shared State & Persistence
 * Single source of truth for conversation + message data.
 * All other chat modules read from and write to ChatState.
 * Depends on: Utils
 */

const ChatState = {
  currentConversation: null,
  conversations: [],
  messages: {},

  /** Load conversations and messages from localStorage. */
  load() {
    this.conversations = Utils.getStorage('conversations') || [];
    this.messages      = Utils.getStorage('messages')      || {};
  },

  /** Persist conversations and messages to localStorage. */
  save() {
    Utils.setStorage('conversations', this.conversations);
    Utils.setStorage('messages',      this.messages);
  },

  /** @returns {object|undefined} */
  findConversation(id) {
    return this.conversations.find(c => c.id === id);
  },

  /** Prepend a new conversation object and initialise its message list. */
  addConversation(conversation) {
    this.conversations.unshift(conversation);
    this.messages[conversation.id] = [];
  },

  /** Append a message to a conversation's message list. */
  addMessage(conversationId, message) {
    if (!this.messages[conversationId]) this.messages[conversationId] = [];
    this.messages[conversationId].push(message);
  },

  /** Return all messages for a conversation (or empty array). */
  getMessages(conversationId) {
    return this.messages[conversationId] || [];
  },
};
