"""
Message Cache & HTTP Response Cache
In-memory caching with TTL (Time To Live) support
"""

import time
import hashlib
from collections import defaultdict, OrderedDict
from threading import Lock
from config import MESSAGE_CACHE_LIMIT, HTTP_CACHE_LIMIT, HTTP_CACHE_TTL
from logger import logger


class MessageCache:
    """In-memory message cache with history"""

    def __init__(self, limit=MESSAGE_CACHE_LIMIT):
        self.messages = defaultdict(list)  # chat_id -> [messages]
        self.limit = limit
        self.lock = Lock()
        logger.info(
            "MessageCache initialized", extra_info=f"Max messages per chat: {limit}"
        )

    def add_message(self, chat_id, message):
        """Add message to cache"""
        with self.lock:
            try:
                self.messages[chat_id].append(message)

                # Keep only recent messages
                if len(self.messages[chat_id]) > self.limit:
                    removed_count = len(self.messages[chat_id]) - self.limit
                    self.messages[chat_id] = self.messages[chat_id][-self.limit :]
                    logger.debug(
                        f"Cache limit reached for chat {chat_id}",
                        extra_info=f"Removed {removed_count} old messages",
                    )

                logger.debug(
                    "Message added to cache",
                    extra_info=f"Chat: {chat_id}, Total: {len(self.messages[chat_id])}",
                )
            except Exception as e:
                logger.exception(
                    "Failed to add message to cache", error_detail=str(e), stop=False
                )

    def get_messages(self, chat_id):
        """Get all cached messages for chat"""
        with self.lock:
            try:
                messages = self.messages.get(chat_id, [])
                logger.debug(
                    "Retrieved cached messages",
                    extra_info=f"Chat: {chat_id}, Count: {len(messages)}",
                )
                return messages
            except Exception as e:
                logger.warning(
                    "Failed to retrieve cached messages",
                    extra_info=f"Chat: {chat_id}, Error: {str(e)}",
                )
                return []

    def clear_chat(self, chat_id):
        """Clear messages for a chat"""
        with self.lock:
            try:
                if chat_id in self.messages:
                    count = len(self.messages[chat_id])
                    del self.messages[chat_id]
                    logger.info(
                        "Cleared chat cache",
                        extra_info=f"Chat: {chat_id}, Cleared: {count} messages",
                    )
                else:
                    logger.debug(f"Chat not in cache: {chat_id}")
            except Exception as e:
                logger.warning(
                    "Failed to clear chat cache",
                    extra_info=f"Chat: {chat_id}, Error: {str(e)}",
                )

    def clear_all(self):
        """Clear all cache"""
        with self.lock:
            try:
                count = sum(len(msgs) for msgs in self.messages.values())
                chat_count = len(self.messages)
                self.messages.clear()
                logger.info(
                    "Cleared entire cache",
                    extra_info=f"Chats: {chat_count}, Messages: {count}",
                )
            except Exception as e:
                logger.warning("Failed to clear cache", extra_info=str(e))


class CacheEntry:
    """Individual cache entry with metadata."""

    def __init__(self, data, ttl=300):
        self.data = data
        self.created_at = time.time()
        self.ttl = ttl
        self.access_count = 0
        self.last_accessed = time.time()

    def is_expired(self):
        """Check if cache entry has expired."""
        return time.time() - self.created_at > self.ttl

    def touch(self):
        """Update access metadata."""
        self.access_count += 1
        self.last_accessed = time.time()


