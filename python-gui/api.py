"""
Chat API Client
HTTP client with automatic cookie jar management and reconnect logic.

Routes mapped exactly from the Rust server source:

  build_api_router_with_config  (routes.rs — shared, both servers)
  ─────────────────────────────────────────────────────────────────
  Open (no auth):
    GET  /health
    GET  /api/config
    POST /api/login
    POST /login          (form alias)
    POST /api/register
    POST /register       (form alias)

  Light (JWT signature + expiry, zero DB):
    GET  /api/profile
    GET  /api/messages          ?chat_id=&limit=&offset=
    GET  /api/chats
    GET  /api/groups
    GET  /api/groups/:id
    GET  /api/groups/:id/members
    GET  /api/users/search      ?q=
    GET  /api/files             ?chat_id=
    GET  /api/files/:id
    GET  /api/avatar/:user_id
    GET  /api/stream            ?chat_id=&limit=&offset=   (SSE)

  Hard (JWT + DB session + IP):
    POST   /api/messages/send
    POST   /api/messages/:id/read
    POST   /api/typing
    POST   /api/chats
    POST   /api/groups
    POST   /api/groups/:id/members
    DELETE /api/groups/:id/members
    PATCH  /api/groups/:id
    DELETE /api/groups/:id
    POST   /api/profile/update
    PUT    /api/profile          (same handler as /api/profile/update)
    POST   /api/settings/password
    POST   /api/settings/logout-all
    POST   /api/logout
    DELETE /api/settings/delete
    POST   /api/files/upload
    DELETE /api/files/:id
    POST   /api/profile/avatar

  build_admin_api_routes  (admin.rs — admin server only)
  ─────────────────────────────────────────────────────
  Hard (JWT + DB + is_admin check):
    GET    /admin/stats              (alias of /admin/api/stats)
    GET    /admin/api/stats
    GET    /admin/metrics            (alias)
    GET    /admin/api/metrics
  Light (JWT + is_admin check):
    GET    /admin/users              (alias)
    GET    /admin/api/users
    GET    /admin/sessions           (alias)
    GET    /admin/api/sessions
  Hard:
    POST   /admin/ban                (alias)
    POST   /admin/api/users/ban      body: {user_id}
    POST   /admin/unban              (alias)
    POST   /admin/api/users/unban    body: {user_id}
    DELETE /admin/users/:id          (alias)
    DELETE /admin/api/users/:id
    POST   /admin/api/users/promote  body: {user_id}
    POST   /admin/api/users/demote   body: {user_id}
"""

import urllib.request
import urllib.parse
import json
import http.cookiejar
import threading
import time
from urllib.error import URLError, HTTPError

from config import DEFAULT_SERVER, USER_AGENT, CONNECTION_TIMEOUT, RECONNECT_TIMEOUT
from cache import MessageCache
from logger import logger


