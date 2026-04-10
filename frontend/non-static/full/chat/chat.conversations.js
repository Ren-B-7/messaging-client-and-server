/**
 * Chat — Conversations
 * Renders the sidebar DM and Groups tabs, handles opening a conversation,
 * search/filter, refreshing from the API, and creating new DMs / groups
 * via modals.
 *
 * Depends on: Utils, ChatState, ChatUI, DOM, EventEmitter
 */

import Utils from "../../../static/js/full/utils/utils.js";
import { DOM } from "../../../static/js/full/utils/dom.js";
import { EventEmitter } from "../../../static/js/full/utils/events.js";
import ChatState from "./chat.state.js";
import ChatUI from "./chat.ui.js";

export const ChatConversations = {
    activeTab: "dm", // 'dm' | 'groups'

    // ─── Tab switching ────────────────────────────────────────────────────────

    setupTabs() {
        document.querySelectorAll(".panel-tab").forEach((btn) => {
            btn.addEventListener("click", () => {
                this._switchTab(btn.dataset.tab);
            });
        });
    },

    _switchTab(tab) {
        this.activeTab = tab;
        document.querySelectorAll(".panel-tab").forEach((btn) => {
            btn.classList.toggle("active", btn.dataset.tab === tab);
        });

        const newBtn = document.getElementById("newChatBtn");
        if (newBtn) {
            newBtn.title = tab === "dm" ? "New Message" : "New Group";
        }

        this.refresh();
        EventEmitter.emit("tab:changed", tab);
    },

    // ─── Rendering ───────────────────────────────────────────────────────────

    render() {
        const items = this.activeTab === "dm" ? ChatState.conversations : ChatState.groups;
        this._renderList(items, this.activeTab);
    },

    _renderList(items, type) {
        const list = document.getElementById("conversationsList");
        if (!list) return;

        if (!items.length) {
            const [title, sub] =
                type === "dm"
                    ? ["No conversations yet", "Start a new chat to begin messaging"]
                    : ["No groups yet", "Create a group to get started"];

            DOM.clear(
                list,
                DOM.create(
                    "div",
                    {
                        className: "text-center",
                        style: { padding: "var(--space-8)", color: "var(--fg-tertiary)" },
                    },
                    [
                        DOM.create("p", {}, title),
                        DOM.create(
                            "p",
                            { style: { fontSize: "var(--text-sm)", marginTop: "var(--space-2)" } },
                            sub
                        ),
                    ]
                )
            );
            return;
        }

        DOM.clear(
            list,
            items.map((item) => this._renderItem(item, type))
        );
    },

    _renderItem(conv, type) {
        const { id, name, lastMessage, timestamp, unreadCount, avatarUrl } = conv;
        const timeStr = Utils.formatRelativeTime(new Date(timestamp));
        const isActive = ChatState.currentConversation?.id === id;
        const prefix = type === "groups" ? "# " : "";

        return DOM.create(
            "div",
            {
                className: `conversation-item ${isActive ? "active" : ""} ${unreadCount > 0 ? "unread" : ""}`,
                dataset: { id, type },
                onclick: () => this.open(id, type),
            },
            [
                DOM.create("div", { className: "avatar-wrapper" }, [
                    type === "dm" && avatarUrl
                        ? DOM.create("img", {
                              className: "avatar avatar-sm avatar-img",
                              src: avatarUrl,
                              alt: name,
                              onerror: (e) => {
                                  e.target.style.display = "none";
                                  e.target.nextElementSibling.style.display = "flex";
                              },
                          })
                        : null,
                    DOM.create(
                        "div",
                        {
                            className: "avatar avatar-sm",
                            style: type === "dm" && avatarUrl ? { display: "none" } : {},
                        },
                        Utils.getInitials(name)
                    ),
                ]),
                DOM.create("div", { className: "conversation-content" }, [
                    DOM.create(
                        "div",
                        { className: "conversation-name" },
                        `${prefix}${Utils.escapeHtml(name)}`
                    ),
                    DOM.create(
                        "div",
                        { className: "conversation-preview" },
                        Utils.escapeHtml(lastMessage || "No messages yet")
                    ),
                ]),
                unreadCount > 0
                    ? DOM.create("div", { className: "unread-count" }, unreadCount)
                    : DOM.create("div", { className: "conversation-time" }, timeStr),
            ]
        );
    },

    // ─── Open / mark read ────────────────────────────────────────────────────

    open(id, type = this.activeTab) {
        const items = type === "groups" ? ChatState.groups : ChatState.conversations;
        const conv = items.find((c) => c.id === id);
        if (!conv) return;

        ChatState.setCurrentConversation(conv, type);
        ChatState.save();

        this.render(); // Update active state in sidebar

        ChatUI.hideEmptyState();
        ChatUI.updateHeader(conv, type);

        this._markAsRead(id, type);
        EventEmitter.emit("conversation:opened", { id, type });
    },

    _markAsRead(id, type) {
        const items = type === "groups" ? ChatState.groups : ChatState.conversations;
        const conv = items.find((c) => c.id === id);
        if (conv && conv.unreadCount > 0) {
            conv.unreadCount = 0;
            ChatState.save();
            this.render();
            EventEmitter.emit("conversation:read", id);
        }
    },

    // ─── Refresh from API ────────────────────────────────────────────────────

    async refresh() {
        const btn = document.getElementById("refreshConvsBtn");
        if (btn) {
            btn.disabled = true;
            btn.textContent = "…";
        }

        try {
            const res = await fetch("/api/chats");
            if (!res.ok) throw new Error(`HTTP ${res.status}`);
            const data = await res.json();
            const chats = data.data?.chats ?? data.chats ?? [];

            ChatState.conversations = chats
                .filter((c) => c.chat_type === "direct")
                .map((c) => ({
                    id: String(c.chat_id || c.id),
                    name: c.name ?? "Unnamed chat",
                    lastMessage: c.last_message ?? "",
                    timestamp: c.last_message_at
                        ? c.last_message_at * 1000
                        : (c.created_at ?? Date.now()),
                    unreadCount: c.unread_count ?? 0,
                    isOnline: c.is_online ?? false,
                    avatarUrl: c.avatar_url ?? null,
                }));

            ChatState.groups = chats
                .filter((c) => c.chat_type === "group")
                .map((g) => ({
                    id: String(g.chat_id || g.id),
                    name: g.name ?? "Unnamed group",
                    lastMessage: g.last_message ?? "",
                    timestamp: g.last_message_at
                        ? g.last_message_at * 1000
                        : (g.created_at ?? Date.now()),
                    unreadCount: g.unread_count ?? 0,
                    memberCount: g.member_count ?? null,
                }));

            ChatState.save();
            this.render();
            EventEmitter.emit("conversations:refreshed");
        } catch (e) {
            console.error("[chat] refresh failed:", e);
            this.render();
        } finally {
            if (btn) {
                btn.disabled = false;
                btn.textContent = "↺";
            }
        }
    },

    // ─── Search ──────────────────────────────────────────────────────────────

    setupSearch() {
        const input = document.getElementById("conversationSearch");
        if (!input) return;

        input.addEventListener(
            "input",
            Utils.debounce((e) => {
                const query = e.target.value.toLowerCase();
                document.querySelectorAll(".conversation-item").forEach((el) => {
                    const name =
                        el.querySelector(".conversation-name")?.textContent.toLowerCase() || "";
                    el.style.display = name.includes(query) ? "flex" : "none";
                });
            }, 300)
        );
    },

    // ─── New DM / Group buttons & modals ─────────────────────────────────────

    setupNewButtons() {
        document.getElementById("newChatBtn")?.addEventListener("click", () => {
            const id = this.activeTab === "dm" ? "new-dm-modal" : "new-group-modal";
            EventEmitter.emit("modal:request:open", id);
        });

        document.getElementById("refreshConvsBtn")?.addEventListener("click", () => {
            this.refresh();
        });

        document
            .getElementById("newDmSubmitBtn")
            ?.addEventListener("click", () => this._submitDm());
        document
            .getElementById("newGroupSubmitBtn")
            ?.addEventListener("click", () => this._submitGroup());

        document.getElementById("dmRecipientInput")?.addEventListener("keydown", (e) => {
            if (e.key === "Enter") this._submitDm();
        });
        document.getElementById("groupNameInput")?.addEventListener("keydown", (e) => {
            if (e.key === "Enter") this._submitGroup();
        });
    },

    async _submitDm() {
        const input = document.getElementById("dmRecipientInput");
        const errorEl = document.getElementById("dmRecipientError");
        const username = input?.value.trim();

        if (!username) {
            if (errorEl) errorEl.textContent = "Please enter a username.";
            return;
        }

        try {
            const response = await fetch("/api/chats", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ username }),
            });

            if (!response.ok) {
                const errData = await response.json();
                throw new Error(errData.message || "Failed to create DM");
            }

            const newChat = await response.json();
            const chatData = newChat.data || newChat;
            const id = String(chatData.id || chatData.chat_id);

            ChatState.addConversation({
                id,
                name: chatData.name || username,
                lastMessage: "",
                timestamp: chatData.created_at || Date.now(),
                unreadCount: 0,
                isOnline: false,
                avatarUrl: chatData.avatar_url ?? null,
            });
            ChatState.save();

            EventEmitter.emit("modal:request:close", "new-dm-modal");
            this._switchTab("dm");
            this.open(id, "dm");
        } catch (err) {
            if (errorEl) errorEl.textContent = err.message;
            console.error("[conversations] Create DM error:", err);
        }
    },

    async _submitGroup() {
        const input = document.getElementById("groupNameInput");
        const errorEl = document.getElementById("groupNameError");
        const name = input?.value.trim();

        if (!name) {
            if (errorEl) errorEl.textContent = "Please enter a group name.";
            return;
        }

        try {
            const response = await fetch("/api/groups", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ name }),
            });

            if (!response.ok) {
                const errData = await response.json();
                throw new Error(errData.message || "Failed to create group");
            }

            const newGroup = await response.json();
            const groupData = newGroup.data || newGroup;
            const id = String(groupData.chat_id || groupData.group_id || groupData.id);

            ChatState.addGroup({
                id,
                name: groupData.name || name,
                lastMessage: "",
                timestamp: groupData.created_at || Date.now(),
                unreadCount: 0,
                memberCount: 1,
            });
            ChatState.save();

            EventEmitter.emit("modal:request:close", "new-group-modal");
            this._switchTab("groups");
            this.open(id, "groups");
        } catch (err) {
            if (errorEl) errorEl.textContent = err.message;
            console.error("[conversations] Create Group error:", err);
        }
    },
};

export default ChatConversations;
