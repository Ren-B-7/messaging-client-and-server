"""
Message Cache
In-memory caching for chat messages with history management
"""

from collections import defaultdict
from config import MESSAGE_CACHE_LIMIT
from logger import logger


class MessageCache:
    """In-memory message cache with history"""

    def __init__(self, limit=MESSAGE_CACHE_LIMIT):
        self.messages = defaultdict(list)  # chat_id -> [messages]
        self.limit = limit
        logger.info(
            "MessageCache initialized",
            extra_info=f"Max messages per chat: {limit}"
        )

    def add_message(self, chat_id, message):
        """Add message to cache"""
        try:
            self.messages[chat_id].append(message)
            
            # Keep only recent messages
            if len(self.messages[chat_id]) > self.limit:
                removed_count = len(self.messages[chat_id]) - self.limit
                self.messages[chat_id] = self.messages[chat_id][-self.limit :]
                logger.debug(
                    f"Cache limit reached for chat {chat_id}",
                    extra_info=f"Removed {removed_count} old messages"
                )
            
            logger.debug(
                f"Message added to cache",
                extra_info=f"Chat: {chat_id}, Total: {len(self.messages[chat_id])}"
            )
        except Exception as e:
            logger.exception(
                "Failed to add message to cache",
                error_detail=str(e),
                stop=False
            )

    def get_messages(self, chat_id):
        """Get all cached messages for chat"""
        try:
            messages = self.messages.get(chat_id, [])
            logger.debug(
                f"Retrieved cached messages",
                extra_info=f"Chat: {chat_id}, Count: {len(messages)}"
            )
            return messages
        except Exception as e:
            logger.warning(
                "Failed to retrieve cached messages",
                extra_info=f"Chat: {chat_id}, Error: {str(e)}"
            )
            return []

    def clear_chat(self, chat_id):
        """Clear messages for a chat"""
        try:
            if chat_id in self.messages:
                count = len(self.messages[chat_id])
                del self.messages[chat_id]
                logger.info(
                    f"Cleared chat cache",
                    extra_info=f"Chat: {chat_id}, Cleared: {count} messages"
                )
            else:
                logger.debug(f"Chat not in cache: {chat_id}")
        except Exception as e:
            logger.warning(
                "Failed to clear chat cache",
                extra_info=f"Chat: {chat_id}, Error: {str(e)}"
            )

    def clear_all(self):
        """Clear all cache"""
        try:
            count = sum(len(msgs) for msgs in self.messages.values())
            chat_count = len(self.messages)
            self.messages.clear()
            logger.info(
                "Cleared entire cache",
                extra_info=f"Chats: {chat_count}, Messages: {count}"
            )
        except Exception as e:
            logger.warning(
                "Failed to clear cache",
                error_detail=str(e)
            )
