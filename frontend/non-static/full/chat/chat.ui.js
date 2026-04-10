/**
 * Chat — UI & Header
 * Manages the empty-state / active-chat view toggle, chat header rendering,
 * action button handlers, group settings (rename, delete, member management),
 * and the typing indicator.
 *
 * Depends on: Utils, ChatState, ChatConversations, DOM, EventEmitter
 */

import Utils from "../../../static/js/full/utils/utils.js";
import { DOM } from "../../../static/js/full/utils/dom.js";
import { EventEmitter } from "../../../static/js/full/utils/events.js";
import ChatState from "./chat.state.js";

export const ChatUI = {
    // ── Empty / active state ─────────────────────────────────────────────────

    showEmptyState() {
        const empty = document.getElementById("emptyChatState");
        const active = document.getElementById("activeChatView");
        if (empty) empty.style.display = "flex";
        if (active) active.style.display = "none";
        EventEmitter.emit("ui:state:empty");
    },

    hideEmptyState() {
        const empty = document.getElementById("emptyChatState");
        const active = document.getElementById("activeChatView");
        if (empty) empty.style.display = "none";
        if (active) active.style.display = "flex";
        EventEmitter.emit("ui:state:active");
    },

    // ── Chat header ──────────────────────────────────────────────────────────

    updateHeader(conversation, type = "dm") {
        const nameEl = document.getElementById("chatName");
        const avatarEl = document.getElementById("chatAvatar");
        const textEl = document.getElementById("statusText");
        const addMemberBtn = document.getElementById("addMemberBtn");
        const groupSettingsBtn = document.getElementById("groupSettingsBtn");

        if (nameEl) nameEl.textContent = Utils.escapeHtml(conversation.name);

        // Render avatar: photo if available, initials otherwise.
        if (avatarEl) {
            DOM.clear(avatarEl);
            if (type === "dm" && conversation.avatarUrl) {
                avatarEl.appendChild(
                    DOM.create("img", {
                        src: conversation.avatarUrl,
                        alt: `Avatar of ${conversation.name}`,
                        style: {
                            width: "100%",
                            height: "100%",
                            objectFit: "cover",
                            borderRadius: "inherit",
                        },
                        onerror: () => {
                            avatarEl.textContent = Utils.getInitials(conversation.name);
                        },
                    })
                );
            } else {
                avatarEl.textContent = Utils.getInitials(conversation.name);
            }
        }

        if (type === "groups") {
            if (textEl)
                textEl.textContent = conversation.memberCount
                    ? `${conversation.memberCount} members`
                    : "Group";
            if (addMemberBtn) addMemberBtn.style.display = "";
            if (groupSettingsBtn) groupSettingsBtn.style.display = "";
        } else {
            if (addMemberBtn) addMemberBtn.style.display = "none";
            if (groupSettingsBtn) groupSettingsBtn.style.display = "none";
            if (textEl) textEl.textContent = "Online"; // Or fallback
        }
    },

    // ── Typing indicator ─────────────────────────────────────────────────────

    _typingUsers: new Set(),

    showTyping(userId) {
        this._typingUsers.add(userId);
        this._renderTyping();
    },

    hideTyping(userId) {
        this._typingUsers.delete(userId);
        this._renderTyping();
    },

    _renderTyping() {
        let banner = document.getElementById("typingIndicator");
        if (this._typingUsers.size === 0) {
            if (banner) banner.remove();
            return;
        }

        if (!banner) {
            banner = DOM.create("div", {
                id: "typingIndicator",
                className: "typing-indicator",
                ariaLive: "polite",
            });
            const bottomBar = document.querySelector(".chat-bottom-bar");
            const inputArea = document.getElementById("messageInputArea");
            if (bottomBar && inputArea) {
                bottomBar.insertBefore(banner, inputArea);
            } else if (inputArea) {
                inputArea.parentNode.insertBefore(banner, inputArea);
            } else {
                document.getElementById("messagesContainer")?.appendChild(banner);
            }
        }

        const count = this._typingUsers.size;
        banner.innerHTML = `
            <span class="typing-dots"><span></span><span></span><span></span></span>
            <span class="typing-text">${
                count === 1 ? "Someone is typing" : `${count} people are typing`
            }&hellip;</span>`;
    },

    // ── Action buttons setup ─────────────────────────────────────────────────

    setupActionButtons() {
        // Own profile picture
        const avatarInput = document.getElementById("avatarFileInput");
        const avatarBtn = document.getElementById("changeAvatarBtn");

        if (avatarBtn && avatarInput) {
            avatarBtn.addEventListener("click", () => avatarInput.click());
            avatarInput.addEventListener("change", async () => {
                const file = avatarInput.files?.[0];
                avatarInput.value = "";
                if (file) await this._uploadAvatar(file);
            });
        }

        // Quick-add button in header
        document.getElementById("addMemberBtn")?.addEventListener("click", () => {
            const conv = ChatState.currentConversation;
            if (!conv) return;
            document.getElementById("addMemberGroupName").textContent = conv.name;
            this._openModal("add-member-modal");
        });

        document
            .getElementById("addMemberSubmitBtn")
            ?.addEventListener("click", () => this._submitAddMember());
        document.getElementById("addMemberInput")?.addEventListener("keydown", (e) => {
            if (e.key === "Enter") this._submitAddMember();
        });

        // Group settings
        document
            .getElementById("groupSettingsBtn")
            ?.addEventListener("click", () => this._openGroupSettings());
        document
            .getElementById("groupRenameBtn")
            ?.addEventListener("click", () => this._submitRename());
        document.getElementById("groupRenameInput")?.addEventListener("keydown", (e) => {
            if (e.key === "Enter") this._submitRename();
        });
        document.getElementById("groupDeleteBtn")?.addEventListener("click", () => {
            const conv = ChatState.currentConversation;
            if (!conv) return;
            document.getElementById("groupDeleteName").textContent = conv.name;
            document.getElementById("groupDeleteError").textContent = "";
            this._openModal("group-delete-confirm-modal");
        });
        document
            .getElementById("groupDeleteConfirmBtn")
            ?.addEventListener("click", () => this._submitDeleteGroup());

        // Add member in settings
        document
            .getElementById("groupAddMemberBtn")
            ?.addEventListener("click", () => this._submitAddMemberFromSettings());
        document.getElementById("groupAddMemberInput")?.addEventListener("keydown", (e) => {
            if (e.key === "Enter") this._submitAddMemberFromSettings();
        });
        document.getElementById("groupAddMemberInput")?.addEventListener(
            "input",
            Utils.debounce((e) => this._searchUsers(e.target.value), 300)
        );

        // Global modal close listeners
        document.querySelectorAll("[data-close-conv-modal]").forEach((btn) => {
            btn.addEventListener("click", () => this._closeModal(btn.dataset.closeConvModal));
        });

        document.addEventListener("click", (e) => {
            if (e.target.classList.contains("conv-modal-backdrop")) {
                this._closeModal(e.target.id);
            }
            // Close search results
            const wrapper = document.querySelector(".group-search-wrapper");
            if (wrapper && !wrapper.contains(e.target)) {
                const results = document.getElementById("groupSearchResults");
                if (results) {
                    results.style.display = "none";
                    results.innerHTML = "";
                }
            }
        });

        document.addEventListener("keydown", (e) => {
            if (e.key === "Escape") {
                const openModal = document.querySelector(".conv-modal-backdrop.open");
                if (openModal) this._closeModal(openModal.id);
            }
        });
    },

    // ── Group settings modal ─────────────────────────────────────────────────

    async _openGroupSettings() {
        const conv = ChatState.currentConversation;
        if (!conv) return;

        const renameInput = document.getElementById("groupRenameInput");
        if (renameInput) renameInput.value = conv.name;

        ["groupRenameError", "groupAddMemberError"].forEach((id) => {
            const el = document.getElementById(id);
            if (el) {
                el.textContent = "";
                el.style.color = "";
            }
        });

        document.getElementById("groupAddMemberInput").value = "";
        const searchResults = document.getElementById("groupSearchResults");
        searchResults.style.display = "none";
        searchResults.innerHTML = "";

        this._openModal("group-settings-modal");
        await this._loadMembers(conv.id);
    },

    async _loadMembers(chatId) {
        const listEl = document.getElementById("groupMembersList");
        const countEl = document.getElementById("groupMemberCount");
        if (!listEl) return;

        DOM.clear(listEl, DOM.create("p", { className: "loading-text" }, "Loading…"));

        try {
            const res = await fetch(`/api/groups/${encodeURIComponent(chatId)}/members`);
            if (!res.ok) throw new Error(`HTTP ${res.status}`);
            const data = await res.json();
            const members = data.data?.members ?? data.members ?? [];

            if (countEl) countEl.textContent = `(${members.length})`;

            if (!members.length) {
                DOM.clear(
                    listEl,
                    DOM.create("p", { className: "empty-text" }, "No members found.")
                );
                return;
            }

            const myId = ChatState.currentUser?.id ?? null;

            DOM.clear(
                listEl,
                members.map((m) => {
                    const isMe = m.user_id === myId;
                    const displayName = m.username || `User ${m.user_id}`;
                    return DOM.create(
                        "div",
                        { className: "group-member-row", dataset: { userId: m.user_id } },
                        [
                            DOM.create(
                                "div",
                                { className: "avatar avatar-sm" },
                                Utils.getInitials(displayName)
                            ),
                            DOM.create("div", { className: "group-member-info" }, [
                                DOM.create(
                                    "span",
                                    { className: "group-member-name" },
                                    Utils.escapeHtml(displayName)
                                ),
                                m.role && m.role !== "member"
                                    ? DOM.create(
                                          "span",
                                          { className: "group-member-role" },
                                          Utils.escapeHtml(m.role)
                                      )
                                    : null,
                                isMe
                                    ? DOM.create("span", { className: "group-member-you" }, "you")
                                    : null,
                            ]),
                            !isMe
                                ? DOM.create(
                                      "button",
                                      {
                                          className: "group-member-remove-btn",
                                          title: "Remove member",
                                          onclick: () => this._removeMember(chatId, m.user_id),
                                      },
                                      "×"
                                  )
                                : null,
                        ]
                    );
                })
            );
        } catch (e) {
            DOM.clear(
                listEl,
                DOM.create("p", { className: "error-text" }, "Failed to load members.")
            );
            console.error("[group-settings] Load members error:", e);
        }
    },

    async _removeMember(chatId, targetUserId) {
        try {
            const res = await fetch(`/api/groups/${encodeURIComponent(chatId)}/members`, {
                method: "DELETE",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ user_id: targetUserId }),
            });
            if (!res.ok) {
                const err = await res.json();
                throw new Error(err.message || "Failed to remove member");
            }

            const group = ChatState.groups.find((g) => g.id === String(chatId));
            if (group && group.memberCount > 0) {
                group.memberCount--;
                this.updateHeader(group, "groups");
                EventEmitter.emit("group:updated", group);
            }

            await this._loadMembers(chatId);
        } catch (e) {
            console.error("[group-settings] Remove member error:", e);
            alert(e.message || "Failed to remove member.");
        }
    },

    async _submitRename() {
        const input = document.getElementById("groupRenameInput");
        const errEl = document.getElementById("groupRenameError");
        const newName = input?.value.trim();
        const conv = ChatState.currentConversation;

        if (errEl) {
            errEl.textContent = "";
            errEl.style.color = "";
        }
        if (!newName) {
            if (errEl) errEl.textContent = "Please enter a group name.";
            return;
        }
        if (!conv) return;

        const btn = document.getElementById("groupRenameBtn");
        if (btn) btn.disabled = true;

        try {
            const res = await fetch(`/api/groups/${encodeURIComponent(conv.id)}`, {
                method: "PATCH",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ name: newName }),
            });

            if (!res.ok) {
                const err = await res.json();
                throw new Error(err.message || "Failed to rename group");
            }

            const group = ChatState.groups.find((g) => g.id === conv.id);
            if (group) {
                group.name = newName;
                this.updateHeader(group, "groups");
                EventEmitter.emit("group:updated", group);
            }

            if (errEl) {
                errEl.style.color = "var(--success)";
                errEl.textContent = "Renamed successfully.";
                setTimeout(() => {
                    errEl.textContent = "";
                    errEl.style.color = "";
                }, 2000);
            }
        } catch (e) {
            if (errEl) errEl.textContent = e.message || "Failed to rename.";
            console.error("[group-settings] Rename error:", e);
        } finally {
            if (btn) btn.disabled = false;
        }
    },

    async _submitDeleteGroup() {
        const conv = ChatState.currentConversation;
        const errEl = document.getElementById("groupDeleteError");
        if (!conv) return;

        const btn = document.getElementById("groupDeleteConfirmBtn");
        if (btn) btn.disabled = true;
        if (errEl) errEl.textContent = "";

        try {
            const res = await fetch(`/api/groups/${encodeURIComponent(conv.id)}`, {
                method: "DELETE",
            });
            if (!res.ok) {
                const err = await res.json();
                throw new Error(err.message || "Failed to delete group");
            }

            ChatState.groups = ChatState.groups.filter((g) => g.id !== conv.id);
            ChatState.currentConversation = null;
            ChatState.save();

            this._closeModal("group-delete-confirm-modal");
            this._closeModal("group-settings-modal");
            this.showEmptyState();
            EventEmitter.emit("group:deleted", conv.id);
        } catch (e) {
            if (errEl) errEl.textContent = e.message || "Failed to delete group.";
            console.error("[group-settings] Delete error:", e);
        } finally {
            if (btn) btn.disabled = false;
        }
    },

    async _submitAddMemberFromSettings() {
        const input = document.getElementById("groupAddMemberInput");
        const errEl = document.getElementById("groupAddMemberError");
        const username = input?.value.trim();
        const conv = ChatState.currentConversation;

        if (errEl) {
            errEl.textContent = "";
            errEl.style.color = "";
        }
        if (!username) {
            if (errEl) errEl.textContent = "Please enter a username.";
            return;
        }
        if (!conv) return;

        const btn = document.getElementById("groupAddMemberBtn");
        if (btn) btn.disabled = true;

        try {
            const res = await fetch(`/api/groups/${encodeURIComponent(conv.id)}/members`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ username }),
            });

            if (!res.ok) {
                const err = await res.json();
                throw new Error(err.message || "Failed to add member");
            }

            const group = ChatState.groups.find((g) => g.id === conv.id);
            if (group) {
                group.memberCount = (group.memberCount ?? 1) + 1;
                this.updateHeader(group, "groups");
                EventEmitter.emit("group:updated", group);
            }

            if (input) input.value = "";
            document.getElementById("groupSearchResults").style.display = "none";

            await this._loadMembers(conv.id);

            if (errEl) {
                errEl.style.color = "var(--success)";
                errEl.textContent = `${username} added successfully.`;
                setTimeout(() => {
                    errEl.textContent = "";
                    errEl.style.color = "";
                }, 2500);
            }
        } catch (e) {
            if (errEl) errEl.textContent = e.message || "Failed to add member.";
            console.error("[group-settings] Add member error:", e);
        } finally {
            if (btn) btn.disabled = false;
        }
    },

    async _searchUsers(query) {
        const resultsEl = document.getElementById("groupSearchResults");
        if (!resultsEl) return;

        const q = query.trim();
        if (q.length < 2) {
            resultsEl.style.display = "none";
            resultsEl.innerHTML = "";
            return;
        }

        try {
            const res = await fetch(`/api/users/search?q=${encodeURIComponent(q)}`);
            if (!res.ok) return;
            const data = await res.json();
            const users = data.data?.users ?? data.users ?? [];

            if (!users.length) {
                resultsEl.style.display = "none";
                return;
            }

            DOM.clear(
                resultsEl,
                users.map((u) =>
                    DOM.create(
                        "div",
                        {
                            className: "group-search-result-item",
                            onclick: () => {
                                document.getElementById("groupAddMemberInput").value = u.username;
                                resultsEl.style.display = "none";
                            },
                        },
                        [
                            DOM.create(
                                "div",
                                { className: "avatar avatar-sm" },
                                Utils.getInitials(u.username)
                            ),
                            DOM.create("span", {}, Utils.escapeHtml(u.username)),
                        ]
                    )
                )
            );

            resultsEl.style.display = "block";
        } catch (e) {
            console.warn("[group-settings] User search error:", e);
        }
    },

    async _submitAddMember() {
        const input = document.getElementById("addMemberInput");
        const errorEl = document.getElementById("addMemberError");
        const name = input?.value.trim();
        const conv = ChatState.currentConversation;

        if (!name) {
            if (errorEl) errorEl.textContent = "Please enter a name or username.";
            return;
        }
        if (!conv) {
            if (errorEl) errorEl.textContent = "No active group selected.";
            return;
        }

        try {
            const addRes = await fetch(`/api/groups/${encodeURIComponent(conv.id)}/members`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ username: name }),
            });

            if (!addRes.ok) {
                const errData = await addRes.json();
                throw new Error(errData.message || "Failed to add member");
            }

            const group = ChatState.groups.find((g) => g.id === conv.id);
            if (group) {
                group.memberCount = (group.memberCount ?? 1) + 1;
                this.updateHeader(group, "groups");
                EventEmitter.emit("group:updated", group);
            }

            this._closeModal("add-member-modal");
        } catch (err) {
            if (errorEl) errorEl.textContent = err.message || "Failed to add member.";
        }
    },

    async _uploadAvatar(file) {
        const statusEl = document.getElementById("avatarUploadStatus");
        const previewEl = document.getElementById("avatarPreview");

        const setStatus = (text, color) => {
            if (!statusEl) return;
            statusEl.textContent = text;
            statusEl.style.color = color || "";
            statusEl.style.display = text ? "block" : "none";
        };

        setStatus("Uploading…", "");

        const formData = new FormData();
        formData.append("avatar", file);

        try {
            const res = await fetch("/api/profile/avatar", { method: "POST", body: formData });
            if (!res.ok) {
                const err = await res.json().catch(() => ({}));
                throw new Error(err.message || `Upload failed (HTTP ${res.status})`);
            }

            const data = await res.json();
            const avatarUrl = data.avatar_url || `/api/avatar/${ChatState.currentUser?.id}`;

            if (ChatState.currentUser) {
                ChatState.currentUser.avatarUrl = avatarUrl;
                Utils.setStorage("user", ChatState.currentUser);
            }

            if (previewEl) {
                previewEl.src = avatarUrl + "?t=" + Date.now();
                previewEl.style.display = "block";
            }

            const userAvatarEl = document.getElementById("userAvatarImg");
            if (userAvatarEl) {
                userAvatarEl.src = avatarUrl + "?t=" + Date.now();
                userAvatarEl.style.display = "block";
                const initialsEl = document.getElementById("userInitials");
                if (initialsEl) initialsEl.style.display = "none";
            }

            setStatus("✓ Avatar updated", "var(--success)");
            setTimeout(() => setStatus("", ""), 3000);
            EventEmitter.emit("user:avatar:updated", avatarUrl);
        } catch (e) {
            setStatus(`✕ ${e.message}`, "var(--danger)");
            setTimeout(() => setStatus("", ""), 5000);
            console.error("[avatar] Upload error:", e);
        }
    },

    // ── Generic modal helpers ─────────────────────────────────────────────────

    _modalTraps: new Map(),

    _openModal(id) {
        const modal = document.getElementById(id);
        if (!modal) return;
        modal.classList.add("open");
        DOM.focus(modal.querySelector('input[type="text"]') || modal);
        this._modalTraps.set(id, DOM.trapFocus(modal));
        EventEmitter.emit("modal:open", id);
    },

    _closeModal(id) {
        const modal = document.getElementById(id);
        if (!modal) return;
        modal.classList.remove("open");
        if (this._modalTraps.has(id)) {
            this._modalTraps.get(id)();
            this._modalTraps.delete(id);
        }
        EventEmitter.emit("modal:close", id);
    },
};

export default ChatUI;
