/**
 * Chat — Initialiser
 * Loads user data from storage, boots all chat sub-modules in the correct
 * order, fetches conversations from the API on first load, and renders
 * the initial state.
 */

import { EventEmitter } from "../../../static/js/full/utils/events.js";
import Utils from "../../../static/js/full/utils/utils.js";
import ChatState from "./chat.state.js";
import ChatUI from "./chat.ui.js";
import ChatMessages from "./chat.messages.js";
import ChatConversations from "./chat.conversations.js";
import ChatSSE from "./chat.sse.js";
import ChatFiles from "./chat.files.js";

document.addEventListener("DOMContentLoaded", async () => {
    // ── Global Event Bus Listeners ───────────────────────────────────────────

    // SSE Events -> UI/Messages
    EventEmitter.on("sse:connected", (chatId) => {
        console.info("[init] SSE Connected:", chatId);
    });

    EventEmitter.on("sse:history:loaded", ({ chatId, messages }) => {
        if (ChatState.currentConversation?.id === chatId) {
            ChatMessages.render(messages);
        }
    });

    EventEmitter.on("sse:message:received", ({ chatId, message }) => {
        if (ChatState.currentConversation?.id === chatId) {
            ChatMessages.renderOne(message);
        }
        ChatConversations.render();
    });

    EventEmitter.on("sse:message:read", ({ chatId, message_id, reader_id }) => {
        if (ChatState.currentConversation?.id === chatId) {
            ChatMessages.renderReadReceipts(chatId, message_id, reader_id);
        }
    });

    EventEmitter.on("sse:typing:start", (userId) => {
        ChatUI.showTyping(userId);
    });

    EventEmitter.on("sse:typing:stop", (userId) => {
        ChatUI.hideTyping(userId);
    });

    EventEmitter.on("sse:chat:created", () => {
        ChatConversations.refresh();
    });

    EventEmitter.on("sse:status", (status) => {
        const dot = document.getElementById("sseStatusDot");
        const textEl = document.getElementById("statusText");
        if (dot) dot.dataset.status = status;
        if (!textEl) return;

        if (ChatState.currentConversationType === "dm") {
            switch (status) {
                case "connected":
                    textEl.textContent = "Connected";
                    textEl.style.color = "var(--success)";
                    break;
                case "reconnecting":
                    textEl.textContent = "Connecting…";
                    textEl.style.color = "var(--warning)";
                    break;
                default:
                    textEl.textContent = "Disconnected";
                    textEl.style.color = "var(--danger)";
            }
        }
    });

    // UI Events -> SSE/Data
    EventEmitter.on("typing:status", (isTyping) => {
        ChatSSE.sendTyping(isTyping);
    });

    EventEmitter.on("typing:stop", () => {
        ChatSSE.sendTyping(false);
    });

    EventEmitter.on("messages:request:load", (chatId) => {
        ChatSSE.connect(chatId);
    });

    EventEmitter.on("modal:request:open", (id) => {
        ChatUI._openModal(id);
    });

    EventEmitter.on("modal:request:close", (id) => {
        ChatUI._closeModal(id);
    });

    EventEmitter.on("group:updated", (group) => {
        ChatState.save();
        ChatConversations.render();
    });

    EventEmitter.on("group:deleted", () => {
        ChatSSE.disconnect();
    });

    // ── Theme ──────────────────────────────────────────────────────────────────
    if (window.themeManager) {
        window.themeManager.init(["base", "chat"]);
        const chatThemeBtn = document.getElementById("themeToggle");
        window.themeManager.syncIcon(chatThemeBtn);
        chatThemeBtn?.addEventListener("click", () => {
            window.themeManager.toggle();
        });
    }

    // ── Fetch current user from server ─────────────────────────────────────────
    try {
        const res = await fetch("/api/profile");
        if (res.ok) {
            const data = await res.json();
            const profile = data.data ?? data;
            ChatState.currentUser = {
                id: Number(profile.user_id),
                username: profile.username ?? "",
                email: profile.email ?? "",
                isAdmin: profile.is_admin ?? false,
                avatarUrl: profile.avatar_url ?? null,
            };
            Utils.setStorage("user", ChatState.currentUser);
            console.info("[init] Current user:", ChatState.currentUser);
        }
    } catch (e) {
        console.error("[init] Profile fetch failed:", e);
    }

    // ── Update UI with user data ──────────────────────────────────────────────
    const initialsEl = document.getElementById("userInitials");
    const userAvatarEl = document.getElementById("userAvatarImg");

    if (ChatState.currentUser) {
        if (ChatState.currentUser.avatarUrl && userAvatarEl) {
            userAvatarEl.src = ChatState.currentUser.avatarUrl;
            userAvatarEl.style.display = "block";
            if (initialsEl) initialsEl.style.display = "none";
        } else if (initialsEl) {
            initialsEl.textContent = Utils.getInitials(ChatState.currentUser.username);
        }
    }

    // ── Load persisted state ───────────────────────────────────────────────────
    ChatState.load();

    // ── Boot sub-modules ───────────────────────────────────────────────────────
    ChatMessages.setupInput();
    ChatConversations.setupTabs();
    ChatConversations.setupSearch();
    ChatConversations.setupNewButtons();
    ChatUI.setupActionButtons();
    ChatFiles.setupUpload();

    // ── Initial Data Fetch ─────────────────────────────────────────────────────
    await ChatConversations.refresh();

    // ── Restore previous conversation ─────────────────────────────────────────
    if (ChatState.currentConversation) {
        ChatConversations.open(ChatState.currentConversation.id, ChatState.currentConversationType);
    }

    // ── Lifecycle ─────────────────────────────────────────────────────────────
    window.addEventListener("beforeunload", () => {
        ChatSSE.disconnect();
        Utils.removeStorage("messages");
        Utils.removeStorage("conversations");
        Utils.removeStorage("groups");
    });
});
