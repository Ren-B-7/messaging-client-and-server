/**
 * Chat — SSE (Server-Sent Events) Manager
 *
 * Manages a single persistent EventSource connection per open chat.
 * Switching chats tears down the old connection and opens a fresh one.
 *
 * Event sequence on connect:
 *   connected        → handshake confirmed
 *   history_start    → backend is about to replay history
 *   history_message  → one per historical message (oldest first)
 *   history_end      → history complete; live events follow
 *   message_sent     → new message from any chat member
 *   message_read     → a member read a message
 *   typing           → a member is typing / stopped typing
 *   chat_created     → a new DM was created (refresh sidebar)
 *   reconnect        → client lagged; connection will be re-established
 *
 * Depends on: Utils, ChatState, ChatMessages, ChatConversations, ChatUI
 */

const ChatSSE = (() => {

  // ── Private state ─────────────────────────────────────────────────────────

  let _source        = null;   // Active EventSource
  let _chatId        = null;   // Chat the connection is scoped to
  let _retryCount    = 0;
  let _retryTimer    = null;
  let _replayBuffer  = [];     // history_message frames collected before history_end
  let _inHistory     = false;  // true between history_start and history_end

  const MAX_RETRIES  = 8;
  const BASE_DELAY   = 1_000;  // ms — doubles each attempt, capped at ~2 min

  // ── Connection lifecycle ──────────────────────────────────────────────────

  /**
   * Open (or re-open) the SSE stream for `chatId`.
   * Calling this while a connection is already open for the same chat is a no-op.
   * Calling with a different chatId cleanly replaces the old connection.
   */
  function connect(chatId) {
    if (_source && _chatId === String(chatId)) return; // already connected
    disconnect();

    _chatId       = String(chatId);
    _retryCount   = 0;
    _replayBuffer = [];
    _inHistory    = false;

    _open();
  }

  /** Tear down any open connection and cancel pending reconnect timers. */
  function disconnect() {
    if (_retryTimer) { clearTimeout(_retryTimer); _retryTimer = null; }
    if (_source)     { _source.close(); _source = null; }
    _chatId    = null;
    _lastTyping = null;  // reset so next chat sends correctly
    _setStatus('disconnected');
  }

  function _open() {
    if (!_chatId) return;

    const url = `/api/stream?chat_id=${encodeURIComponent(_chatId)}`;
    _source = new EventSource(url, { withCredentials: true });

    _source.addEventListener('connected',        _onConnected);
    _source.addEventListener('history_start',    _onHistoryStart);
    _source.addEventListener('history_message',  _onHistoryMessage);
    _source.addEventListener('history_end',      _onHistoryEnd);
    _source.addEventListener('message_sent',     _onMessageSent);
    _source.addEventListener('message_read',     _onMessageRead);
    _source.addEventListener('typing',           _onTyping);
    _source.addEventListener('chat_created',     _onChatCreated);
    _source.addEventListener('reconnect',        _onReconnectHint);
    _source.onerror = _onError;
  }

  // ── EventSource event handlers ────────────────────────────────────────────

  function _onConnected() {
    _retryCount = 0;
    _setStatus('connected');
    console.info('[sse] Connected to chat', _chatId);
  }

  function _onHistoryStart(e) {
    _inHistory    = true;
    _replayBuffer = [];
    const payload = _parse(e);
    console.info('[sse] History start — expecting', payload?.count ?? '?', 'messages');
  }

  function _onHistoryMessage(e) {
    if (!_inHistory) return;
    const msg = _parse(e);
    if (msg) _replayBuffer.push(msg);
  }

  function _onHistoryEnd() {
    _inHistory = false;

    if (!_chatId) return;

    const myId = ChatState.currentUser?.id ?? null;

    // Normalise history frames to the same shape ChatMessages uses
    const messages = _replayBuffer.map(msg => ({
      id:          msg.id,
      text:        msg.content,
      content:     msg.content,
      timestamp:   (msg.sent_at ?? 0) * 1000,
      isSent:      myId !== null && msg.sender_id === myId,
      sender_id:   msg.sender_id,
      delivered_at: msg.delivered_at,
      read_at:     msg.read_at,
      message_type: msg.message_type,
    }));

    // Oldest-first order from the server — reverse so newest is at bottom
    messages.reverse();

    ChatState.messages[_chatId] = messages;
    try { ChatState.save(); } catch (_) {}

    // Only render if we're still viewing this chat
    if (ChatState.currentConversation?.id === _chatId) {
      ChatMessages.render(ChatState.getMessages(_chatId));
    }

    _replayBuffer = [];
    console.info('[sse] History end —', messages.length, 'messages loaded');
  }

  function _onMessageSent(e) {
    const msg = _parse(e);
    if (!msg || !_chatId) return;

    const myId    = ChatState.currentUser?.id ?? null;
    const isSent  = myId !== null && msg.sender_id === myId;

    // Deduplicate: if we sent this message optimistically we already have it
    const existing = ChatState.getMessages(_chatId);
    if (existing.some(m => m.id === msg.id)) return;

    const normalized = {
      id:          msg.id,
      text:        msg.content,
      content:     msg.content,
      timestamp:   (msg.sent_at ?? 0) * 1000,
      isSent,
      sender_id:   msg.sender_id,
      message_type: msg.message_type,
    };

    ChatState.addMessage(_chatId, normalized);

    // Update conversation preview in the sidebar
    const conv = ChatState.findConversation(_chatId) ?? ChatState.findGroup(_chatId);
    if (conv) {
      conv.lastMessage = msg.content;
      conv.timestamp   = Date.now();
      // Increment unread only for messages from others that aren't active
      if (!isSent && ChatState.currentConversation?.id !== _chatId) {
        conv.unreadCount = (conv.unreadCount ?? 0) + 1;
      }
    }

    try { ChatState.save(); } catch (_) {}

    if (ChatState.currentConversation?.id === _chatId) {
      ChatMessages.renderOne(normalized);   // append single bubble, no DOM wipe
    }

    ChatConversations.render();
  }

  function _onMessageRead(e) {
    const payload = _parse(e);
    if (!payload || !_chatId) return;

    // Mark the message as read in our local copy
    const msgs = ChatState.getMessages(_chatId);
    const msg  = msgs.find(m => m.id === payload.message_id);
    if (msg) {
      msg.read_at = payload.read_at;
      try { ChatState.save(); } catch (_) {}
    }

    if (ChatState.currentConversation?.id === _chatId) {
      ChatMessages.renderReadReceipts(_chatId, payload.message_id, payload.reader_id);
    }
  }

  // Typing indicator: clear after 3 s of silence
  const _typingTimers = {};

  function _onTyping(e) {
    const payload = _parse(e);
    if (!payload) return;

    const { chat_id, user_id, is_typing } = payload;
    if (String(chat_id) !== _chatId) return;
    if (user_id === ChatState.currentUser?.id) return; // ignore own echo

    const key = `${chat_id}:${user_id}`;

    if (_typingTimers[key]) {
      clearTimeout(_typingTimers[key]);
      delete _typingTimers[key];
    }

    if (is_typing) {
      ChatUI.showTyping(user_id);
      _typingTimers[key] = setTimeout(() => {
        ChatUI.hideTyping(user_id);
        delete _typingTimers[key];
      }, 5_000);
    } else {
      ChatUI.hideTyping(user_id);
    }
  }

  function _onChatCreated(e) {
    const payload = _parse(e);
    console.info('[sse] New chat created:', payload);
    // Refresh the sidebar so the new DM/group appears immediately
    ChatConversations.refresh().catch(() => {});
  }

  function _onReconnectHint(e) {
    const payload = _parse(e);
    console.warn('[sse] Server requested reconnect:', payload);
    _scheduleRetry();
  }

  function _onError(err) {
    // EventSource fires onerror on any network hiccup; we only log + retry.
    if (_source?.readyState === EventSource.CLOSED) {
      console.warn('[sse] Connection closed, scheduling retry…');
      _scheduleRetry();
    }
  }

  // ── Reconnect with exponential back-off ───────────────────────────────────

  function _scheduleRetry() {
    if (_retryTimer || !_chatId) return;
    if (_retryCount >= MAX_RETRIES) {
      console.error('[sse] Max retries reached for chat', _chatId);
      _setStatus('failed');
      return;
    }

    const delay = Math.min(BASE_DELAY * 2 ** _retryCount, 128_000);
    _retryCount++;
    _setStatus('reconnecting');

    console.info(`[sse] Retry #${_retryCount} in ${delay}ms…`);
    _retryTimer = setTimeout(() => {
      _retryTimer = null;
      if (_source) { _source.close(); _source = null; }
      _open();
    }, delay);
  }

  // ── Outbound: typing indicator ────────────────────────────────────────────

  let _lastTyping = null;  // last value sent — avoid duplicate POSTs

  /**
   * Send a typing indicator for the currently open chat.
   *
   * Driven purely by the input box contents: called with `true` when the box
   * has text, `false` when it is empty or the message is sent.  Deduplicates
   * so the same value is never POSTed twice in a row.
   *
   * @param {boolean} isTyping
   */
  function sendTyping(isTyping) {
    if (!_chatId || _lastTyping === isTyping) return;
    _lastTyping = isTyping;

    fetch('/api/typing', {
      method:  'POST',
      headers: { 'Content-Type': 'application/json' },
      body:    JSON.stringify({ chat_id: parseInt(_chatId), is_typing: isTyping }),
    }).catch(e => console.warn('[sse] Typing POST failed:', e));
  }

  // ── Status indicator (optional small UI hook) ─────────────────────────────

  function _setStatus(status) {
    // status: 'connected' | 'disconnected' | 'reconnecting' | 'failed'
    const dot = document.getElementById('sseStatusDot');
    if (!dot) return;
    dot.dataset.status = status;
    dot.title = {
      connected:    'Live — connected',
      disconnected: 'Disconnected',
      reconnecting: 'Reconnecting…',
      failed:       'Connection failed',
    }[status] ?? status;
  }

  // ── Helpers ───────────────────────────────────────────────────────────────

  function _parse(e) {
    try { return JSON.parse(e.data); } catch (_) { return null; }
  }

  // ── Public API ────────────────────────────────────────────────────────────

  return { connect, disconnect, sendTyping };

})();
