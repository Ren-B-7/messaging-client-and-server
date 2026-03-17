#!/usr/bin/env python3
"""
Chat Client Advanced - Main Entry Point
Modular version with separate components
"""

import tkinter as tk
import os
import sys

from logger import Logger
from app import ChatClientApp


def main():
    """Main entry point"""
    # Initialize logger
    dev_mode = os.getenv("CHAT_DEV_MODE", "false").lower() == "true"
    logger = Logger(dev_mode=dev_mode)
    
    logger.separator("APPLICATION STARTUP")
    logger.info("Chat Client Advanced starting")
    logger.info(f"Running in {'DEV' if dev_mode else 'STANDARD'} mode")
    
    try:
        # Create root window
        logger.debug("Creating Tkinter root window")
        root = tk.Tk()
        logger.info("Root window created successfully")
        
        # Initialize the application
        logger.info("Initializing ChatClientApp")
        app = ChatClientApp(root)
        logger.info("ChatClientApp initialized successfully")
        
        # Start the main event loop
        logger.info("Starting main event loop")
        logger.separator("APPLICATION READY")
        root.mainloop()
        
        # Application closed
        logger.info("Main event loop terminated")
        logger.separator("APPLICATION SHUTDOWN")
        logger.info("Chat Client Advanced closed successfully")
        
    except Exception as e:
        logger.exception(
            "Critical error in main",
            error_detail=str(e),
            stop=True
        )
        sys.exit(1)


if __name__ == "__main__":
    main()
