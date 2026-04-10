/**
 * Chat — SSE (Server-Sent Events) Manager
 *
 * Manages a single persistent EventSource connection per open chat.
 * Switching chats tears down the old connection and opens a fresh one.
 *
 * Depends on: Utils, ChatState, EventEmitter
 */

import Utils from "../../../static/js/full/utils/utils.js";
import { EventEmitter } from "../../../static/js/full/utils/events.js";
import ChatState from "./chat.state.js";

export const ChatSSE = {
    // ── Private state ─────────────────────────────────────────────────────────

    _source: null,
    _chatId: null,
    _retryCount: 0,
    _retryTimer: null,
    _replayBuffer: [],
    _inHistory: false,
    _lastTyping: null,

    MAX_RETRIES: 8,
    BASE_DELAY: 1000,

    // ── Connection lifecycle ──────────────────────────────────────────────────

    connect(chatId) {
        if (this._source && this._chatId === String(chatId)) return;
        this.disconnect();

        this._chatId = String(chatId);
        this._retryCount = 0;
        this._replayBuffer = [];
        this._inHistory = false;

        this._open();
    },

    disconnect() {
        if (this._retryTimer) {
            clearTimeout(this._retryTimer);
            this._retryTimer = null;
        }
        if (this._source) {
            this._source.close();
            this._source = null;
        }
        this._chatId = null;
        this._lastTyping = null;
        this._setStatus("disconnected");
    },

    _open() {
        if (!this._chatId) return;

        const url = `/api/stream?chat_id=${encodeURIComponent(this._chatId)}`;
        this._source = new EventSource(url, { withCredentials: true });

        this._source.addEventListener("connected", (e) => this._onConnected(e));
        this._source.addEventListener("history_start", (e) => this._onHistoryStart(e));
        this._source.addEventListener("history_message", (e) => this._onHistoryMessage(e));
        this._source.addEventListener("history_end", (e) => this._onHistoryEnd(e));
        this._source.addEventListener("message_sent", (e) => this._onMessageSent(e));
        this._source.addEventListener("message_read", (e) => this._onMessageRead(e));
        this._source.addEventListener("typing", (e) => this._onTyping(e));
        this._source.addEventListener("chat_created", (e) => this._onChatCreated(e));
        this._source.addEventListener("reconnect", (e) => this._onReconnectHint(e));
        this._source.onerror = (e) => this._onError(e);
    },

    // ── Event handlers ────────────────────────────────────────────────────────

    _onConnected() {
        this._retryCount = 0;
        this._setStatus("connected");
        EventEmitter.emit("sse:connected", this._chatId);
    },

    _onHistoryStart(e) {
        this._inHistory = true;
        this._replayBuffer = [];
        const payload = this._parse(e);
        console.info("[sse] History start — expecting", payload?.count ?? "?", "messages");
    },

    _onHistoryMessage(e) {
        if (!this._inHistory) return;
        const msg = this._parse(e);
        if (msg) this._replayBuffer.push(msg);
    },

    _onHistoryEnd() {
        this._inHistory = false;
        if (!this._chatId) return;

        const myId = ChatState.currentUser?.id ?? null;
        const messages = this._replayBuffer.map((msg) => ({
            id: msg.id,
            text: msg.content,
            content: msg.content,
            timestamp: (msg.sent_at ?? 0) * 1000,
            isSent: myId !== null && msg.sender_id === myId,
            sender_id: msg.sender_id,
            delivered_at: msg.delivered_at,
            read_at: msg.read_at,
            message_type: msg.message_type,
        }));

        messages.reverse();
        ChatState.messages[this._chatId] = messages;
        ChatState.save();

        EventEmitter.emit("sse:history:loaded", { chatId: this._chatId, messages });
        this._replayBuffer = [];
    },

    _onMessageSent(e) {
        const msg = this._parse(e);
        if (!msg || !this._chatId) return;

        const myId = ChatState.currentUser?.id ?? null;
        const isSent = myId !== null && msg.sender_id === myId;

        const existing = ChatState.getMessages(this._chatId);
        if (existing.some((m) => m.id === msg.id)) return;

        const normalized = {
            id: msg.id,
            text: msg.content,
            content: msg.content,
            timestamp: (msg.sent_at ?? 0) * 1000,
            isSent,
            sender_id: msg.sender_id,
            message_type: msg.message_type,
        };

        ChatState.addMessage(this._chatId, normalized);

        const conv = ChatState.findConversation(this._chatId) ?? ChatState.findGroup(this._chatId);
        if (conv) {
            conv.lastMessage = msg.content;
            conv.timestamp = Date.now();
            if (!isSent && ChatState.currentConversation?.id !== this._chatId) {
                conv.unreadCount = (conv.unreadCount ?? 0) + 1;
            }
        }

        ChatState.save();
        EventEmitter.emit("sse:message:received", { chatId: this._chatId, message: normalized });
    },

    _onMessageRead(e) {
        const payload = this._parse(e);
        if (!payload || !this._chatId) return;

        const msgs = ChatState.getMessages(this._chatId);
        const msg = msgs.find((m) => m.id === payload.message_id);
        if (msg) {
            msg.read_at = payload.read_at;
            ChatState.save();
        }

        EventEmitter.emit("sse:message:read", { chatId: this._chatId, ...payload });
    },

    _typingTimers: new Map(),

    _onTyping(e) {
        const payload = this._parse(e);
        if (!payload) return;

        const { chat_id, user_id, is_typing } = payload;
        if (String(chat_id) !== this._chatId) return;
        if (user_id === ChatState.currentUser?.id) return;

        const key = `${chat_id}:${user_id}`;

        if (this._typingTimers.has(key)) {
            clearTimeout(this._typingTimers.get(key));
            this._typingTimers.delete(key);
        }

        if (is_typing) {
            EventEmitter.emit("sse:typing:start", user_id);
            this._typingTimers.set(
                key,
                setTimeout(() => {
                    EventEmitter.emit("sse:typing:stop", user_id);
                    this._typingTimers.delete(key);
                }, 5000)
            );
        } else {
            EventEmitter.emit("sse:typing:stop", user_id);
        }
    },

    _onChatCreated(e) {
        EventEmitter.emit("sse:chat:created", this._parse(e));
    },

    _onReconnectHint(e) {
        console.warn("[sse] Server requested reconnect:", this._parse(e));
        this._scheduleRetry();
    },

    _onError() {
        if (this._source?.readyState === EventSource.CLOSED) {
            this._scheduleRetry();
        }
    },

    _scheduleRetry() {
        if (this._retryTimer || !this._chatId) return;
        if (this._retryCount >= this.MAX_RETRIES) {
            this._setStatus("failed");
            return;
        }

        const delay = Math.min(this.BASE_DELAY * 2 ** this._retryCount, 128000);
        this._retryCount++;
        this._setStatus("reconnecting");

        this._retryTimer = setTimeout(() => {
            this._retryTimer = null;
            if (this._source) {
                this._source.close();
                this._source = null;
            }
            this._open();
        }, delay);
    },

    sendTyping(isTyping) {
        if (!this._chatId || this._lastTyping === isTyping) return;
        this._lastTyping = isTyping;

        fetch("/api/typing", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ chat_id: parseInt(this._chatId, 10), is_typing: isTyping }),
        }).catch((e) => console.warn("[sse] Typing POST failed:", e));
    },

    _setStatus(status) {
        EventEmitter.emit("sse:status", status);
    },

    _parse(e) {
        try {
            return JSON.parse(e.data);
        } catch (_) {
            return null;
        }
    },
};

export default ChatSSE;
