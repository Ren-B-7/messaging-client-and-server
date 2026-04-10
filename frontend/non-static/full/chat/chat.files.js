/**
 * Chat — Files
 * Handles file upload, listing, download, and deletion for the active chat.
 *
 * Depends on: Utils, ChatState, DOM, EventEmitter
 */

import Utils from "../../../static/js/full/utils/utils.js";
import { DOM } from "../../../static/js/full/utils/dom.js";
import { EventEmitter } from "../../../static/js/full/utils/events.js";
import ChatState from "./chat.state.js";

export const ChatFiles = {
    // ── API calls ─────────────────────────────────────────────────────────────

    async upload(file, chatId) {
        const body = new FormData();
        body.append("file", file);
        body.append("chat_id", String(chatId));

        const res = await fetch("/api/files/upload", { method: "POST", body });
        if (!res.ok) {
            let msg = `Upload failed (HTTP ${res.status})`;
            try {
                const e = await res.json();
                msg = e.message || msg;
            } catch (_) {}
            throw new Error(msg);
        }
        return res.json();
    },

    async list(chatId) {
        const res = await fetch(`/api/files?chat_id=${encodeURIComponent(chatId)}`);
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const data = await res.json();
        return data.data?.files ?? data.files ?? [];
    },

    async delete(fileId) {
        const res = await fetch(`/api/files/${fileId}`, { method: "DELETE" });
        if (!res.ok) {
            let msg = `Delete failed (HTTP ${res.status})`;
            try {
                const e = await res.json();
                msg = e.message || msg;
            } catch (_) {}
            throw new Error(msg);
        }
    },

    // ── Upload flow ───────────────────────────────────────────────────────────

    setupUpload() {
        const attachBtn = document.getElementById("attachFileBtn");
        const fileInput = document.getElementById("fileInput");

        if (attachBtn && fileInput) {
            attachBtn.addEventListener("click", () => {
                if (!ChatState.currentConversation) return;
                fileInput.click();
            });

            fileInput.addEventListener("change", async () => {
                const files = Array.from(fileInput.files ?? []);
                fileInput.value = "";
                if (!files.length || !ChatState.currentConversation) return;
                for (const file of files) {
                    await this._uploadOne(file, ChatState.currentConversation.id);
                }
            });
        }

        document.getElementById("viewFilesBtn")?.addEventListener("click", () => {
            if (ChatState.currentConversation) this.openModal();
        });
    },

    async _uploadOne(file, chatId) {
        const statusEl = document.getElementById("uploadStatus");
        this._setStatus(statusEl, `Uploading ${file.name}…`, "");

        try {
            await this.upload(file, chatId);
            this._setStatus(statusEl, `✓ ${file.name} uploaded`, "var(--success)");
            setTimeout(() => this._hideStatus(statusEl), 3000);
            EventEmitter.emit("file:uploaded", { file, chatId });
        } catch (e) {
            this._setStatus(statusEl, `✕ ${e.message}`, "var(--danger)");
            setTimeout(() => this._hideStatus(statusEl), 5000);
            console.error("[files] Upload error:", e);
        }
    },

    _setStatus(el, text, color) {
        if (!el) return;
        el.textContent = text;
        el.style.color = color || "";
        el.style.display = "block";
    },

    _hideStatus(el) {
        if (!el) return;
        el.style.display = "none";
        el.style.color = "";
    },

    // ── Files modal ───────────────────────────────────────────────────────────

    openModal() {
        EventEmitter.emit("modal:request:open", "files-modal");
        this._loadModal();
    },

    closeModal() {
        EventEmitter.emit("modal:request:close", "files-modal");
    },

    async _loadModal() {
        const listEl = document.getElementById("filesModalList");
        if (!listEl) return;

        const conv = ChatState.currentConversation;
        if (!conv) return;

        DOM.clear(
            listEl,
            DOM.create(
                "p",
                { className: "loading-text", style: { padding: "var(--space-6)" } },
                "Loading…"
            )
        );

        try {
            const files = await this.list(conv.id);
            this._renderModal(files);
        } catch (e) {
            DOM.clear(
                listEl,
                DOM.create(
                    "p",
                    { className: "error-text", style: { padding: "var(--space-6)" } },
                    "Failed to load files."
                )
            );
            console.error("[files] List error:", e);
        }
    },

    _renderModal(files) {
        const listEl = document.getElementById("filesModalList");
        if (!listEl) return;

        if (!files.length) {
            DOM.clear(
                listEl,
                DOM.create(
                    "p",
                    { className: "empty-text", style: { padding: "var(--space-6)" } },
                    "No files in this conversation yet."
                )
            );
            return;
        }

        const myId = ChatState.currentUser?.id ?? null;

        DOM.clear(
            listEl,
            files.map((f) => {
                const fileId = f.id ?? f.file_id;
                const filename = f.filename ?? f.file_name ?? "file";
                const size = Utils.formatFileSize(f.size ?? f.file_size ?? 0);
                const uploader = f.uploader_id ?? f.sender_id ?? null;
                const canDelete = myId !== null && uploader === myId;

                return DOM.create("div", { className: "files-modal-item", dataset: { fileId } }, [
                    DOM.create("div", { className: "files-modal-icon" }, this._fileIcon(filename)),
                    DOM.create("div", { className: "files-modal-info" }, [
                        DOM.create(
                            "span",
                            { className: "files-modal-name", title: filename },
                            Utils.escapeHtml(filename)
                        ),
                        DOM.create("span", { className: "files-modal-meta" }, size),
                    ]),
                    DOM.create("div", { className: "files-modal-actions" }, [
                        DOM.create(
                            "a",
                            {
                                href: `/api/files/${fileId}`,
                                download: filename,
                                className: "btn btn-ghost btn-sm",
                                title: "Download",
                                ariaLabel: `Download ${filename}`,
                            },
                            "↓"
                        ),
                        canDelete
                            ? DOM.create(
                                  "button",
                                  {
                                      className: "btn btn-ghost btn-sm files-delete-btn",
                                      title: "Delete",
                                      ariaLabel: `Delete ${filename}`,
                                      onclick: () => this._confirmDelete(fileId),
                                  },
                                  "✕"
                              )
                            : null,
                    ]),
                ]);
            })
        );
    },

    async _confirmDelete(fileId) {
        if (!confirm("Delete this file? This cannot be undone.")) return;
        try {
            await this.delete(fileId);
            await this._loadModal();
            EventEmitter.emit("file:deleted", fileId);
        } catch (e) {
            alert(e.message || "Failed to delete file.");
        }
    },

    // ── Helpers ───────────────────────────────────────────────────────────────

    _fileIcon(filename) {
        const ext = (filename.split(".").pop() ?? "").toLowerCase();
        const map = {
            jpg: "🖼️",
            jpeg: "🖼️",
            png: "🖼️",
            gif: "🖼️",
            webp: "🖼️",
            svg: "🖼️",
            mp4: "🎬",
            webm: "🎬",
            mov: "🎬",
            avi: "🎬",
            mkv: "🎬",
            mp3: "🎵",
            wav: "🎵",
            ogg: "🎵",
            flac: "🎵",
            aac: "🎵",
            pdf: "📄",
            doc: "📝",
            docx: "📝",
            txt: "📝",
            md: "📝",
            xls: "📊",
            xlsx: "📊",
            csv: "📊",
            zip: "📦",
            tar: "📦",
            gz: "📦",
            rar: "📦",
            "7z": "📦",
            js: "💻",
            ts: "💻",
            py: "💻",
            rs: "💻",
            go: "💻",
            html: "💻",
            css: "💻",
        };
        return map[ext] ?? "📎";
    },
};

export default ChatFiles;
