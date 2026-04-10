"""
Chat API Client - Enhanced with Compression, Advanced Caching, and Rate Limiting
HTTP client with automatic cookie jar management, reconnect logic,
ThreadPoolExecutor, compression support, and client-side rate limiting.
"""

import urllib.request
import urllib.parse
import json
import http.cookiejar
import threading
import time
import gzip
import zlib
import io
from urllib.error import URLError, HTTPError
from concurrent.futures import ThreadPoolExecutor

import mimetypes
import os
import uuid

from config import (
    DEFAULT_SERVER,
    USER_AGENT,
    CONNECTION_TIMEOUT,
    RECONNECT_TIMEOUT,
    ENABLE_COMPRESSION,
    MAX_RETRIES,
    RATE_LIMIT_REQUESTS,
    RATE_LIMIT_WINDOW,
)
from cache import MessageCache, HTTPCache
from logger import logger


class RateLimiter:
    """Client-side rate limiter using token bucket algorithm."""

    def __init__(self, max_requests=100, window_seconds=60):
        self.max_requests = max_requests
        self.window = window_seconds
        self.tokens = max_requests
        self.last_update = time.time()
        self.lock = threading.Lock()

    def acquire(self):
        """Acquire a token, blocking if necessary."""
        with self.lock:
            now = time.time()
            elapsed = now - self.last_update
            self.tokens = min(
                self.max_requests,
                self.tokens + elapsed * (self.max_requests / self.window),
            )
            self.last_update = now

            if self.tokens < 1:
                sleep_time = (1 - self.tokens) * (self.window / self.max_requests)
                logger.debug(f"Rate limit hit, sleeping {sleep_time:.2f}s")
                time.sleep(sleep_time)
                self.tokens = 0
            else:
                self.tokens -= 1

    def try_acquire(self):
        """Try to acquire token without blocking. Returns True if successful."""
        with self.lock:
            now = time.time()
            elapsed = now - self.last_update
            self.tokens = min(
                self.max_requests,
                self.tokens + elapsed * (self.max_requests / self.window),
            )
            self.last_update = now

            if self.tokens >= 1:
                self.tokens -= 1
                return True
            return False


class CompressionHandler(urllib.request.BaseHandler):
    """Handler to automatically decompress gzip/deflate responses."""

    def http_response(self, request, response):
        return self._decompress(response)

    def https_response(self, request, response):
        return self._decompress(response)

    def _decompress(self, response):
        encoding = response.headers.get("Content-Encoding", "").lower()

        if encoding == "gzip":
            logger.debug("Decompressing gzip response")
            response = urllib.request.addinfourl(
                gzip.GzipFile(fileobj=io.BytesIO(response.read())),
                response.headers,
                response.url,
                response.code,
            )
        elif encoding == "deflate":
            logger.debug("Decompressing deflate response")
            raw_data = response.read()
            try:
                decompressed = zlib.decompress(raw_data)
            except zlib.error:
                # Try without header (raw deflate)
                decompressed = zlib.decompress(raw_data, -15)
            response = urllib.request.addinfourl(
                io.BytesIO(decompressed), response.headers, response.url, response.code
            )

        return response


