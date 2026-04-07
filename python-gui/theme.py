"""
Theme Manager
Light/dark theme management with persistence
"""

import os
import json

from config import COLORS
from logger import logger


class ThemeManager:
    """Enhanced theme manager with widget color synchronization"""

    def __init__(self, root):
        self.root = root
        self.theme = "light"
        self.colors = COLORS[self.theme]
        self.callbacks = []

        logger.info("ThemeManager initialized")
        self.load_theme()

    def _config_path(self):
        """Return the path to the theme config file (XDG-compliant)."""
        xdg_config = os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config"))
        config_dir = os.path.join(xdg_config, "chat-client")
        os.makedirs(config_dir, exist_ok=True)
        return os.path.join(config_dir, "config.json")

    def load_theme(self):
        """Load saved theme from config"""
        logger.debug("Loading theme preference from config file")

        config_file = self._config_path()

        if os.path.exists(config_file):
            logger.debug(f"Config file found: {config_file}")
            try:
                with open(config_file, "r") as f:
                    config = json.load(f)
                    self.theme = config.get("theme", "light")
                    self.colors = COLORS[self.theme]
                    logger.info(
                        "Theme loaded from config", extra_info=f"Theme: {self.theme}"
                    )
            except Exception as e:
                logger.warning(
                    "Failed to load theme from config",
                    extra_info=f"Using default (light). Error: {str(e)}",
                )
                self.theme = "light"
                self.colors = COLORS[self.theme]
        else:
            logger.debug(f"Config file not found: {config_file}")
            logger.info("Using default theme: light")

    def save_theme(self):
        """Save theme preference"""
        logger.debug("Saving theme preference to config file")

        config_file = self._config_path()
        config = {"theme": self.theme}

        try:
            with open(config_file, "w") as f:
                json.dump(config, f)
            logger.info(
                "Theme saved to config",
                extra_info=f"Theme: {self.theme}, File: {config_file}",
            )
        except Exception as e:
            logger.warning(
                "Failed to save theme to config",
                extra_info=f"Theme: {self.theme}, Error: {str(e)}",
            )

    def toggle(self):
        """Toggle between light and dark"""
        old_theme = self.theme
        self.theme = "dark" if self.theme == "light" else "light"
        self.colors = COLORS[self.theme]

        logger.info("Theme toggled", extra_info=f"{old_theme} → {self.theme}")

        self.save_theme()

        # Notify callbacks
        logger.debug(f"Notifying {len(self.callbacks)} theme change callbacks")
        for callback in self.callbacks:
            try:
                callback()
            except Exception as e:
                logger.warning(
                    "Theme callback failed",
                    extra_info=f"Callback: {callback.__name__}, Error: {str(e)}",
                )

    def subscribe(self, callback):
        """Subscribe to theme changes"""
        try:
            self.callbacks.append(callback)
            logger.debug(
                "Theme change callback registered",
                extra_info=f"Callback: {callback.__name__}, Total: {len(self.callbacks)}",
            )
        except Exception as e:
            logger.warning(
                "Failed to register theme callback", extra_info=f"Error: {str(e)}"
            )

    def apply(self):
        """Apply theme to root window"""
        try:
            self.root.configure(bg=self.colors["bg_primary"])
            logger.debug(f"Theme applied to root window: {self.theme}")
        except Exception as e:
            logger.warning(
                "Failed to apply theme to root window",
                extra_info=f"Theme: {self.theme}, Error: {str(e)}",
            )
