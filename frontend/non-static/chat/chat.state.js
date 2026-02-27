/**
 * Chat — Shared State & Persistence
 * Single source of truth for conversation, group, and message data.
 * All other chat modules read from and write to ChatState.
 * Depends on: Utils
 */

const ChatState = {
  currentUser:             null,   // { id, username } — fetched from /api/profile on init
  currentConversation:     null,
  currentConversationType: 'dm',   // 'dm' | 'groups'
  conversations:           [],     // direct message threads
  groups:                  [],     // group chats
  messages:                {},

  /** Load all persisted state from localStorage. */
  load() {
    this.conversations = Utils.getStorage('conversations') || [];
    this.groups        = Utils.getStorage('groups')        || [];
    this.messages      = Utils.getStorage('messages')      || {};
  },

  /** Persist all state to localStorage. */
  save() {
    Utils.setStorage('conversations', this.conversations);
    Utils.setStorage('groups',        this.groups);
    Utils.setStorage('messages',      this.messages);
  },

  // ── Conversations ─────────────────────────────────────────────────────────

  /** @returns {object|undefined} */
  findConversation(id) {
    return this.conversations.find(c => c.id === id);
  },

  /** Prepend a new DM conversation and initialise its message list. */
  addConversation(conversation) {
    this.conversations.unshift(conversation);
    this.messages[conversation.id] = [];
  },

  // ── Groups ────────────────────────────────────────────────────────────────

  /** @returns {object|undefined} */
  findGroup(id) {
    return this.groups.find(g => g.id === id);
  },

  /** Prepend a new group and initialise its message list. */
  addGroup(group) {
    this.groups.unshift(group);
    this.messages[group.id] = [];
  },

  // ── Messages ──────────────────────────────────────────────────────────────

  /** Append a message to any conversation or group by id. */
  addMessage(conversationId, message) {
    if (!this.messages[conversationId]) this.messages[conversationId] = [];
    this.messages[conversationId].push(message);
  },

  /** Return all messages for a conversation or group (or empty array). */
  getMessages(id) {
    return this.messages[id] || [];
  },
};
