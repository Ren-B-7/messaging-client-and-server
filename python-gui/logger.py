"""
Chat Client Logging System
Provides comprehensive logging with DEV and STANDARD modes

Usage:
    from logger import Logger, LOG_INFO, LOG_WARNING, LOG_EXCEPTION

    logger = Logger(dev_mode=True)  # or False for standard mode

    logger.info("Login successful", func_name="login")
    logger.warning("Connection slow", func_name="connect")
    logger.exception("Connection failed", "Critical error", func_name="connect")
"""

import os
from datetime import datetime
from functools import wraps
import traceback
import inspect

# ============================================================================
# Log Level Constants
# ============================================================================

LOG_INFO = "INFO"
LOG_WARNING = "WARNING"
LOG_EXCEPTION = "EXCEPTION"
LOG_DEBUG = "DEBUG"

# ============================================================================
# Color Codes for Terminal Output
# ============================================================================

COLORS = {
    "RESET": "\033[0m",
    "BOLD": "\033[1m",
    "DIM": "\033[2m",
    "CYAN": "\033[36m",
    "YELLOW": "\033[33m",
    "RED": "\033[31m",
    "GREEN": "\033[32m",
    "BLUE": "\033[34m",
    "MAGENTA": "\033[35m",
    "WHITE": "\033[37m",
}

# ============================================================================
# Logger Class
# ============================================================================


class Logger:
    """
    Comprehensive logging system for chat client

    Modes:
        - DEV: Prints all logs (INFO, WARNING, EXCEPTION)
        - STANDARD: Prints only errors (WARNING, EXCEPTION)
    """

    def __init__(self, dev_mode=False, log_file=None):
        """
        Initialize logger

        Args:
            dev_mode (bool): If True, print all logs. If False, print only errors.
            log_file (str): Optional path to log file. If None, uses default.
        """
        self.dev_mode = dev_mode
        self.start_time = datetime.now()

        # Setup log file
        if log_file is None:
            xdg_data = os.environ.get(
                "XDG_DATA_HOME", os.path.expanduser("~/.local/share")
            )
            log_dir = os.path.join(xdg_data, "chat-client", "logs")
            os.makedirs(log_dir, exist_ok=True)
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            self.log_file = os.path.join(log_dir, f"chat_{timestamp}.log")
        else:
            self.log_file = log_file

        # Write initial log
        self._write_log_file("=" * 80)
        self._write_log_file(f"Chat Client Logger Started - {datetime.now()}")
        self._write_log_file(f"Mode: {'DEV' if dev_mode else 'STANDARD'}")
        self._write_log_file("=" * 80)

    def _get_timestamp(self):
        """Get current timestamp"""
        return datetime.now().strftime("%Y-%m-%d %H:%M:%S.%f")[:-3]

    def _get_caller_info(self):
        """Get information about the caller"""
        frame = inspect.currentframe().f_back.f_back.f_back
        func_name = frame.f_code.co_name
        line_no = frame.f_lineno
        file_name = os.path.basename(frame.f_code.co_filename)
        return file_name, func_name, line_no

    def _format_message(self, level, message, extra_info=""):
        """Format log message"""
        timestamp = self._get_timestamp()
        file_name, func_name, line_no = self._get_caller_info()

        # Base message
        formatted = f"[{timestamp}] [{level}] {message}"

        # Add function info
        formatted += f" ({file_name}:{func_name}:{line_no})"

        # Add extra info if provided
        if extra_info:
            formatted += f"\n  └─ {extra_info}"

        return formatted

    def _write_log_file(self, message):
        """Write message to log file"""
        try:
            with open(self.log_file, "a", encoding="utf-8") as f:
                f.write(message + "\n")
        except Exception:
            pass  # Silently fail if can't write to file

    def _print_with_color(self, level, formatted_message):
        """Print message with color coding"""
        color_map = {
            LOG_INFO: COLORS["CYAN"],
            LOG_WARNING: COLORS["YELLOW"],
            LOG_EXCEPTION: COLORS["RED"],
            LOG_DEBUG: COLORS["BLUE"],
        }

        color = color_map.get(level, COLORS["WHITE"])
        reset = COLORS["RESET"]

        print(f"{color}{formatted_message}{reset}")

    def info(self, message, extra_info="", func_name=None):
        """
        Log info message (only shown in DEV mode)

        Args:
            message (str): Main message
            extra_info (str): Additional information
            func_name (str): Optional function name for context
        """
        if not self.dev_mode:
            return  # Don't log in STANDARD mode

        formatted = self._format_message(LOG_INFO, message, extra_info)
        self._print_with_color(LOG_INFO, formatted)
        self._write_log_file(formatted)

    def warning(self, message, extra_info="", func_name=None):
        """
        Log warning message (shown in both modes)

        Args:
            message (str): Main message
            extra_info (str): Additional information
            func_name (str): Optional function name for context
        """
        if not extra_info:
            extra_info = ""
        formatted = self._format_message(LOG_WARNING, message, extra_info)
        self._print_with_color(LOG_WARNING, formatted)
        self._write_log_file(formatted)

    def exception(self, message, error_detail="", func_name=None, stop=False):
        """
        Log exception message (shown in both modes)

        Args:
            message (str): Main message
            error_detail (str): Error details/traceback
            func_name (str): Optional function name for context
            stop (bool): If True, prints "SYSTEM CRITICAL - STOPPING" and returns True

        Returns:
            bool: True if stop=True, False otherwise
        """
        # Add traceback if available
        if not error_detail:
            error_detail = traceback.format_exc()

        formatted = self._format_message(LOG_EXCEPTION, message, error_detail)

        if stop:
            formatted += "\n  └─ SYSTEM CRITICAL - STOPPING APPLICATION"

        self._print_with_color(LOG_EXCEPTION, formatted)
        self._write_log_file(formatted)

        return stop

    def debug(self, message, extra_info="", func_name=None):
        """
        Log debug message (only shown in DEV mode)

        Args:
            message (str): Main message
            extra_info (str): Additional information
            func_name (str): Optional function name for context
        """
        if not self.dev_mode:
            return  # Don't log in STANDARD mode

        formatted = self._format_message(LOG_DEBUG, message, extra_info)
        self._print_with_color(LOG_DEBUG, formatted)
        self._write_log_file(formatted)

    def log(self, level, message, extra_info=""):
        """
        Log message with specified level

        Args:
            level (str): One of LOG_INFO, LOG_WARNING, LOG_EXCEPTION, LOG_DEBUG
            message (str): Main message
            extra_info (str): Additional information
        """
        if level == LOG_INFO:
            self.info(message, extra_info)
        elif level == LOG_WARNING:
            self.warning(message, extra_info)
        elif level == LOG_EXCEPTION:
            self.exception(message, extra_info)
        elif level == LOG_DEBUG:
            self.debug(message, extra_info)

    def separator(self, title=""):
        """Print a separator line"""
        sep = "=" * 80
        if title:
            sep = f"{'=' * 10} {title} {'=' * (60 - len(title))}"

        print(f"{COLORS['DIM']}{sep}{COLORS['RESET']}")
        self._write_log_file(sep)

    def get_log_file_path(self):
        """Get path to current log file"""
        return self.log_file