class ChatAPIClient:
    """HTTP client for the Rust chat server."""

    def __init__(self, server_url=DEFAULT_SERVER):
        self.server_url = server_url.rstrip("/")
        self.cookie_jar = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(
            urllib.request.HTTPCookieProcessor(self.cookie_jar)
        )
        self.user_agent = USER_AGENT
        self.message_cache = MessageCache()
        self.last_error = None
        # Shutdown flag — set to stop background SSE threads cleanly.
        self._shutdown = threading.Event()

        logger.info("ChatAPIClient initialized", extra_info=f"Server: {self.server_url}")

    # =========================================================================
    # Low-level request
    # =========================================================================

    def _make_request(self, method, path, data=None, headers=None,
                      timeout=CONNECTION_TIMEOUT):
        """Make an HTTP request.  Returns a result dict; never raises."""
        url = f"{self.server_url}{path}"
        request_headers = {
            "User-Agent": self.user_agent,
            "Content-Type": "application/json",
        }
        if headers:
            request_headers.update(headers)

        if data is not None:
            if isinstance(data, dict):
                data = json.dumps(data).encode("utf-8")
            elif isinstance(data, str):
                data = data.encode("utf-8")

        try:
            logger.debug("Making request", extra_info=f"{method} {url}")
            req = urllib.request.Request(
                url, data=data, headers=request_headers, method=method
            )
            response = self.opener.open(req, timeout=timeout)
            body = response.read()
            self.last_error = None
            logger.debug("Request OK", extra_info=f"HTTP {response.status}, {len(body)} bytes")
            return {
                "status":  response.status,
                "data":    body.decode("utf-8"),
                "headers": dict(response.headers),
                "success": True,
            }

        except HTTPError as e:
            body = e.read().decode("utf-8")
            self.last_error = f"HTTP {e.code}: {body[:100]}"
            logger.warning(f"HTTP {e.code}", extra_info=f"{url} — {self.last_error}")
            return {"status": e.code, "data": body, "error": str(e), "success": False}

        except URLError as e:
            self.last_error = f"Connection error: {e}"
            logger.warning("Connection error", extra_info=f"{url} — {e}")
            return {"status": 0, "error": self.last_error, "success": False}

        except Exception as e:
            self.last_error = str(e)
            logger.exception("Unexpected request error",
                             error_detail=f"{method} {url} — {e}", stop=False)
            return {"status": 0, "error": self.last_error, "success": False}

    def _make_multipart_request(self, path, file_path, field_name="file",
                                extra_fields=None, timeout=CONNECTION_TIMEOUT):
        """
        POST a multipart/form-data request (for file / avatar uploads).
        Returns the same result dict as _make_request.
        """
        import mimetypes, os, uuid
        url = f"{self.server_url}{path}"
        boundary = uuid.uuid4().hex

        parts = []
        # Extra text fields
        for name, value in (extra_fields or {}).items():
            parts.append(
                f"--{boundary}\r\n"
                f'Content-Disposition: form-data; name="{name}"\r\n\r\n'
                f"{value}\r\n"
            )
        # File field
        filename = os.path.basename(file_path)
        mime_type = mimetypes.guess_type(file_path)[0] or "application/octet-stream"
        with open(file_path, "rb") as fh:
            file_data = fh.read()

        header = (
            f"--{boundary}\r\n"
            f'Content-Disposition: form-data; name="{field_name}"; filename="{filename}"\r\n'
            f"Content-Type: {mime_type}\r\n\r\n"
        )
        footer = f"\r\n--{boundary}--\r\n"

        body = b"".join([
            p.encode() for p in parts
        ] + [header.encode(), file_data, footer.encode()])

        req = urllib.request.Request(
            url, data=body, method="POST",
            headers={
                "User-Agent": self.user_agent,
                "Content-Type": f"multipart/form-data; boundary={boundary}",
                "Content-Length": str(len(body)),
            },
        )
        try:
            response = self.opener.open(req, timeout=timeout)
            resp_body = response.read()
            return {
                "status":  response.status,
                "data":    resp_body.decode("utf-8"),
                "headers": dict(response.headers),
                "success": True,
            }
        except HTTPError as e:
            body_err = e.read().decode("utf-8")
            return {"status": e.code, "data": body_err, "error": str(e), "success": False}
        except Exception as e:
            return {"status": 0, "error": str(e), "success": False}

    # =========================================================================
    # Health / config  — Open (no auth)
    # GET /health
    # GET /api/config
    # =========================================================================

    def health(self):
        """GET /health  →  {"health": "ok"}  (418 I'm a Teapot — by design)"""
        return self._make_request("GET", "/health")

    def get_server_config(self):
        """GET /api/config  →  {email_required, token_expiry_minutes}"""
        return self._make_request("GET", "/api/config")

    # =========================================================================
    # Authentication  — Open (no auth)
    # POST /api/login
    # POST /api/register
    # POST /api/logout        (hard auth — session revoked server-side)
    # POST /api/settings/logout-all
    # =========================================================================

    def login(self, username, password):
        """POST /api/login  body: {username, password}"""
        logger.info("Login request", extra_info=f"Username: {username}")
        r = self._make_request("POST", "/api/login",
                               {"username": username, "password": password})
        if r.get("success"):
            logger.info("Login successful", extra_info=f"Username: {username}")
        else:
            logger.warning("Login failed", extra_info=f"{username} — {r.get('error')}")
        return r

    def register(self, email, username, password, full_name):
        """POST /api/register  body: {email, username, password, full_name}"""
        logger.info("Register request", extra_info=f"Email: {email}, Username: {username}")
        r = self._make_request("POST", "/api/register", {
            "email":     email,
            "username":  username,
            "password":  password,
            "full_name": full_name,
        })
        if r.get("success"):
            logger.info("Registration successful", extra_info=f"{email}")
        else:
            logger.warning("Registration failed", extra_info=r.get("error"))
        return r

    def logout(self):
        """POST /api/logout  (hard auth — server revokes the current session)"""
        logger.info("Logout request")
        r = self._make_request("POST", "/api/logout", {})
        if r.get("success"):
            logger.info("Logout successful")
        else:
            logger.warning("Logout failed", extra_info=r.get("error"))
        return r

    def logout_all(self):
        """POST /api/settings/logout-all  (hard auth — revokes ALL sessions for this user)"""
        logger.info("Logout-all request")
        return self._make_request("POST", "/api/settings/logout-all", {})

    # =========================================================================
    # Profile  — Light / Hard
    # Light: GET /api/profile
    # Hard:  POST /api/profile/update   (canonical mutation path)
    #        PUT  /api/profile           (same handler, alternate verb)
    #        POST /api/settings/password
    #        DELETE /api/settings/delete
    #        POST /api/profile/avatar   (multipart)
    # =========================================================================

    def get_profile(self):
        """GET /api/profile  (light auth)"""
        logger.debug("Fetching profile")
        return self._make_request("GET", "/api/profile")

    def update_profile(self, data):
        """
        POST /api/profile/update  (hard auth)
        Preferred over PUT /api/profile — both hit the same handler but
        POST better reflects the mutation intent.
        body: {full_name?, email?, username?, bio?, ...}
        """
        logger.info("Updating profile")
        return self._make_request("POST", "/api/profile/update", data)

    def update_profile_put(self, data):
        """PUT /api/profile  (hard auth) — alternate verb, same handler as POST /api/profile/update"""
        logger.info("Updating profile (PUT)")
        return self._make_request("PUT", "/api/profile", data)

    def change_password(self, old_password, new_password):
        """POST /api/settings/password  (hard auth)  body: {old_password, new_password}"""
        logger.info("Changing password")
        return self._make_request("POST", "/api/settings/password", {
            "old_password": old_password,
            "new_password": new_password,
        })

    def delete_account(self):
        """DELETE /api/settings/delete  (hard auth)"""
        logger.info("Delete account request")
        return self._make_request("DELETE", "/api/settings/delete", {})

    def upload_avatar(self, file_path):
        """
        POST /api/profile/avatar  (hard auth, multipart/form-data)
        Uploads or replaces the current user's profile picture.
        file_path: local path to the image file.
        """
        logger.info("Uploading avatar", extra_info=f"File: {file_path}")
        r = self._make_multipart_request("/api/profile/avatar", file_path,
                                         field_name="avatar")
        if r.get("success"):
            logger.info("Avatar uploaded successfully")
        else:
            logger.warning("Avatar upload failed", extra_info=r.get("error"))
        return r

    # =========================================================================
    # Avatars  — Light
    # GET /api/avatar/:user_id
    # =========================================================================

    def get_avatar(self, user_id):
        """GET /api/avatar/:user_id  (light auth) — returns raw image bytes"""
        logger.debug("Fetching avatar", extra_info=f"User: {user_id}")
        return self._make_request("GET", f"/api/avatar/{user_id}")

    # =========================================================================
    # Chats  — Light / Hard
    # Light: GET /api/chats
    # Hard:  POST /api/chats
    # =========================================================================

    def get_chats(self):
        """GET /api/chats  (light auth) — all chats the current user is in"""
        logger.debug("Loading chats")
        return self._make_request("GET", "/api/chats")

    def get_conversations(self):
        """Alias for get_chats() — backward-compatible name used by app.py."""
        return self.get_chats()

    def create_chat(self, target_user_id):
        """
        POST /api/chats  (hard auth)
        Creates a 1-on-1 direct chat with target_user_id.
        body: {user_id: <int>}
        """
        logger.info("Creating chat", extra_info=f"Target user: {target_user_id}")
        return self._make_request("POST", "/api/chats", {"user_id": target_user_id})

    # =========================================================================
    # Groups  — Light / Hard
    # Light: GET /api/groups
    #        GET /api/groups/:id
    #        GET /api/groups/:id/members
    # Hard:  POST   /api/groups
    #        POST   /api/groups/:id/members   body: {user_id}
    #        DELETE /api/groups/:id/members   body: {user_id}
    #        PATCH  /api/groups/:id           body: {name}
    #        DELETE /api/groups/:id
    # =========================================================================

    def get_groups(self):
        """GET /api/groups  (light auth)"""
        logger.debug("Loading groups")
        return self._make_request("GET", "/api/groups")

    def get_group(self, group_id):
        """GET /api/groups/:id  (light auth)"""
        return self._make_request("GET", f"/api/groups/{group_id}")

    def get_group_members(self, group_id):
        """GET /api/groups/:id/members  (light auth)"""
        return self._make_request("GET", f"/api/groups/{group_id}/members")

    def create_group(self, name, member_ids=None):
        """
        POST /api/groups  (hard auth)
        body: {name: str, member_ids?: [int, ...]}
        """
        logger.info("Creating group", extra_info=f"Name: {name}")
        payload = {"name": name}
        if member_ids:
            payload["member_ids"] = member_ids
        return self._make_request("POST", "/api/groups", payload)

    def add_group_member(self, group_id, user_id):
        """POST /api/groups/:id/members  (hard auth)  body: {user_id}"""
        return self._make_request("POST", f"/api/groups/{group_id}/members",
                                  {"user_id": user_id})

    def remove_group_member(self, group_id, user_id):
        """DELETE /api/groups/:id/members  (hard auth)  body: {user_id}"""
        return self._make_request("DELETE", f"/api/groups/{group_id}/members",
                                  {"user_id": user_id})

    def rename_group(self, group_id, new_name):
        """PATCH /api/groups/:id  (hard auth)  body: {name}"""
        return self._make_request("PATCH", f"/api/groups/{group_id}", {"name": new_name})

    def delete_group(self, group_id):
        """DELETE /api/groups/:id  (hard auth)"""
        return self._make_request("DELETE", f"/api/groups/{group_id}", {})

    # =========================================================================
    # Messages  — Light / Hard
    # Light: GET /api/messages?chat_id=X&limit=N&offset=N
    # Hard:  POST /api/messages/send
    #        POST /api/messages/:id/read
    #        POST /api/typing
    # =========================================================================

    def get_messages(self, chat_id, limit=50, offset=0):
        """
        GET /api/messages  (light auth)
        Query: ?chat_id=<int>&limit=<int>&offset=<int>
        Server returns messages oldest-first.
        """
        logger.debug("Loading messages",
                     extra_info=f"Chat: {chat_id}, limit={limit}, offset={offset}")
        return self._make_request(
            "GET", f"/api/messages?chat_id={chat_id}&limit={limit}&offset={offset}"
        )

    def send_message(self, chat_id, content, message_type="text"):
        """
        POST /api/messages/send  (hard auth)
        body: {chat_id, content, message_type}
        message_type: "text" | "image" | "file"
        """
        logger.debug("Sending message", extra_info=f"Chat: {chat_id}, len={len(content)}")
        r = self._make_request("POST", "/api/messages/send", {
            "chat_id":      chat_id,
            "content":      content,
            "message_type": message_type,
        })
        if r.get("success"):
            logger.info("Message sent", extra_info=f"Chat: {chat_id}")
        else:
            logger.warning("Send failed", extra_info=r.get("error"))
        return r

    def mark_message_read(self, message_id):
        """
        POST /api/messages/:id/read  (hard auth)
        Path: /api/messages/<message_id>/read  — the server extracts the id
        from path segment 3 (0-indexed split on '/').
        """
        logger.debug("Mark read", extra_info=f"Message: {message_id}")
        return self._make_request("POST", f"/api/messages/{message_id}/read", {})

    def send_typing(self, chat_id):
        """POST /api/typing  (hard auth)  body: {chat_id} — fire-and-forget indicator"""
        return self._make_request("POST", "/api/typing", {"chat_id": chat_id})

    # =========================================================================
    # User search  — Light
    # GET /api/users/search?q=<query>
    # =========================================================================

    def search_users(self, query):
        """GET /api/users/search?q=<query>  (light auth)"""
        q = urllib.parse.quote(query)
        return self._make_request("GET", f"/api/users/search?q={q}")

    # =========================================================================
    # Files  — Light / Hard
    # Hard:  POST   /api/files/upload   (multipart/form-data)
    #        DELETE /api/files/:id
    # Light: GET    /api/files?chat_id=N
    #        GET    /api/files/:id
    # =========================================================================

    def get_chat_files(self, chat_id):
        """GET /api/files?chat_id=N  (light auth) — list files attached to a chat"""
        return self._make_request("GET", f"/api/files?chat_id={chat_id}")

    def get_file(self, file_id):
        """GET /api/files/:id  (light auth) — download a single file"""
        return self._make_request("GET", f"/api/files/{file_id}")

    def upload_file(self, file_path, chat_id):
        """
        POST /api/files/upload  (hard auth, multipart/form-data)
        Attaches a file to chat_id.
        file_path : local path to the file to upload.
        chat_id   : the chat the file belongs to.
        """
        logger.info("Uploading file",
                    extra_info=f"File: {file_path}, Chat: {chat_id}")
        r = self._make_multipart_request(
            "/api/files/upload", file_path,
            field_name="file",
            extra_fields={"chat_id": str(chat_id)},
        )
        if r.get("success"):
            logger.info("File uploaded", extra_info=f"Chat: {chat_id}")
        else:
            logger.warning("File upload failed", extra_info=r.get("error"))
        return r

    def delete_file(self, file_id):
        """DELETE /api/files/:id  (hard auth) — delete own file"""
        return self._make_request("DELETE", f"/api/files/{file_id}", {})

    # =========================================================================
    # Real-time SSE stream  — Light (then hard-verified inside the handler)
    # GET /api/stream?chat_id=X[&limit=N&offset=N]
    #
    # Event sequence on connect:
    #   connected       — handshake OK
    #   history_start   — {count: N}
    #   history_message — {id, sender_id, chat_id, content, message_type,
    #                       sent_at, delivered_at, read_at}  (one per message)
    #   history_end     — {}
    #   <live events>   — forwarded from the server broadcast channel
    #   reconnect       — {reason: "lagged", missed: N}  client should reconnect
    # =========================================================================

    def stream_messages(self, chat_id, on_message, on_error,
                        on_connect=None, limit=50, offset=0):
        """
        Open a persistent SSE stream for chat_id.

        Parameters
        ----------
        chat_id    : int
        on_message : callable(event_type: str, data: dict)
        on_error   : callable(error_msg: str)
        on_connect : callable() | None  — fired once the HTTP response opens.
        limit      : int  history messages to replay on connect (server max 100).
        offset     : int  history pagination offset.

        Returns
        -------
        threading.Thread  — daemon; stops when self._shutdown is set.
        """
        logger.info("Starting SSE stream", extra_info=f"Chat: {chat_id}")
        url = (
            f"{self.server_url}/api/stream"
            f"?chat_id={chat_id}&limit={limit}&offset={offset}"
        )

        def _run():
            reconnect_count = 0
            max_reconnects = 5
            event_type = "unknown"

            while not self._shutdown.is_set() and reconnect_count < max_reconnects:
                try:
                    logger.debug("Opening SSE connection",
                                 extra_info=f"Chat: {chat_id}, attempt={reconnect_count + 1}")
                    req = urllib.request.Request(
                        url, headers={"User-Agent": self.user_agent}
                    )
                    response = self.opener.open(req, timeout=CONNECTION_TIMEOUT)

                    if on_connect:
                        on_connect()
                    logger.info("SSE connected", extra_info=f"Chat: {chat_id}")
                    reconnect_count = 0  # reset on clean connect

                    for raw_line in response:
                        if self._shutdown.is_set():
                            logger.debug("SSE stopping (shutdown)")
                            return

                        line = raw_line.decode("utf-8").strip()
                        if line.startswith("event:"):
                            event_type = line.split(":", 1)[1].strip()
                        elif line.startswith("data:"):
                            raw_data = line.split(":", 1)[1].strip()
                            try:
                                on_message(event_type, json.loads(raw_data))
                            except json.JSONDecodeError:
                                logger.warning(
                                    "SSE JSON parse error",
                                    extra_info=f"Chat: {chat_id}, raw={raw_data[:80]}")
                        # id: and blank lines are SSE protocol — ignore

                except Exception as e:
                    if self._shutdown.is_set():
                        return
                    reconnect_count += 1
                    msg = str(e)
                    on_error(msg)
                    logger.warning("SSE error",
                                   extra_info=f"Chat: {chat_id}, attempt={reconnect_count}, {msg}")
                    if reconnect_count < max_reconnects:
                        wait = min(RECONNECT_TIMEOUT * reconnect_count, 30)
                        for _ in range(int(wait * 10)):
                            if self._shutdown.is_set():
                                return
                            time.sleep(0.1)
                    else:
                        on_error(f"Stream failed after {max_reconnects} attempts")
                        logger.exception("SSE permanently failed",
                                         error_detail=f"Chat: {chat_id}", stop=False)

            logger.info("SSE stream exited", extra_info=f"Chat: {chat_id}")

        t = threading.Thread(target=_run, daemon=True, name=f"sse-chat-{chat_id}")
        t.start()
        logger.debug("SSE thread started", extra_info=f"Chat: {chat_id}")
        return t

    # =========================================================================
    # Admin routes  — admin server only, hard auth + is_admin claim
    #
    # Both short (/admin/stats) and canonical (/admin/api/stats) paths exist
    # in the Rust source; we always use the canonical /admin/api/... form.
    #
    # GET    /admin/api/stats
    # GET    /admin/api/metrics
    # GET    /admin/api/users
    # GET    /admin/api/sessions
    # POST   /admin/api/users/ban      body: {user_id}
    # POST   /admin/api/users/unban    body: {user_id}
    # POST   /admin/api/users/promote  body: {user_id}
    # POST   /admin/api/users/demote   body: {user_id}
    # DELETE /admin/api/users/:id
    # =========================================================================

    def get_admin_stats(self):
        """GET /admin/api/stats  (hard auth + is_admin)"""
        logger.debug("Loading admin stats")
        return self._make_request("GET", "/admin/api/stats")

    def get_admin_metrics(self):
        """GET /admin/api/metrics  (hard auth + is_admin)"""
        logger.debug("Loading admin metrics")
        return self._make_request("GET", "/admin/api/metrics")

    def get_admin_users(self):
        """GET /admin/api/users  (light auth + is_admin)"""
        logger.info("Loading admin users")
        return self._make_request("GET", "/admin/api/users")

    def get_admin_sessions(self):
        """GET /admin/api/sessions  (light auth + is_admin)"""
        logger.info("Loading admin sessions")
        return self._make_request("GET", "/admin/api/sessions")

    def admin_ban_user(self, user_id):
        """POST /admin/api/users/ban  body: {user_id}  (hard auth + is_admin)"""
        logger.info("Banning user", extra_info=f"User ID: {user_id}")
        r = self._make_request("POST", "/admin/api/users/ban", {"user_id": user_id})
        if r.get("success"):
            logger.info("User banned", extra_info=f"User ID: {user_id}")
        else:
            logger.warning("Ban failed", extra_info=r.get("error"))
        return r

    def admin_unban_user(self, user_id):
        """POST /admin/api/users/unban  body: {user_id}  (hard auth + is_admin)"""
        logger.info("Unbanning user", extra_info=f"User ID: {user_id}")
        r = self._make_request("POST", "/admin/api/users/unban", {"user_id": user_id})
        if r.get("success"):
            logger.info("User unbanned", extra_info=f"User ID: {user_id}")
        else:
            logger.warning("Unban failed", extra_info=r.get("error"))
        return r

    def admin_promote_user(self, user_id):
        """POST /admin/api/users/promote  body: {user_id}  (hard auth + is_admin)"""
        logger.info("Promoting user", extra_info=f"User ID: {user_id}")
        return self._make_request("POST", "/admin/api/users/promote", {"user_id": user_id})

    def admin_demote_user(self, user_id):
        """POST /admin/api/users/demote  body: {user_id}  (hard auth + is_admin)"""
        logger.info("Demoting user", extra_info=f"User ID: {user_id}")
        return self._make_request("POST", "/admin/api/users/demote", {"user_id": user_id})

    def admin_delete_user(self, user_id):
        """DELETE /admin/api/users/:id  (hard auth + is_admin)"""
        logger.info("Deleting user (admin)", extra_info=f"User ID: {user_id}")
        return self._make_request("DELETE", f"/admin/api/users/{user_id}", {})

    # =========================================================================
    # Shutdown
    # =========================================================================

    def shutdown(self):
        """Signal all background SSE threads to exit cleanly. Safe to call multiple times."""
        logger.info("API client shutdown requested")
        self._shutdown.set()