class ChatAPIClient:
    """HTTP client for the Rust chat server with optimized thread pool."""

    def __init__(self, server_url=DEFAULT_SERVER):
        self.server_url = server_url.rstrip("/")
        self.cookie_jar = http.cookiejar.CookieJar()

        # Build opener with compression handler
        handlers = [urllib.request.HTTPCookieProcessor(self.cookie_jar)]
        if ENABLE_COMPRESSION:
            handlers.append(CompressionHandler())

        self.opener = urllib.request.build_opener(*handlers)
        self.user_agent = USER_AGENT
        self.message_cache = MessageCache()
        self.http_cache = HTTPCache()
        self.last_error = None
        self._shutdown = threading.Event()

        # Rate limiter
        self.rate_limiter = RateLimiter(RATE_LIMIT_REQUESTS, RATE_LIMIT_WINDOW)

        self.executor = ThreadPoolExecutor(
            max_workers=4, thread_name_prefix="api-worker"
        )

        logger.info(
            "ChatAPIClient initialized",
            extra_info=f"Server: {self.server_url}, Compression: {ENABLE_COMPRESSION}",
        )

    def _make_request(
        self,
        method,
        path,
        data=None,
        headers=None,
        timeout=CONNECTION_TIMEOUT,
        use_cache=False,
        cache_ttl=300,
    ):
        """Make an HTTP request with compression, retries, and caching."""
        url = f"{self.server_url}{path}"
        cache_key = f"{method}:{url}:{hash(str(data))}"

        # Check cache for GET requests
        if use_cache and method == "GET":
            cached = self.http_cache.get(cache_key)
            if cached:
                logger.debug(f"Cache hit for {url}")
                cached["from_cache"] = True
                return cached

        # Rate limiting
        self.rate_limiter.acquire()

        request_headers = {
            "User-Agent": self.user_agent,
            "Accept": "application/json",
            "Accept-Encoding": "gzip, deflate" if ENABLE_COMPRESSION else "identity",
        }
        if headers:
            request_headers.update(headers)

        if data is not None:
            if isinstance(data, dict):
                data = json.dumps(data).encode("utf-8")
                request_headers["Content-Type"] = "application/json"
            elif isinstance(data, str):
                data = data.encode("utf-8")

        # Retry logic with exponential backoff
        for attempt in range(MAX_RETRIES):
            try:
                logger.debug(
                    "Making request",
                    extra_info=f"{method} {url} (attempt {attempt + 1})",
                )
                req = urllib.request.Request(
                    url, data=data, headers=request_headers, method=method
                )
                response = self.opener.open(req, timeout=timeout)
                body = response.read()
                self.last_error = None

                result = {
                    "status": response.status,
                    "data": body.decode("utf-8"),
                    "headers": dict(response.headers),
                    "success": True,
                    "from_cache": False,
                }

                # Cache successful GET requests
                if use_cache and method == "GET":
                    self.http_cache.set(cache_key, result, ttl=cache_ttl)

                logger.debug(
                    "Request OK",
                    extra_info=f"HTTP {response.status}, {len(body)} bytes, encoded: {response.headers.get('Content-Encoding', 'none')}",
                )
                return result

            except HTTPError as e:
                body = e.read().decode("utf-8")
                self.last_error = f"HTTP {e.code}: {body[:100]}"
                logger.warning(
                    f"HTTP {e.code}", extra_info=f"{url} — {self.last_error}"
                )

                # Don't retry client errors (4xx) except 429 (rate limited)
                if 400 <= e.code < 500 and e.code != 429:
                    return {
                        "status": e.code,
                        "data": body,
                        "error": str(e),
                        "success": False,
                    }

                if attempt < MAX_RETRIES - 1:
                    wait = min(2**attempt, 30)
                    logger.info(f"Retrying in {wait}s...")
                    time.sleep(wait)
                else:
                    return {
                        "status": e.code,
                        "data": body,
                        "error": str(e),
                        "success": False,
                    }

            except URLError as e:
                self.last_error = f"Connection error: {e}"
                logger.warning("Connection error", extra_info=f"{url} — {e}")
                if attempt < MAX_RETRIES - 1:
                    time.sleep(min(2**attempt, 30))
                else:
                    return {"status": 0, "error": self.last_error, "success": False}

            except Exception as e:
                self.last_error = str(e)
                logger.exception(
                    "Unexpected request error",
                    error_detail=f"{method} {url} — {e}",
                    stop=False,
                )
                return {"status": 0, "error": self.last_error, "success": False}

    def _make_multipart_request(
        self,
        path,
        file_path,
        field_name="file",
        extra_fields=None,
        timeout=CONNECTION_TIMEOUT,
        compress_upload=False,
    ):
        """POST a multipart/form-data request (for file / avatar uploads)."""
        url = f"{self.server_url}{path}"
        boundary = uuid.uuid4().hex

        self.rate_limiter.acquire()

        parts = []
        for name, value in (extra_fields or {}).items():
            parts.append(
                f"--{boundary}\r\n"
                f'Content-Disposition: form-data; name="{name}"\r\n\r\n'
                f"{value}\r\n"
            )
        filename = os.path.basename(file_path)
        mime_type = mimetypes.guess_type(file_path)[0] or "application/octet-stream"

        with open(file_path, "rb") as fh:
            file_data = fh.read()

        # Compress file data if requested and large enough
        if compress_upload and len(file_data) > 1024:
            file_data = gzip.compress(file_data)
            filename += ".gz"
            logger.debug(f"Compressed upload file: {len(file_data)} bytes")

        header = (
            f"--{boundary}\r\n"
            f'Content-Disposition: form-data; name="{field_name}"; filename="{filename}"\r\n'
            f"Content-Type: {mime_type}\r\n"
        )
        if compress_upload:
            header += "Content-Encoding: gzip\r\n"
        header += "\r\n"

        footer = f"\r\n--{boundary}--\r\n"

        body = b"".join(
            [p.encode() for p in parts] + [header.encode(), file_data, footer.encode()]
        )

        req = urllib.request.Request(
            url,
            data=body,
            method="POST",
            headers={
                "User-Agent": self.user_agent,
                "Content-Type": f"multipart/form-data; boundary={boundary}",
                "Accept-Encoding": (
                    "gzip, deflate" if ENABLE_COMPRESSION else "identity"
                ),
            },
        )

        try:
            logger.debug(
                "Making multipart request", extra_info=f"POST {url}, file={filename}"
            )
            response = self.opener.open(req, timeout=timeout)
            body = response.read()
            logger.debug("Multipart OK", extra_info=f"HTTP {response.status}")
            return {
                "status": response.status,
                "data": body.decode("utf-8"),
                "headers": dict(response.headers),
                "success": True,
            }
        except HTTPError as e:
            body = e.read().decode("utf-8")
            logger.warning(f"HTTP {e.code}", extra_info=f"{url} — {body[:100]}")
            return {"status": e.code, "data": body, "error": str(e), "success": False}
        except Exception as e:
            logger.exception("Multipart request error", error_detail=str(e), stop=False)
            return {"status": 0, "error": str(e), "success": False}

    # Standard API methods with caching hints
    def login(self, username, password):
        logger.info("Attempting login", extra_info=f"Username: {username}")
        return self._make_request(
            "POST",
            "/api/login",
            {"username": username, "password": password},
            use_cache=False,
        )

    def register(self, email, username, password, fullname="", confirm_password=""):
        logger.info("Attempting registration", extra_info=f"Username: {username}")
        return self._make_request(
            "POST",
            "/api/register",
            {
                "username": username,
                "email": email,
                "password": password,
                "confirm_password": confirm_password,
                "full_name": fullname,
            },
            use_cache=False,
        )

    def logout(self):
        logger.info("Logging out")
        result = self._make_request("POST", "/api/logout", {}, use_cache=False)
        self.clear_cache()
        return result

    def logout_all_sessions(self):
        logger.info("Logging out all sessions")
        return self._make_request(
            "POST", "/api/settings/logout-all", {}, use_cache=False
        )

    def get_profile(self, use_cache=True):
        logger.debug("Loading profile")
        return self._make_request(
            "GET", "/api/profile", use_cache=use_cache, cache_ttl=60
        )

    def update_profile(self, **kwargs):
        logger.info("Updating profile", extra_info=f"Fields: {list(kwargs.keys())}")
        return self._make_request(
            "POST", "/api/profile/update", kwargs, use_cache=False
        )

    def set_avatar(self, file_path, compress=False):
        logger.info("Uploading avatar", extra_info=f"File: {file_path}")
        return self._make_multipart_request(
            "/api/profile/avatar", file_path, compress_upload=compress
        )

    def delete_account(self):
        logger.warning("Deleting account")
        return self._make_request("DELETE", "/api/settings/delete", {}, use_cache=False)

    def get_chats(self, use_cache=True):
        logger.debug("Loading chats")
        return self._make_request(
            "GET", "/api/chats", use_cache=use_cache, cache_ttl=30
        )

    def create_chat(self, user_id):
        logger.info("Creating chat", extra_info=f"User ID: {user_id}")
        return self._make_request(
            "POST", "/api/chats", {"user_id": user_id}, use_cache=False
        )

    def get_messages(self, chat_id, limit=50, offset=0, use_cache=True):
        logger.debug("Loading messages", extra_info=f"Chat: {chat_id}")
        url = f"/api/messages?chat_id={chat_id}&limit={limit}&offset={offset}"
        return self._make_request("GET", url, use_cache=use_cache, cache_ttl=10)

    def send_message(self, chat_id, content, message_type="text"):
        logger.debug("Sending message", extra_info=f"Chat: {chat_id}")
        return self._make_request(
            "POST",
            "/api/messages/send",
            {"chat_id": chat_id, "content": content, "message_type": message_type},
            use_cache=False,
        )

    def mark_message_read(self, message_id):
        logger.debug("Marking message read", extra_info=f"Message ID: {message_id}")
        return self._make_request(
            "POST", f"/api/messages/{message_id}/read", {}, use_cache=False
        )

    def send_typing_indicator(self, chat_id):
        logger.debug("Sending typing indicator", extra_info=f"Chat: {chat_id}")
        return self._make_request(
            "POST", "/api/typing", {"chat_id": chat_id}, use_cache=False
        )

    def get_groups(self, use_cache=True):
        logger.debug("Loading groups")
        return self._make_request(
            "GET", "/api/groups", use_cache=use_cache, cache_ttl=30
        )

    def get_group_info(self, group_id, use_cache=True):
        logger.debug("Loading group info", extra_info=f"Group: {group_id}")
        return self._make_request(
            "GET", f"/api/groups/{group_id}", use_cache=use_cache, cache_ttl=60
        )

    def get_group_members(self, group_id, use_cache=True):
        logger.debug("Loading group members", extra_info=f"Group: {group_id}")
        return self._make_request(
            "GET", f"/api/groups/{group_id}/members", use_cache=use_cache, cache_ttl=60
        )

    def create_group(self, name, member_ids=None, description=""):
        logger.info("Creating group", extra_info=f"Name: {name}")
        body = {"name": name, "description": description}
        if member_ids:
            body["member_ids"] = member_ids
        return self._make_request("POST", "/api/groups", body, use_cache=False)

    def update_group(self, group_id, **kwargs):
        logger.info("Updating group", extra_info=f"Group: {group_id}")
        return self._make_request(
            "PATCH", f"/api/groups/{group_id}", kwargs, use_cache=False
        )

    def delete_group(self, group_id):
        logger.warning("Deleting group", extra_info=f"Group: {group_id}")
        return self._make_request(
            "DELETE", f"/api/groups/{group_id}", {}, use_cache=False
        )

    def add_group_member(self, group_id, user_id):
        logger.info(
            "Adding group member", extra_info=f"Group: {group_id}, User: {user_id}"
        )
        return self._make_request(
            "POST",
            f"/api/groups/{group_id}/members",
            {"user_id": user_id},
            use_cache=False,
        )

    def remove_group_member(self, group_id, user_id):
        logger.info(
            "Removing group member", extra_info=f"Group: {group_id}, User: {user_id}"
        )
        return self._make_request(
            "DELETE",
            f"/api/groups/{group_id}/members",
            {"user_id": user_id},
            use_cache=False,
        )

    def search_users(self, query):
        logger.debug("Searching users", extra_info=f"Query: {query}")
        url = f"/api/users/search?q={urllib.parse.quote(query)}"
        return self._make_request("GET", url, use_cache=False)  # Don't cache search

    def get_avatar(self, user_id, use_cache=True):
        logger.debug("Loading avatar", extra_info=f"User: {user_id}")
        return self._make_request(
            "GET", f"/api/avatar/{user_id}", use_cache=use_cache, cache_ttl=300
        )

    def get_files(self, chat_id=None, use_cache=True):
        if chat_id:
            logger.debug("Loading files", extra_info=f"Chat: {chat_id}")
            return self._make_request(
                "GET",
                f"/api/files?chat_id={chat_id}",
                use_cache=use_cache,
                cache_ttl=60,
            )
        else:
            logger.debug("Loading all files")
            return self._make_request(
                "GET", "/api/files", use_cache=use_cache, cache_ttl=60
            )

    def get_file(self, file_id, use_cache=True):
        logger.debug("Loading file", extra_info=f"File ID: {file_id}")
        return self._make_request(
            "GET", f"/api/files/{file_id}", use_cache=use_cache, cache_ttl=60
        )

    def upload_file(self, file_path, chat_id, compress=False, progress_callback=None):
        """
        Upload file with optional progress tracking.

        Args:
            progress_callback: Callable(current_bytes, total_bytes) -> bool
                              Return False to cancel upload
        """

        logger.info("Uploading file", extra_info=f"Chat: {chat_id}, File: {file_path}")

        url = f"{self.server_url}/api/files/upload"
        boundary = uuid.uuid4().hex
        filename = os.path.basename(file_path)

        # Read file
        with open(file_path, "rb") as fh:
            file_data = fh.read()

        original_size = len(file_data)

        # Compress if requested and beneficial
        if compress and len(file_data) > 1024:
            file_data = gzip.compress(file_data)
            filename += ".gz"
            logger.debug(
                f"Compressed file from {original_size} to {len(file_data)} bytes"
            )

        # Build multipart body
        fields = {"chat_id": str(chat_id)}
        parts = []

        for name, value in fields.items():
            parts.append(
                f"--{boundary}\r\n"
                f'Content-Disposition: form-data; name="{name}"\r\n\r\n'
                f"{value}\r\n"
            )

        mime_type = mimetypes.guess_type(file_path)[0] or "application/octet-stream"
        header = (
            f"--{boundary}\r\n"
            f'Content-Disposition: form-data; name="file"; filename="{filename}"\r\n'
            f"Content-Type: {mime_type}\r\n"
        )
        if compress:
            header += "Content-Encoding: gzip\r\n"
        header += "\r\n"

        footer = f"\r\n--{boundary}--\r\n"

        # Build full body
        body_prefix = "".join(parts).encode() + header.encode()
        body_suffix = footer.encode()
        total_size = len(body_prefix) + len(file_data) + len(body_suffix)
        full_body = body_prefix + file_data + body_suffix

        # Wrap in progress-tracking reader if callback provided
        if progress_callback:

            class ProgressReader(io.BytesIO):
                def __init__(self, data, callback, total):
                    super().__init__(data)
                    self.callback = callback
                    self.total = total
                    self.read_so_far = 0

                def read(self, size=-1):
                    chunk = super().read(size)
                    if chunk:
                        self.read_so_far += len(chunk)
                        # Report every 64KB or on completion
                        if self.read_so_far % 65536 < len(chunk) or not chunk:
                            if not self.callback(self.read_so_far, self.total):
                                raise Exception("Upload cancelled")
                    return chunk

            body_stream = ProgressReader(full_body, progress_callback, total_size)
        else:
            body_stream = io.BytesIO(full_body)

        req = urllib.request.Request(
            url,
            data=body_stream,
            method="POST",
            headers={
                "User-Agent": self.user_agent,
                "Content-Type": f"multipart/form-data; boundary={boundary}",
                "Accept-Encoding": (
                    "gzip, deflate" if ENABLE_COMPRESSION else "identity"
                ),
                "Content-Length": str(total_size),
            },
        )

        try:
            self.rate_limiter.acquire()
            # Longer timeout for large uploads
            response = self.opener.open(req, timeout=CONNECTION_TIMEOUT * 5)
            body = response.read()

            result = {
                "status": response.status,
                "data": body.decode("utf-8"),
                "headers": dict(response.headers),
                "success": True,
            }

            if result.get("success"):
                logger.info(
                    "File uploaded",
                    extra_info=f"Chat: {chat_id}, Size: {original_size} bytes",
                )
            else:
                info = result.get("error") or ""
                logger.warning("File upload failed", extra_info=info)

            return result

        except HTTPError as e:
            body = e.read().decode("utf-8")
            logger.warning(f"HTTP {e.code}", extra_info=f"{url} — {body[:100]}")
            return {"status": e.code, "data": body, "error": str(e), "success": False}
        except Exception as e:
            if "cancelled" in str(e).lower():
                return {
                    "status": 0,
                    "error": "Upload cancelled",
                    "success": False,
                    "cancelled": True,
                }
            logger.exception("Upload error", error_detail=str(e))
            return {"status": 0, "error": str(e), "success": False}

    def delete_file(self, file_id):
        return self._make_request(
            "DELETE", f"/api/files/{file_id}", {}, use_cache=False
        )

    def stream_messages(
        self, chat_id, on_message, on_error, on_connect=None, limit=50, offset=0
    ):
        """Open a persistent SSE stream for chat_id."""
        logger.info("Starting SSE stream", extra_info=f"Chat: {chat_id}")
        url = f"{self.server_url}/api/stream?chat_id={chat_id}&limit={limit}&offset={offset}"

        def _run():
            reconnect_count = 0
            max_reconnects = 5
            event_type = "unknown"

            while not self._shutdown.is_set() and reconnect_count < max_reconnects:
                try:
                    logger.debug(
                        f"Opening SSE connection, attempt={reconnect_count + 1}"
                    )
                    req = urllib.request.Request(
                        url, headers={"User-Agent": self.user_agent}
                    )
                    response = self.opener.open(req, timeout=CONNECTION_TIMEOUT)

                    if on_connect:
                        on_connect()
                    logger.info("SSE connected", extra_info=f"Chat: {chat_id}")
                    reconnect_count = 0

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
                                    "SSE JSON parse error", extra_info=raw_data[:80]
                                )

                except Exception as e:
                    if self._shutdown.is_set():
                        return
                    reconnect_count += 1
                    msg = str(e)
                    on_error(msg)
                    logger.warning(
                        "SSE error", extra_info=f"Attempt {reconnect_count}, {msg}"
                    )
                    if reconnect_count < max_reconnects:
                        wait = min(RECONNECT_TIMEOUT * reconnect_count, 30)
                        for _ in range(int(wait * 10)):
                            if self._shutdown.is_set():
                                return
                            time.sleep(0.1)
                    else:
                        on_error(f"Stream failed after {max_reconnects} attempts")
                        logger.exception(
                            "SSE permanently failed",
                            error_detail=f"Chat: {chat_id}",
                            stop=False,
                        )

            logger.info("SSE stream exited", extra_info=f"Chat: {chat_id}")

        t = threading.Thread(target=_run, daemon=True, name=f"sse-chat-{chat_id}")
        t.start()
        logger.debug("SSE thread started", extra_info=f"Chat: {chat_id}")
        return t

    # Admin endpoints
    def get_admin_stats(self, use_cache=True):
        logger.debug("Loading admin stats")
        return self._make_request(
            "GET", "/admin/api/stats", use_cache=use_cache, cache_ttl=10
        )

    def get_admin_metrics(self, use_cache=True):
        logger.debug("Loading admin metrics")
        return self._make_request(
            "GET", "/admin/api/metrics", use_cache=use_cache, cache_ttl=10
        )

    def get_admin_users(self, use_cache=True):
        logger.info("Loading admin users")
        return self._make_request(
            "GET", "/admin/api/users", use_cache=use_cache, cache_ttl=30
        )

    def get_admin_sessions(self, use_cache=True):
        logger.info("Loading admin sessions")
        return self._make_request(
            "GET", "/admin/api/sessions", use_cache=use_cache, cache_ttl=30
        )

    def admin_ban_user(self, user_id):
        logger.info("Banning user", extra_info=f"User ID: {user_id}")
        return self._make_request(
            "POST", "/admin/api/users/ban", {"user_id": user_id}, use_cache=False
        )

    def admin_unban_user(self, user_id):
        logger.info("Unbanning user", extra_info=f"User ID: {user_id}")
        return self._make_request(
            "POST", "/admin/api/users/unban", {"user_id": user_id}, use_cache=False
        )

    def admin_promote_user(self, user_id):
        logger.info("Promoting user", extra_info=f"User ID: {user_id}")
        return self._make_request(
            "POST", "/admin/api/users/promote", {"user_id": user_id}, use_cache=False
        )

    def admin_demote_user(self, user_id):
        logger.info("Demoting user", extra_info=f"User ID: {user_id}")
        return self._make_request(
            "POST", "/admin/api/users/demote", {"user_id": user_id}, use_cache=False
        )

    def admin_delete_user(self, user_id):
        logger.info("Deleting user (admin)", extra_info=f"User ID: {user_id}")
        return self._make_request(
            "DELETE", f"/admin/api/users/{user_id}", {}, use_cache=False
        )

    def shutdown(self):
        """Signal all background threads to exit cleanly."""
        logger.info("API client shutdown requested")
        self._shutdown.set()

        try:
            logger.debug("Shutting down thread pool executor")
            self.executor.shutdown(wait=True, cancel_futures=True)
            logger.info("Thread pool executor shutdown complete")
        except Exception as e:
            logger.warning("Error during executor shutdown", extra_info=f"Error: {e}")

    def clear_cache(self):
        """Clear all caches."""
        self.http_cache.clear()
        self.message_cache.clear_all()
        logger.info("All caches cleared")
