/**
 * Chat — Messages
 * Renders the message list, handles sending, and integrates with ChatSSE
 * for real-time delivery.
 *
 * Depends on: Utils, ChatState, DOM, EventEmitter
 */

import Utils from "../../../static/js/full/utils/utils.js";
import { DOM } from "../../../static/js/full/utils/dom.js";
import { EventEmitter } from "../../../static/js/full/utils/events.js";
import ChatState from "./chat.state.js";

export const ChatMessages = {
    // ── Helpers ──────────────────────────────────────────────────────────────

    _currentUserId() {
        return ChatState.currentUser?.id ?? null;
    },

    _fromApi(msg) {
        const myId = this._currentUserId();
        return {
            id: msg.id,
            text: msg.content,
            content: msg.content,
            timestamp: msg.sent_at * 1000,
            isSent: myId !== null && msg.sender_id === myId,
            sender_id: msg.sender_id,
            delivered_at: msg.delivered_at,
            read_at: msg.read_at,
            message_type: msg.message_type,
        };
    },

    // ── Rendering ────────────────────────────────────────────────────────────

    render(messages) {
        const container = document.getElementById("messagesContainer");
        if (!container) return;

        if (!messages || !messages.length) {
            DOM.clear(
                container,
                DOM.create(
                    "div",
                    {
                        className: "text-center",
                        style: { padding: "var(--space-8)", color: "var(--fg-tertiary)" },
                    },
                    [
                        DOM.create("p", {}, "No messages yet"),
                        DOM.create(
                            "p",
                            { style: { fontSize: "var(--text-sm)", marginTop: "var(--space-2)" } },
                            "Send a message to start the conversation"
                        ),
                    ]
                )
            );
            return;
        }

        const reversed = [...messages].reverse();
        DOM.clear(
            container,
            reversed.map((msg) => this._renderItem(msg))
        );
        container.scrollTop = 0;
    },

    _renderItem(msg) {
        const { id, text, content, timestamp, isSent, sender_id, read_at } = msg;
        const messageText = text || content || "";
        const time = typeof timestamp === "number" ? new Date(timestamp) : new Date();
        const myId = this._currentUserId();
        const sentByMe = isSent || (myId !== null && sender_id === myId);

        const readTick = sentByMe
            ? DOM.create(
                  "span",
                  {
                      className: `message-read-tick ${read_at ? "" : "message-read-tick--sent"}`,
                      title: read_at ? "Read" : "Sent",
                  },
                  read_at ? "✓✓" : "✓"
              )
            : null;

        return DOM.create(
            "div",
            {
                className: `message ${sentByMe ? "sent" : "received"}`,
                dataset: { msgId: id ?? "" },
            },
            [
                DOM.create("div", { className: "message-bubble" }, Utils.escapeHtml(messageText)),
                DOM.create("div", { className: "message-meta" }, [
                    DOM.create("span", { className: "message-time" }, Utils.formatTime(time)),
                    readTick,
                ]),
            ]
        );
    },

    renderReadReceipts(chatId, messageId, readerId) {
        const msgs = ChatState.getMessages(chatId);
        const msg = msgs.find((m) => m.id === messageId);
        if (msg && !msg.read_at) {
            msg.read_at = Math.floor(Date.now() / 1000);
        }

        const el = document.querySelector(`[data-msg-id="${messageId}"]`);
        if (!el) return;

        const tick = el.querySelector(".message-read-tick");
        if (tick) {
            tick.textContent = "✓✓";
            tick.title = "Read";
            tick.classList.remove("message-read-tick--sent");
        }
    },

    renderOne(msg) {
        const container = document.getElementById("messagesContainer");
        if (!container) return;

        const placeholder = container.querySelector(".text-center");
        if (placeholder) DOM.clear(container);

        const node = this._renderItem(msg);
        container.prepend(node);

        if (container.scrollTop < 120) {
            container.scrollTop = 0;
        }
    },

    _showSendError(message) {
        const container = document.getElementById("messagesContainer");
        if (!container) return;

        const banner = DOM.create("div", { className: "message-send-error" }, message);
        container.appendChild(banner);
        container.scrollTop = container.scrollHeight;

        setTimeout(() => banner.remove(), 4000);
    },

    // ── Sending ──────────────────────────────────────────────────────────────

    async send() {
        const input = document.getElementById("messageInput");
        const text = input?.value.trim();

        if (!text) return;

        if (!ChatState.currentConversation) {
            this._showSendError("No conversation selected. Please select one first.");
            return;
        }

        if (text.length > 10000) {
            this._showSendError("Message is too long (max 10,000 characters).");
            return;
        }

        const chatId = String(ChatState.currentConversation.id);
        const sendBtn = document.getElementById("sendBtn");
        if (sendBtn) sendBtn.disabled = true;

        input.value = "";
        input.style.height = "auto";

        EventEmitter.emit("typing:stop");

        try {
            const response = await fetch("/api/messages/send", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    chat_id: parseInt(chatId, 10),
                    content: text,
                    message_type: "text",
                }),
            });

            if (!response.ok) {
                const errData = await response.json();
                throw new Error(errData.message || "Failed to send message");
            }

            const result = await response.json();
            const messageData = result.data || result;

            const message = {
                id: messageData.message_id || Utils.generateId(),
                text,
                content: text,
                timestamp: (messageData.sent_at || Math.floor(Date.now() / 1000)) * 1000,
                isSent: true,
                sender_id: this._currentUserId(),
            };

            const existing = ChatState.getMessages(chatId);
            if (!existing.some((m) => m.id === message.id)) {
                ChatState.addMessage(chatId, message);
                this.renderOne(message);
            }

            const conv = ChatState.findConversation(chatId) ?? ChatState.findGroup(chatId);
            if (conv) {
                conv.lastMessage = text;
                conv.timestamp = Date.now();
                EventEmitter.emit("conversation:updated", conv);
            }

            ChatState.save();
        } catch (err) {
            if (input) input.value = text;
            console.error("[messages] Send failed:", err);
            this._showSendError(err.message || "Failed to send message. Please try again.");
            this.render(ChatState.getMessages(chatId));
        } finally {
            if (sendBtn) {
                sendBtn.disabled = (input?.value.trim() ?? "") === "";
            }
        }
    },

    // ── Input setup ──────────────────────────────────────────────────────────

    setupInput() {
        const input = document.getElementById("messageInput");
        const sendBtn = document.getElementById("sendBtn");
        if (!input || !sendBtn) return;

        input.addEventListener("input", () => {
            input.style.height = "auto";
            input.style.height = Math.min(input.scrollHeight, 120) + "px";

            const hasText = input.value.trim().length > 0;
            sendBtn.disabled = !hasText;

            if (ChatState.currentConversation) {
                EventEmitter.emit("typing:status", hasText);
            }
        });

        input.addEventListener("keydown", (e) => {
            if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                this.send();
            }
        });

        sendBtn.addEventListener("click", () => this.send());
    },

    async loadMessages(chatId) {
        const cached = ChatState.getMessages(String(chatId));
        if (cached.length) {
            this.render(cached);
        }

        EventEmitter.emit("messages:request:load", chatId);
    },
};

export default ChatMessages;
