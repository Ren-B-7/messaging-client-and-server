/**
 * Chat — Files
 * Handles file upload, listing, download, and deletion for the active chat.
 *
 * API surface used:
 *   POST   /api/files/upload         multipart: fields `file` + `chat_id`
 *   GET    /api/files?chat_id=N      list files for a chat
 *   GET    /api/files/:id            download a file (used as anchor href)
 *   DELETE /api/files/:id            delete own file
 *
 * Depends on: Utils, ChatState
 */

const ChatFiles = {

  // ── API calls ─────────────────────────────────────────────────────────────

  /**
   * Upload a single file to the active chat.
   * @param {File}   file
   * @param {string} chatId
   * @returns {Promise<object>} server response JSON
   */
  async upload(file, chatId) {
    const body = new FormData();
    body.append('file', file);
    body.append('chat_id', String(chatId));

    const res = await fetch('/api/files/upload', { method: 'POST', body });
    if (!res.ok) {
      let msg = `Upload failed (HTTP ${res.status})`;
      try { const e = await res.json(); msg = e.message || msg; } catch (_) {}
      throw new Error(msg);
    }
    return res.json();
  },

  /**
   * List all files for a chat.
   * @param {string} chatId
   * @returns {Promise<object[]>}
   */
  async list(chatId) {
    const res = await fetch(`/api/files?chat_id=${encodeURIComponent(chatId)}`);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = await res.json();
    return data.data?.files ?? data.files ?? [];
  },

  /**
   * Delete a file by its ID.
   * @param {string|number} fileId
   */
  async delete(fileId) {
    const res = await fetch(`/api/files/${fileId}`, { method: 'DELETE' });
    if (!res.ok) {
      let msg = `Delete failed (HTTP ${res.status})`;
      try { const e = await res.json(); msg = e.message || msg; } catch (_) {}
      throw new Error(msg);
    }
  },

  // ── Upload flow ───────────────────────────────────────────────────────────

  /**
   * Wire up the paperclip button and the hidden <input type="file">.
   * Also attaches the "View Files" header button and the files modal.
   * Called once from ChatUI.setupActionButtons().
   */
  setupUpload() {
    const attachBtn = document.getElementById('attachFileBtn');
    const fileInput = document.getElementById('fileInput');

    if (attachBtn && fileInput) {
      attachBtn.addEventListener('click', () => {
        if (!ChatState.currentConversation) return;
        fileInput.click();
      });

      fileInput.addEventListener('change', async () => {
        const files = Array.from(fileInput.files ?? []);
        // Reset immediately so the same file can be re-selected later
        fileInput.value = '';
        if (!files.length || !ChatState.currentConversation) return;
        for (const file of files) {
          await this._uploadOne(file, ChatState.currentConversation.id);
        }
      });
    }

    // View files header button
    document.getElementById('viewFilesBtn')?.addEventListener('click', () => {
      if (ChatState.currentConversation) this.openModal();
    });

    // Files modal close triggers
    document.querySelectorAll('[data-close-conv-modal="files-modal"]').forEach(btn => {
      btn.addEventListener('click', () => this.closeModal());
    });
    document.getElementById('files-modal')?.addEventListener('click', e => {
      if (e.target === e.currentTarget) this.closeModal();
    });

    document.addEventListener('keydown', e => {
      if (e.key === 'Escape') this.closeModal();
    });
  },

  async _uploadOne(file, chatId) {
    const statusEl = document.getElementById('uploadStatus');
    this._setStatus(statusEl, `Uploading ${file.name}…`, '');

    try {
      await this.upload(file, chatId);
      this._setStatus(statusEl, `✓ ${file.name} uploaded`, '');
      setTimeout(() => this._hideStatus(statusEl), 3_000);
    } catch (e) {
      this._setStatus(statusEl, `✕ ${e.message}`, 'var(--danger)');
      setTimeout(() => this._hideStatus(statusEl), 5_000);
      console.error('[files] Upload error:', e);
    }
  },

  _setStatus(el, text, color) {
    if (!el) return;
    el.textContent = text;
    el.style.color = color || '';
    el.style.display = 'block';
  },

  _hideStatus(el) {
    if (!el) return;
    el.style.display = 'none';
    el.style.color = '';
  },

  // ── Files modal ───────────────────────────────────────────────────────────

  openModal() {
    const modal = document.getElementById('files-modal');
    if (!modal) return;
    modal.classList.add('open');
    this._loadModal();
  },

  closeModal() {
    document.getElementById('files-modal')?.classList.remove('open');
  },

  async _loadModal() {
    const listEl = document.getElementById('filesModalList');
    if (!listEl) return;

    const conv = ChatState.currentConversation;
    if (!conv) return;

    listEl.innerHTML = '<p style="color:var(--fg-tertiary);font-size:var(--text-sm);padding:var(--space-6)">Loading…</p>';

    try {
      const files = await this.list(conv.id);
      this._renderModal(files);
    } catch (e) {
      listEl.innerHTML = '<p style="color:var(--danger);font-size:var(--text-sm);padding:var(--space-6)">Failed to load files.</p>';
      console.error('[files] List error:', e);
    }
  },

  _renderModal(files) {
    const listEl = document.getElementById('filesModalList');
    if (!listEl) return;

    if (!files.length) {
      listEl.innerHTML = '<p style="color:var(--fg-tertiary);font-size:var(--text-sm);padding:var(--space-6)">No files in this conversation yet.</p>';
      return;
    }

    const myId = ChatState.currentUser?.id ?? null;

    listEl.innerHTML = files.map(f => {
      const fileId   = f.id ?? f.file_id;
      const filename = f.filename ?? f.file_name ?? 'file';
      const size     = this._formatSize(f.size ?? f.file_size ?? 0);
      const uploader = f.uploader_id ?? f.sender_id ?? null;
      const canDelete = myId !== null && uploader === myId;

      return `
        <div class="files-modal-item" data-file-id="${fileId}">
          <div class="files-modal-icon">${this._fileIcon(filename)}</div>
          <div class="files-modal-info">
            <span class="files-modal-name" title="${Utils.escapeHtml(filename)}">${Utils.escapeHtml(filename)}</span>
            <span class="files-modal-meta">${size}</span>
          </div>
          <div class="files-modal-actions">
            <a
              href="/api/files/${fileId}"
              download="${Utils.escapeHtml(filename)}"
              class="btn btn-ghost btn-sm"
              title="Download"
              aria-label="Download ${Utils.escapeHtml(filename)}"
            >↓</a>
            ${canDelete
              ? `<button
                   class="btn btn-ghost btn-sm files-delete-btn"
                   data-file-id="${fileId}"
                   title="Delete"
                   aria-label="Delete ${Utils.escapeHtml(filename)}"
                 >✕</button>`
              : ''}
          </div>
        </div>`;
    }).join('');

    listEl.querySelectorAll('.files-delete-btn').forEach(btn => {
      btn.addEventListener('click', () => this._confirmDelete(btn.dataset.fileId));
    });
  },

  async _confirmDelete(fileId) {
    if (!confirm('Delete this file? This cannot be undone.')) return;
    try {
      await this.delete(fileId);
      // Re-load the list to reflect the deletion
      await this._loadModal();
    } catch (e) {
      alert(e.message || 'Failed to delete file.');
    }
  },

  // ── Helpers ───────────────────────────────────────────────────────────────

  _formatSize(bytes) {
    if (!bytes || bytes === 0) return '';
    if (bytes < 1_024)             return `${bytes} B`;
    if (bytes < 1_048_576)         return `${(bytes / 1_024).toFixed(1)} KB`;
    if (bytes < 1_073_741_824)     return `${(bytes / 1_048_576).toFixed(1)} MB`;
    return `${(bytes / 1_073_741_824).toFixed(1)} GB`;
  },

  _fileIcon(filename) {
    const ext = (filename.split('.').pop() ?? '').toLowerCase();
    const map = {
      // Images
      jpg: '🖼️', jpeg: '🖼️', png: '🖼️', gif: '🖼️', webp: '🖼️', svg: '🖼️',
      // Video
      mp4: '🎬', webm: '🎬', mov: '🎬', avi: '🎬', mkv: '🎬',
      // Audio
      mp3: '🎵', wav: '🎵', ogg: '🎵', flac: '🎵', aac: '🎵',
      // Documents
      pdf: '📄', doc: '📝', docx: '📝', txt: '📝', md: '📝',
      // Spreadsheets
      xls: '📊', xlsx: '📊', csv: '📊',
      // Archives
      zip: '📦', tar: '📦', gz: '📦', rar: '📦', '7z': '📦',
      // Code
      js: '💻', ts: '💻', py: '💻', rs: '💻', go: '💻', html: '💻', css: '💻',
    };
    return map[ext] ?? '📎';
  },
};