class HTTPCache:
    """
    LRU (Least Recently Used) HTTP response cache with TTL support.

    Features:
    - Automatic expiration of old entries
    - Size limiting (evicts oldest when full)
    - Thread-safe operations
    - Hit/miss statistics
    """

    def __init__(self, max_size=HTTP_CACHE_LIMIT, default_ttl=HTTP_CACHE_TTL):
        self.max_size = max_size
        self.default_ttl = default_ttl
        self.cache = OrderedDict()  # Maintains insertion order for LRU
        self.lock = Lock()
        self.stats = {"hits": 0, "misses": 0, "evictions": 0}

        logger.info(
            "HTTPCache initialized",
            extra_info=f"Max size: {max_size}, Default TTL: {default_ttl}s",
        )

    def _generate_key(self, method, url, data=None):
        """Generate cache key from request parameters."""
        key_str = f"{method}:{url}:{str(data)}"
        return hashlib.md5(key_str.encode()).hexdigest()

    def get(self, key):
        """
        Retrieve item from cache.
        Returns None if not found or expired.
        """
        with self.lock:
            if key not in self.cache:
                self.stats["misses"] += 1
                return None

            entry = self.cache[key]

            if entry.is_expired():
                del self.cache[key]
                self.stats["misses"] += 1
                logger.debug(f"Cache entry expired: {key[:8]}...")
                return None

            # Move to end (most recently used)
            self.cache.move_to_end(key)
            entry.touch()
            self.stats["hits"] += 1

            return entry.data

    def set(self, key, data, ttl=None):
        """
        Store item in cache.
        If cache is full, evicts oldest entry.
        """
        if ttl is None:
            ttl = self.default_ttl

        with self.lock:
            # If key exists, update it and move to end
            if key in self.cache:
                self.cache.move_to_end(key)
                self.cache[key] = CacheEntry(data, ttl)
                return

            # Evict oldest if at capacity
            while len(self.cache) >= self.max_size:
                oldest_key, oldest_entry = self.cache.popitem(last=False)
                self.stats["evictions"] += 1
                logger.debug(f"Evicted cache entry: {oldest_key[:8]}...")

            self.cache[key] = CacheEntry(data, ttl)
            logger.debug(f"Cached response: {key[:8]}... (TTL: {ttl}s)")

    def invalidate(self, pattern=None):
        """
        Invalidate cache entries.
        If pattern is None, clears all. Otherwise removes keys containing pattern.
        """
        with self.lock:
            if pattern is None:
                count = len(self.cache)
                self.cache.clear()
                logger.info(f"Invalidated all cache entries ({count} items)")
            else:
                to_remove = [k for k in self.cache.keys() if pattern in k]
                for k in to_remove:
                    del self.cache[k]
                logger.info(
                    f"Invalidated {len(to_remove)} entries matching '{pattern}'"
                )

    def clear(self):
        """Clear all cache entries."""
        self.invalidate(None)

    def get_stats(self):
        """Get cache statistics."""
        with self.lock:
            total = self.stats["hits"] + self.stats["misses"]
            hit_rate = (self.stats["hits"] / total * 100) if total > 0 else 0
            return {
                "size": len(self.cache),
                "max_size": self.max_size,
                "hits": self.stats["hits"],
                "misses": self.stats["misses"],
                "evictions": self.stats["evictions"],
                "hit_rate": f"{hit_rate:.1f}%",
            }

    def cleanup_expired(self):
        """Remove all expired entries. Returns count removed."""
        with self.lock:
            expired = [key for key, entry in self.cache.items() if entry.is_expired()]
            for key in expired:
                del self.cache[key]
            if expired:
                logger.debug(f"Cleaned up {len(expired)} expired cache entries")
            return len(expired)


class CompressedCache:
    """
    Cache that stores compressed data to reduce memory usage.
    Useful for large text responses (like message history).
    """

    def __init__(self, max_size=100):
        self.max_size = max_size
        self.cache = OrderedDict()
        self.lock = Lock()

    def get(self, key):
        """Get and decompress data."""
        import gzip
        import json

        with self.lock:
            if key not in self.cache:
                return None

            compressed, timestamp, ttl = self.cache[key]
            if time.time() - timestamp > ttl:
                del self.cache[key]
                return None

            self.cache.move_to_end(key)
            try:
                decompressed = gzip.decompress(compressed)
                return json.loads(decompressed.decode("utf-8"))
            except Exception:
                return None

    def set(self, key, data, ttl=300):
        """Compress and store data."""
        import gzip
        import json

        with self.lock:
            json_bytes = json.dumps(data).encode("utf-8")
            compressed = gzip.compress(json_bytes)

            while len(self.cache) >= self.max_size:
                self.cache.popitem(last=False)

            self.cache[key] = (compressed, time.time(), ttl)
