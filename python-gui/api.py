"""
Chat API Client
HTTP client with automatic cookie jar management and reconnect logic
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
    """Enhanced HTTP client with retry logic and better error handling"""

    def __init__(self, server_url=DEFAULT_SERVER):
        self.server_url = server_url.rstrip("/")
        self.cookie_jar = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(
            urllib.request.HTTPCookieProcessor(self.cookie_jar)
        )
        self.user_agent = USER_AGENT
        self.message_cache = MessageCache()
        self.last_error = None
        
        logger.info(
            "ChatAPIClient initialized",
            extra_info=f"Server: {self.server_url}"
        )

    def _make_request(
        self, method, path, data=None, headers=None, timeout=CONNECTION_TIMEOUT
    ):
        """Make HTTP request with error handling"""
        url = f"{self.server_url}{path}"

        request_headers = {
            "User-Agent": self.user_agent,
            "Content-Type": "application/json",
        }
        if headers:
            request_headers.update(headers)

        if data:
            if isinstance(data, dict):
                data = json.dumps(data).encode("utf-8")
            elif isinstance(data, str):
                data = data.encode("utf-8")

        try:
            logger.debug(
                f"Making {method} request",
                extra_info=f"URL: {url}"
            )
            
            req = urllib.request.Request(
                url, data=data, headers=request_headers, method=method
            )

            response = self.opener.open(req, timeout=timeout)
            response_data = response.read()

            self.last_error = None
            request = {
                "status": response.status,
                "data": response_data.decode("utf-8"),
                "headers": dict(response.headers),
                "success": True,
            }
            
            logger.debug(
                f"Request successful",
                extra_info=f"Status: {response.status}, Size: {len(response_data)} bytes"
            )
            return request

        except HTTPError as e:
            error_data = e.read().decode("utf-8")
            self.last_error = f"HTTP {e.code}: {error_data[:100]}"
            request = {
                "status": e.code,
                "data": error_data,
                "error": str(e),
                "success": False,
            }
            
            logger.warning(
                f"HTTP Error {e.code}",
                extra_info=f"URL: {url}, Error: {self.last_error}"
            )
            return request
            
        except URLError as e:
            self.last_error = f"Connection error: {str(e)}"
            request = {
                "status": 0,
                "error": f"Connection error: {str(e)}",
                "success": False,
            }
            
            logger.warning(
                "Connection error",
                extra_info=f"URL: {url}, Error: {str(e)}"
            )
            return request
            
        except Exception as e:
            self.last_error = str(e)
            request = {"status": 0, "error": str(e), "success": False}
            
            logger.exception(
                "Unexpected error in API request",
                error_detail=f"URL: {url}, Method: {method}, Error: {str(e)}",
                stop=False
            )
            return request

    # ========================================================================
    # Authentication
    # ========================================================================

    def login(self, username, password):
        """POST /api/login"""
        logger.info(f"Login request", extra_info=f"Username: {username}")
        
        data = {"username": username, "password": password}
        response = self._make_request("POST", "/api/login", data)
        
        if response.get("success"):
            logger.info("Login successful", extra_info=f"Username: {username}")
        else:
            logger.warning(
                "Login failed",
                extra_info=f"Username: {username}, Error: {response.get('error')}"
            )
        
        return response

    def register(self, email, username, password, full_name):
        """POST /api/register"""
        logger.info(
            "Register request",
            extra_info=f"Email: {email}, Username: {username}"
        )
        
        data = {
            "email": email,
            "username": username,
            "password": password,
            "full_name": full_name,
        }
        response = self._make_request("POST", "/api/register", data)
        
        if response.get("success"):
            logger.info(
                "Registration successful",
                extra_info=f"Email: {email}, Username: {username}"
            )
        else:
            logger.warning(
                "Registration failed",
                extra_info=f"Email: {email}, Error: {response.get('error')}"
            )
        
        return response

    # ========================================================================
    # User Profile
    # ========================================================================

    def get_profile(self):
        """GET /api/profile"""
        logger.debug("Fetching user profile")
        response = self._make_request("GET", "/api/profile")
        
        if response.get("success"):
            logger.debug("User profile fetched successfully")
        else:
            logger.warning("Failed to fetch user profile")
        
        return response

    def update_profile(self, data):
        """PUT /api/profile"""
        logger.info("Updating user profile")
        response = self._make_request("PUT", "/api/profile", data)
        
        if response.get("success"):
            logger.info("User profile updated successfully")
        else:
            logger.warning(
                "Failed to update user profile",
                extra_info=response.get("error")
            )
        
        return response

    def upload_avatar(self, file_path):
        """Upload profile avatar"""
        logger.debug(f"Avatar upload attempted", extra_info=f"File: {file_path}")
        logger.warning("Avatar upload not yet implemented")
        return {"status": 501, "error": "Avatar upload not yet implemented"}

    # ========================================================================
    # Chat Operations
    # ========================================================================

    def get_conversations(self):
        """GET /api/conversations"""
        logger.debug("Loading conversations")
        response = self._make_request("GET", "/api/conversations")
        
        if response.get("success"):
            try:
                data = json.loads(response["data"])
                conv_count = len(data) if isinstance(data, list) else 0
                logger.info(
                    "Conversations loaded",
                    extra_info=f"Count: {conv_count}"
                )
            except:
                logger.debug("Conversations loaded successfully")
        else:
            logger.warning("Failed to load conversations")
        
        return response

    def get_messages(self, chat_id, limit=50, offset=0):
        """GET /api/messages?chat_id=X"""
        logger.debug(
            "Loading messages",
            extra_info=f"Chat: {chat_id}, Limit: {limit}, Offset: {offset}"
        )
        
        path = f"/api/messages?chat_id={chat_id}&limit={limit}&offset={offset}"
        response = self._make_request("GET", path)
        
        if response.get("success"):
            logger.debug(
                "Messages loaded",
                extra_info=f"Chat: {chat_id}"
            )
        else:
            logger.warning(
                "Failed to load messages",
                extra_info=f"Chat: {chat_id}"
            )
        
        return response

    def send_message(self, chat_id, content):
        """POST /api/messages"""
        logger.debug(
            "Sending message",
            extra_info=f"Chat: {chat_id}, Content length: {len(content)}"
        )
        
        data = {"chat_id": chat_id, "content": content}
        response = self._make_request("POST", "/api/messages", data)
        
        if response.get("success"):
            logger.info(
                "Message sent",
                extra_info=f"Chat: {chat_id}"
            )
        else:
            logger.warning(
                "Failed to send message",
                extra_info=f"Chat: {chat_id}, Error: {response.get('error')}"
            )
        
        return response

    def delete_message(self, message_id):
        """DELETE /api/messages/:id"""
        logger.info(
            "Deleting message",
            extra_info=f"Message ID: {message_id}"
        )
        
        response = self._make_request("DELETE", f"/api/messages/{message_id}", {})
        
        if response.get("success"):
            logger.info(
                "Message deleted",
                extra_info=f"Message ID: {message_id}"
            )
        else:
            logger.warning(
                "Failed to delete message",
                extra_info=f"Message ID: {message_id}"
            )
        
        return response

    # ========================================================================
    # Real-time Streaming
    # ========================================================================

    def stream_messages(self, chat_id, on_message, on_error, on_connect=None):
        """SSE stream /api/stream?chat_id=X with reconnect logic"""
        logger.info(
            "Starting SSE stream",
            extra_info=f"Chat ID: {chat_id}"
        )
        
        url = f"{self.server_url}/api/stream?chat_id={chat_id}"

        def stream_thread():
            reconnect_count = 0
            max_reconnects = 5

            while reconnect_count < max_reconnects:
                try:
                    logger.debug(
                        "Opening SSE connection",
                        extra_info=f"Chat: {chat_id}, Attempt: {reconnect_count + 1}"
                    )
                    
                    req = urllib.request.Request(
                        url, headers={"User-Agent": self.user_agent}
                    )
                    response = self.opener.open(req, timeout=CONNECTION_TIMEOUT)

                    if on_connect:
                        on_connect()
                    
                    logger.info(
                        "SSE stream connected",
                        extra_info=f"Chat: {chat_id}"
                    )

                    reconnect_count = 0  # Reset on successful connect

                    for line in response:
                        line = line.decode("utf-8").strip()

                        if line.startswith("event:"):
                            event_type = line.split(":", 1)[1].strip()
                        elif line.startswith("data:"):
                            data = line.split(":", 1)[1].strip()
                            try:
                                logger.debug(
                                    f"SSE event received",
                                    extra_info=f"Chat: {chat_id}, Type: {event_type}"
                                )
                                on_message(event_type, json.loads(data))
                            except json.JSONDecodeError:
                                logger.warning(
                                    "Failed to parse SSE data",
                                    extra_info=f"Chat: {chat_id}"
                                )
                        elif line == "":
                            pass

                except Exception as e:
                    reconnect_count += 1
                    error_msg = str(e)
                    on_error(error_msg)
                    
                    logger.warning(
                        "SSE stream error",
                        extra_info=f"Chat: {chat_id}, Attempt: {reconnect_count}, Error: {error_msg}"
                    )

                    if reconnect_count < max_reconnects:
                        wait_time = min(RECONNECT_TIMEOUT * reconnect_count, 30)
                        logger.debug(
                            f"SSE reconnecting",
                            extra_info=f"Chat: {chat_id}, Wait: {wait_time}s"
                        )
                        time.sleep(wait_time)
                    else:
                        final_error = f"Stream disconnected after {max_reconnects} reconnect attempts"
                        on_error(final_error)
                        logger.exception(
                            "SSE stream failed permanently",
                            error_detail=f"Chat: {chat_id}, Max retries exceeded",
                            stop=False
                        )

        thread = threading.Thread(target=stream_thread, daemon=True)
        thread.start()
        
        logger.debug(f"SSE stream thread started for chat {chat_id}")
        return thread

    # ========================================================================
    # Admin Operations
    # ========================================================================

    def get_admin_users(self):
        """GET /admin/api/users"""
        logger.info("Loading admin users list")
        response = self._make_request("GET", "/admin/api/users")
        
        if response.get("success"):
            logger.info("Admin users list loaded successfully")
        else:
            logger.warning("Failed to load admin users list")
        
        return response

    def admin_ban_user(self, user_id):
        """POST /admin/api/users/:id/ban"""
        logger.info(
            "Banning user",
            extra_info=f"User ID: {user_id}"
        )
        
        response = self._make_request("POST", f"/admin/api/users/{user_id}/ban", {})
        
        if response.get("success"):
            logger.info(
                "User banned",
                extra_info=f"User ID: {user_id}"
            )
        else:
            logger.warning(
                "Failed to ban user",
                extra_info=f"User ID: {user_id}"
            )
        
        return response

    def admin_unban_user(self, user_id):
        """DELETE /admin/api/users/:id/ban"""
        logger.info(
            "Unbanning user",
            extra_info=f"User ID: {user_id}"
        )
        
        response = self._make_request("DELETE", f"/admin/api/users/{user_id}/ban", {})
        
        if response.get("success"):
            logger.info(
                "User unbanned",
                extra_info=f"User ID: {user_id}"
            )
        else:
            logger.warning(
                "Failed to unban user",
                extra_info=f"User ID: {user_id}"
            )
        
        return response

    def get_admin_stats(self):
        """GET /admin/api/stats"""
        logger.debug("Loading admin statistics")
        response = self._make_request("GET", "/admin/api/stats")
        
        if response.get("success"):
            logger.info("Admin statistics loaded successfully")
        else:
            logger.warning("Failed to load admin statistics")
        
        return response
