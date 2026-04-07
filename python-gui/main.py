#!/usr/bin/env python3
"""
Chat Client Advanced - Main Entry Point

Shutdown order
--------------
1. Tkinter mainloop exits (window closed or WM_DELETE_WINDOW).
2. _on_close() is called:
   a. Tells the API client to set its _shutdown Event so SSE threads exit.
   b. Gives threads up to SHUTDOWN_TIMEOUT seconds to finish.
   c. Destroys the Tk root to release OS resources.
3. main() logs the clean exit and returns.

Signal handling (SIGINT / SIGTERM)
-----------------------------------
A signal handler posted to the Tk event loop via root.after() ensures
Ctrl-C in the terminal triggers the same clean-close path as clicking ×.

CLI Arguments
-------------
  --clean    Remove all cached files, logs, and config, then exit.
             Removes:
               ~/.cache/chat-client/        (favicon cache)
               ~/.local/share/chat-client/  (log files)
               ~/.config/chat-client/       (theme config)
"""

import tkinter as tk
import os
import sys
import signal
import shutil
import threading

from logger import Logger
from app import ChatClientApp

# Seconds to wait for daemon threads (SSE) to stop before forcing exit.
SHUTDOWN_TIMEOUT = 3


def _clean_user_data():
    """Delete all cached files, logs, and config written by chat-client."""
    xdg_cache = os.environ.get("XDG_CACHE_HOME", os.path.expanduser("~/.cache"))
    xdg_data = os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
    xdg_config = os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config"))

    targets = [
        (os.path.join(xdg_cache, "chat-client"), "cache"),
        (os.path.join(xdg_data, "chat-client"), "logs"),
        (os.path.join(xdg_config, "chat-client"), "config"),
    ]

    cleaned = []
    errors = []
    for path, kind in targets:
        if os.path.exists(path):
            try:
                shutil.rmtree(path)
                cleaned.append(f"  ✓ {kind}: {path}")
            except Exception as e:
                errors.append(f"  ✗ {kind}: {path} — {e}")
        else:
            cleaned.append(f"  - {kind}: {path} (not found, skipped)")

    print("Chat Client — Clean")
    print("=" * 50)
    for line in cleaned:
        print(line)
    for line in errors:
        print(line)
    if errors:
        print("\nCompleted with errors.")
        sys.exit(1)
    else:
        print("\nDone. All data removed.")


def main():
    # ── CLI argument handling ─────────────────────────────────────────────────
    if "--clean" in sys.argv:
        _clean_user_data()
        return

    dev_mode = os.getenv("CHAT_DEV_MODE", "false").lower() == "true"
    logger = Logger(dev_mode=dev_mode)

    logger.separator("APPLICATION STARTUP")
    logger.info("Chat Client Advanced starting")
    logger.info(f"Mode: {'DEV' if dev_mode else 'STANDARD'}")

    try:
        root = tk.Tk()
        app = ChatClientApp(root)

        # ── Clean shutdown helper ─────────────────────────────────────────────
        _closing = threading.Event()

        def _on_close():
            """Orchestrate a clean shutdown from any calling context."""
            if _closing.is_set():
                return  # guard against double-call
            _closing.set()

            logger.separator("APPLICATION SHUTDOWN")
            logger.info("Shutdown initiated")

            # 1. Stop SSE threads
            try:
                app.api.shutdown()
                logger.debug("API client shutdown requested")
            except Exception as e:
                logger.warning("Error during API shutdown", extra_info=str(e))

            threads = [
                t
                for t in threading.enumerate()
                if not (t.daemon or t is threading.main_thread())
            ]

            for t in threads:
                t.join(timeout=SHUTDOWN_TIMEOUT)

            # 3. Destroy the Tk window
            try:
                root.destroy()
            except Exception as e:
                logger.warning("Error during API shutdown", extra_info=str(e))

            logger.info("Chat Client Advanced closed cleanly")

        # ── Register close handler ────────────────────────────────────────────
        root.protocol("WM_DELETE_WINDOW", _on_close)

        # ── Signal handler (SIGINT / SIGTERM → clean close) ───────────────────
        def _signal_handler(sig, _frame):
            sig_name = signal.Signals(sig).name
            logger.info(f"Signal received: {sig_name}")
            # Schedule _on_close on the Tk main thread (thread-safe).
            try:
                root.after(0, _on_close)
            except RuntimeError:
                _on_close()

        def _poll_signals():
            root.after(100, _poll_signals)

        _poll_signals()

        for sig in (signal.SIGINT, signal.SIGTERM):
            try:
                signal.signal(sig, _signal_handler)
            except (OSError, ValueError):
                # Signal registration may fail on some platforms (e.g. non-main thread).
                pass

        logger.separator("APPLICATION READY")
        root.mainloop()

        # mainloop() returns after root.destroy()
        logger.info("Main event loop terminated")

    except Exception as e:
        logger.exception("Critical error in main", error_detail=str(e), stop=True)
        sys.exit(1)


if __name__ == "__main__":
    main()