# ============================================================================
# Function Decorator (Optional - Auto-Logging)
# ============================================================================


def log_function(logger, level=LOG_INFO):
    """
    Decorator to automatically log function calls

    Usage:
        @log_function(logger, level=LOG_DEBUG)
        def my_function(x, y):
            return x + y
    """

    def decorator(func):
        @wraps(func)
        def wrapper(*args, **kwargs):
            func_name = func.__name__
            args_str = ", ".join(
                [str(arg)[:50] for arg in args[1:]]  # Skip 'self'
            )  # Limit arg length
            kwargs_str = ", ".join([f"{k}={str(v)[:50]}" for k, v in kwargs.items()])

            all_args = ", ".join([s for s in [args_str, kwargs_str] if s])
            msg = f"Calling {func_name}({all_args})"

            logger.log(level, msg, func_name=func_name)

            try:
                result = func(*args, **kwargs)
                logger.log(
                    level,
                    f"{func_name} completed successfully",
                    func_name=func_name,
                )
                return result
            except Exception as e:
                logger.exception(
                    f"Error in {func_name}",
                    str(e),
                    func_name=func_name,
                    stop=False,
                )
                raise

        return wrapper

    return decorator


# ============================================================================
# Global Logger Instance
# ============================================================================

# Initialize with DEV_MODE (set to False for production)
DEV_MODE = os.getenv("CHAT_DEV_MODE", "false").lower() == "true"

# Create global logger instance
logger = Logger(dev_mode=DEV_MODE)

# ============================================================================
# Convenience Functions (Optional - Use if you prefer functions over class)
# ============================================================================


def log_info(message, extra_info=""):
    """Log info message"""
    logger.info(message, extra_info)


def log_warning(message, extra_info=""):
    """Log warning message"""
    logger.warning(message, extra_info)


def log_exception(message, error_detail="", stop=False):
    """Log exception message"""
    return logger.exception(message, error_detail, stop=stop)


def log_debug(message, extra_info=""):
    """Log debug message"""
    logger.debug(message, extra_info)


# ============================================================================
# Examples / Testing
# ============================================================================

if __name__ == "__main__":
    print("Testing Logger Module\n")

    # Test with DEV mode
    test_logger = Logger(dev_mode=True)

    print("\n" + "=" * 80)
    print("DEV MODE TESTS (should show all messages)")
    print("=" * 80 + "\n")

    test_logger.info("Application started", extra_info="Loading configuration...")
    test_logger.debug(
        "Debug message",
        extra_info="This shows additional details for debugging",
    )
    test_logger.warning("Connection timeout", extra_info="Retrying in 5 seconds")
    test_logger.exception(
        "Connection failed",
        error_detail="[Errno 61] Connection refused",
        stop=False,
    )

    # Test with STANDARD mode
    print("\n" + "=" * 80)
    print("STANDARD MODE TESTS (should only show warnings & exceptions)")
    print("=" * 80 + "\n")

    test_logger2 = Logger(dev_mode=False)

    test_logger2.info("This won't show (DEV mode disabled)")
    test_logger2.warning("This warning WILL show")
    test_logger2.exception("This exception WILL show", "Error details here")

    print(f"\nLog file created at: {test_logger.get_log_file_path()}")
