"""
Chat Client App - WITH COMPREHENSIVE LOGGING
Main application class with all UI components
Logging integrated throughout - replaces all print statements
"""

import tkinter as tk
from tkinter import ttk, messagebox
import json
import subprocess
import os
import threading
from collections import defaultdict
from datetime import datetime

import config
from api import ChatAPIClient
from theme import ThemeManager
from logger import Logger


class ChatClientApp:
    """Enhanced chat application with advanced features"""

    def __init__(self, root):
        # Initialize logger FIRST
        dev_mode = os.getenv("CHAT_DEV_MODE", "false").lower() == "true"
        self.logger = Logger(dev_mode=dev_mode)
        
        self.logger.separator("CHAT CLIENT INITIALIZATION")
        self.logger.info("Initializing Chat Client Application")
        
        self.root = root
        self.root.title("Chat Client - Advanced")
        self.root.geometry("1200x800")
        self.root.minsize(900, 600)
        
        self.logger.debug("Window created: 1200x800, Min: 900x600")

        # API Client
        try:
            self.api = ChatAPIClient(config.DEFAULT_SERVER)
            self.logger.info(
                "API client initialized",
                extra_info=f"Server: {config.DEFAULT_SERVER}"
            )
            
            # Log configuration (debug only)
            if dev_mode:
                self.logger.debug("Application Configuration:")
                self.logger.debug(f"  CONNECTION_TIMEOUT: {config.CONNECTION_TIMEOUT}s")
                self.logger.debug(f"  DEFAULT_SERVER: {config.DEFAULT_SERVER}")
                self.logger.debug(f"  MESSAGE_CACHE_LIMIT: {config.MESSAGE_CACHE_LIMIT}")
                self.logger.debug(f"  RECONNECT_TIMEOUT: {config.RECONNECT_TIMEOUT}s")
                self.logger.debug(f"  USER_AGENT: {config.USER_AGENT}")
        except Exception as e:
            self.logger.exception(
                "Failed to initialize API client",
                error_detail=str(e),
                stop=True
            )
            raise
        
        # State
        self.current_user = None
        self.is_admin = False
        self.current_chat_id = None
        self.stream_thread = None
        self.typing_users = set()
        self.unread_counts = defaultdict(int)
        
        self.logger.debug("Application state initialized")

        # Theme
        try:
            self.theme = ThemeManager(root)
            self.theme.subscribe(self.rebuild_ui)
            self.logger.info(f"Theme manager initialized (default: {self.theme.theme})")
        except Exception as e:
            self.logger.exception(
                "Failed to initialize theme manager",
                error_detail=str(e),
                stop=False
            )

        # Load favicon
        try:
            self.logger.debug("Loading favicon")
            self.load_favicon()
        except Exception as e:
            self.logger.warning("Favicon loading failed", str(e))

        # Build UI
        try:
            self.logger.info("Building UI")
            self.build_ui()
            self.logger.debug("UI built successfully")
            
            self.logger.info("Displaying login screen")
            self.show_login_screen()
            self.logger.debug("Login screen displayed")
        except Exception as e:
            self.logger.exception(
                "Failed to build UI",
                error_detail=str(e),
                stop=True
            )
            raise

        self.logger.separator("CHAT CLIENT READY")
        self.logger.info("Application initialization complete")

    def get_color(self, name):
        """Get color from current theme"""
        return self.theme.colors.get(name, "#000000")

    def load_favicon(self):
        """Load favicon from server or use default"""
        self.logger.debug("Starting favicon load sequence")
        
        favicon_path = os.path.expanduser("~/.chat_client_favicon.png")
        favicon_url = f"{self.api.server_url}/static/icons/favicons/favicon-32x32.png"

        # Try to load from cache first
        if os.path.exists(favicon_path):
            self.logger.info("Favicon found in cache")
            try:
                icon = tk.PhotoImage(file=favicon_path)
                self.root.iconphoto(False, icon)
                self.logger.info("Favicon loaded from cache successfully")
                return
            except Exception as e:
                self.logger.warning("Failed to load cached favicon", str(e))
                # Fall through to download

        # Try to fetch from server via API
        self.logger.debug("Attempting favicon download via API")
        try:
            response = self.api._make_request(
                "GET", "/static/icons/favicons/favicon-32x32.png"
            )
            if response.get("success") or response.get("status") == 200:
                # Save favicon data
                data = response.get("data")
                if data and isinstance(data, str):
                    # It's base64 encoded, decode it
                    try:
                        import base64

                        favicon_data = base64.b64decode(data)
                        with open(favicon_path, "wb") as f:
                            f.write(favicon_data)
                        icon = tk.PhotoImage(file=favicon_path)
                        self.root.iconphoto(False, icon)
                        self.logger.info("Favicon downloaded from API successfully")
                        return
                    except Exception as e:
                        self.logger.warning("Failed to decode API favicon", str(e))
                        # Fall through to curl
        except Exception as e:
            self.logger.warning("API favicon download failed", str(e))
            # Fall through to curl

        # Fallback: Try curl
        self.logger.debug("Attempting favicon download via curl")
        try:
            result = subprocess.run(
                ["curl", "-s", "-o", favicon_path, favicon_url],
                capture_output=True,
                timeout=3,
            )
            if result.returncode == 0 and os.path.exists(favicon_path):
                try:
                    icon = tk.PhotoImage(file=favicon_path)
                    self.root.iconphoto(False, icon)
                    self.logger.info("Favicon downloaded via curl successfully")
                    return
                except Exception as e:
                    self.logger.warning("Failed to set favicon from curl", str(e))
        except Exception as e:
            self.logger.warning("Curl favicon download failed", str(e))

        # If all else fails, set a colored icon (fallback)
        self.logger.debug("Using colored fallback favicon")
        try:
            # Create a simple colored square as icon
            icon = tk.PhotoImage(width=32, height=32)
            accent_color = self.get_color("accent")
            # Fill with accent color
            for i in range(32):
                for j in range(32):
                    icon.put(accent_color, (i, j))
            self.root.iconphoto(False, icon)
            self.logger.info("Colored fallback favicon applied")
        except Exception as e:
            self.logger.warning("Failed to create fallback favicon", str(e))

    def build_ui(self):
        """Build main UI structure"""
        self.logger.debug("Building main UI structure")
        
        # Clear
        for widget in self.root.winfo_children():
            widget.destroy()

        # Main container
        self.main_container = tk.Frame(self.root, bg=self.get_color("bg_primary"))
        self.main_container.pack(fill=tk.BOTH, expand=True)
        self.logger.debug("Main container created")

        # Configure style
        style = ttk.Style()
        style.theme_use("clam")
        style.configure(
            "TNotebook",
            background=self.get_color("bg_secondary"),
            foreground=self.get_color("fg_primary"),
        )
        self.logger.debug("Ttk style configured")

        # Notebook
        self.notebook = ttk.Notebook(self.main_container)
        self.notebook.pack(fill=tk.BOTH, expand=True, padx=1, pady=1)
        self.logger.debug("Notebook created")

        # Tab frames (use tk.Frame instead of ttk.Frame for proper bg styling)
        self.login_frame = tk.Frame(self.notebook, bg=self.get_color("bg_primary"))
        self.chat_frame = tk.Frame(self.notebook, bg=self.get_color("bg_primary"))
        self.settings_frame = tk.Frame(self.notebook, bg=self.get_color("bg_primary"))
        self.admin_frame = tk.Frame(self.notebook, bg=self.get_color("bg_primary"))

        self.notebook.add(self.login_frame, text="Login")
        self.notebook.add(self.chat_frame, text="Chat", state="disabled")
        self.notebook.add(self.settings_frame, text="Settings", state="disabled")
        self.notebook.add(self.admin_frame, text="Admin", state="disabled")

        # Bind tab changes
        self.notebook.bind("<<NotebookTabChanged>>", self.on_tab_changed)
        
        self.logger.debug("All tabs created")

    def on_tab_changed(self, event):
        """Handle tab change"""
        try:
            selected = self.notebook.index("current")
            self.logger.debug(f"Tab changed to index: {selected}")
            
            if selected == 1:  # Chat
                self.show_chat_screen()
            elif selected == 2:  # Settings
                self.show_settings_screen()
            elif selected == 3:  # Admin
                self.show_admin_screen()
        except Exception as e:
            self.logger.exception(
                "Error handling tab change",
                error_detail=str(e),
                stop=False
            )

    def show_login_screen(self):
        """Show login/register screen"""
        self.logger.info("Displaying login screen")
        
        try:
            # Clear
            for widget in self.login_frame.winfo_children():
                widget.destroy()
            
            self.logger.debug("Login frame cleared")

            # Background
            self.login_frame.configure(bg=self.get_color("bg_secondary"))

            # Scroll container
            canvas = tk.Canvas(
                self.login_frame, bg=self.get_color("bg_secondary"), highlightthickness=0
            )
            scrollbar = ttk.Scrollbar(
                self.login_frame, orient="vertical", command=canvas.yview
            )
            scrollable_frame = tk.Frame(canvas, bg=self.get_color("bg_secondary"))

            scrollable_frame.bind(
                "<Configure>", lambda e: canvas.configure(scrollregion=canvas.bbox("all"))
            )

            canvas.create_window((0, 0), window=scrollable_frame, anchor="nw")
            canvas.configure(yscrollcommand=scrollbar.set)

            canvas.pack(side="left", fill="both", expand=True)
            scrollbar.pack(side="right", fill="y")

            # Content container
            container = tk.Frame(scrollable_frame, bg=self.get_color("bg_secondary"))
            container.pack(expand=True, padx=40, pady=60)

            # Logo
            logo = tk.Label(
                container,
                text="💬",
                font=("Helvetica", 60),
                bg=self.get_color("bg_secondary"),
                fg=self.get_color("accent"),
            )
            logo.pack(pady=20)

            # Title
            title = tk.Label(
                container,
                text="Chat App",
                font=("Helvetica", 32, "bold"),
                bg=self.get_color("bg_secondary"),
                fg=self.get_color("fg_primary"),
            )
            title.pack(pady=10)

            # Subtitle
            subtitle = tk.Label(
                container,
                text="Connect & communicate",
                font=("Helvetica", 14),
                bg=self.get_color("bg_secondary"),
                fg=self.get_color("fg_secondary"),
            )
            subtitle.pack(pady=5)

            # Form frame
            form_frame = tk.Frame(container, bg=self.get_color("bg_secondary"))
            form_frame.pack(pady=30, fill=tk.X, padx=20)

            # Server
            tk.Label(
                form_frame,
                text="Server URL",
                bg=self.get_color("bg_secondary"),
                fg=self.get_color("fg_primary"),
                font=("Helvetica", 10),
            ).pack(anchor=tk.W, pady=(0, 5))

            server_var = tk.StringVar(value=config.DEFAULT_SERVER)
            server_entry = tk.Entry(
                form_frame,
                textvariable=server_var,
                bg=self.get_color("bg_primary"),
                fg=self.get_color("fg_primary"),
                relief=tk.FLAT,
                border=1,
                font=("Helvetica", 10),
                width=40,
            )
            server_entry.pack(fill=tk.X, pady=(0, 15))

            # Username
            tk.Label(
                form_frame,
                text="Username",
                bg=self.get_color("bg_secondary"),
                fg=self.get_color("fg_primary"),
                font=("Helvetica", 10),
            ).pack(anchor=tk.W, pady=(0, 5))

            self.username_input = tk.Entry(
                form_frame,
                bg=self.get_color("bg_primary"),
                fg=self.get_color("fg_primary"),
                relief=tk.FLAT,
                border=1,
                font=("Helvetica", 10),
                width=40,
            )
            self.username_input.pack(fill=tk.X, pady=(0, 15))
            self.username_input.bind("<Return>", lambda e: self.handle_login())

            # Password
            tk.Label(
                form_frame,
                text="Password",
                bg=self.get_color("bg_secondary"),
                fg=self.get_color("fg_primary"),
                font=("Helvetica", 10),
            ).pack(anchor=tk.W, pady=(0, 5))

            self.password_input = tk.Entry(
                form_frame,
                bg=self.get_color("bg_primary"),
                fg=self.get_color("fg_primary"),
                relief=tk.FLAT,
                border=1,
                font=("Helvetica", 10),
                show="•",
                width=40,
            )
            self.password_input.pack(fill=tk.X, pady=(0, 20))
            self.password_input.bind("<Return>", lambda e: self.handle_login())

            # Buttons
            button_frame = tk.Frame(form_frame, bg=self.get_color("bg_secondary"))
            button_frame.pack(fill=tk.X, pady=10)

            login_btn = tk.Button(
                button_frame,
                text="Sign In",
                command=self.handle_login,
                bg=self.get_color("accent"),
                fg="#ffffff",
                relief=tk.FLAT,
                padx=20,
                pady=10,
                font=("Helvetica", 11, "bold"),
                activebackground=self.get_color("accent_hover"),
                cursor="hand2",
            )
            login_btn.pack(side=tk.LEFT, padx=5)

            register_btn = tk.Button(
                button_frame,
                text="Create Account",
                command=self.show_register_screen,
                bg=self.get_color("bg_primary"),
                fg=self.get_color("accent"),
                relief=tk.FLAT,
                padx=20,
                pady=10,
                font=("Helvetica", 11, "bold"),
                activebackground=self.get_color("bg_tertiary"),
                cursor="hand2",
            )
            register_btn.pack(side=tk.LEFT, padx=5)
            
            self.logger.info("Login screen rendered successfully")
        except Exception as e:
            self.logger.exception(
                "Failed to render login screen",
                error_detail=str(e),
                stop=False
            )

    def show_register_screen(self):
        """Show register screen"""
        self.logger.info("Displaying register screen")
        
        try:
            # Clear
            for widget in self.login_frame.winfo_children():
                widget.destroy()

            self.logger.debug("Register frame cleared")

            self.login_frame.configure(bg=self.get_color("bg_secondary"))

            # Scroll container
            canvas = tk.Canvas(
                self.login_frame, bg=self.get_color("bg_secondary"), highlightthickness=0
            )
            scrollbar = ttk.Scrollbar(
                self.login_frame, orient="vertical", command=canvas.yview
            )
            scrollable_frame = tk.Frame(canvas, bg=self.get_color("bg_secondary"))

            scrollable_frame.bind(
                "<Configure>", lambda e: canvas.configure(scrollregion=canvas.bbox("all"))
            )

            canvas.create_window((0, 0), window=scrollable_frame, anchor="nw")
            canvas.configure(yscrollcommand=scrollbar.set)

            canvas.pack(side="left", fill="both", expand=True)
            scrollbar.pack(side="right", fill="y")

            # Content container
            container = tk.Frame(scrollable_frame, bg=self.get_color("bg_secondary"))
            container.pack(expand=True, padx=40, pady=30)

            # Title
            title = tk.Label(
                container,
                text="Create Account",
                font=("Helvetica", 28, "bold"),
                bg=self.get_color("bg_secondary"),
                fg=self.get_color("fg_primary"),
            )
            title.pack(pady=10)

            # Form
            form_frame = tk.Frame(container, bg=self.get_color("bg_secondary"))
            form_frame.pack(pady=20, fill=tk.X, padx=20)

            # Email
            tk.Label(
                form_frame,
                text="Email",
                bg=self.get_color("bg_secondary"),
                fg=self.get_color("fg_primary"),
                font=("Helvetica", 10),
            ).pack(anchor=tk.W, pady=(0, 5))

            email_input = tk.Entry(
                form_frame,
                bg=self.get_color("bg_primary"),
                fg=self.get_color("fg_primary"),
                relief=tk.FLAT,
                border=1,
                font=("Helvetica", 10),
                width=40,
            )
            email_input.pack(fill=tk.X, pady=(0, 10))

            # Full Name
            tk.Label(
                form_frame,
                text="Full Name",
                bg=self.get_color("bg_secondary"),
                fg=self.get_color("fg_primary"),
                font=("Helvetica", 10),
            ).pack(anchor=tk.W, pady=(0, 5))

            fullname_input = tk.Entry(
                form_frame,
                bg=self.get_color("bg_primary"),
                fg=self.get_color("fg_primary"),
                relief=tk.FLAT,
                border=1,
                font=("Helvetica", 10),
                width=40,
            )
            fullname_input.pack(fill=tk.X, pady=(0, 10))

            # Username
            tk.Label(
                form_frame,
                text="Username",
                bg=self.get_color("bg_secondary"),
                fg=self.get_color("fg_primary"),
                font=("Helvetica", 10),
            ).pack(anchor=tk.W, pady=(0, 5))

            username_input = tk.Entry(
                form_frame,
                bg=self.get_color("bg_primary"),
                fg=self.get_color("fg_primary"),
                relief=tk.FLAT,
                border=1,
                font=("Helvetica", 10),
                width=40,
            )
            username_input.pack(fill=tk.X, pady=(0, 10))

            # Password
            tk.Label(
                form_frame,
                text="Password",
                bg=self.get_color("bg_secondary"),
                fg=self.get_color("fg_primary"),
                font=("Helvetica", 10),
            ).pack(anchor=tk.W, pady=(0, 5))

            password_input = tk.Entry(
                form_frame,
                bg=self.get_color("bg_primary"),
                fg=self.get_color("fg_primary"),
                relief=tk.FLAT,
                border=1,
                font=("Helvetica", 10),
                show="•",
                width=40,
            )
            password_input.pack(fill=tk.X, pady=(0, 20))

            # Buttons
            button_frame = tk.Frame(form_frame, bg=self.get_color("bg_secondary"))
            button_frame.pack(fill=tk.X, pady=10)

            register_btn = tk.Button(
                button_frame,
                text="Register",
                command=lambda: self.handle_register(
                    email_input.get(), username_input.get(), password_input.get(), fullname_input.get()
                ),
                bg=self.get_color("accent"),
                fg="#ffffff",
                relief=tk.FLAT,
                padx=20,
                pady=10,
                font=("Helvetica", 11, "bold"),
                activebackground=self.get_color("accent_hover"),
                cursor="hand2",
            )
            register_btn.pack(side=tk.LEFT, padx=5)

            back_btn = tk.Button(
                button_frame,
                text="Back",
                command=self.show_login_screen,
                bg=self.get_color("bg_primary"),
                fg=self.get_color("accent"),
                relief=tk.FLAT,
                padx=20,
                pady=10,
                font=("Helvetica", 11, "bold"),
                activebackground=self.get_color("bg_tertiary"),
                cursor="hand2",
            )
            back_btn.pack(side=tk.LEFT, padx=5)
            
            self.logger.info("Register screen rendered successfully")
        except Exception as e:
            self.logger.exception(
                "Failed to render register screen",
                error_detail=str(e),
                stop=False
            )

    def handle_login(self):
        """Handle login"""
        self.logger.info("Login attempt initiated")
        
        username = self.username_input.get()
        password = self.password_input.get()
        
        if not username or not password:
            self.logger.warning("Login attempt with missing credentials")
            messagebox.showerror("Error", "Please enter username and password")
            return

        self.logger.debug(f"Login credentials provided: username={username}")

        def login_thread():
            try:
                self.logger.info(f"Authenticating user: {username}")
                response = self.api.login(username, password)
                self.root.after(0, lambda: self._handle_login_response(response, username))
            except Exception as e:
                self.logger.exception(
                    "Error during login",
                    error_detail=str(e),
                    stop=False
                )
                self.root.after(0, lambda: messagebox.showerror("Error", f"Login failed: {str(e)}"))

        thread = threading.Thread(target=login_thread, daemon=True)
        thread.start()

    def _handle_login_response(self, response, username):
        """Handle login response"""
        try:
            if response.get("success"):
                try:
                    user_data = json.loads(response.get("data", "{}"))
                    self.current_user = user_data
                    self.is_admin = user_data.get("is_admin", False)
                    
                    self.logger.info(
                        f"Login successful for {username}",
                        extra_info=f"User ID: {user_data.get('id')}, Admin: {self.is_admin}"
                    )

                    # Enable tabs
                    self.notebook.tab(1, state="normal")
                    self.notebook.tab(2, state="normal")
                    if self.is_admin:
                        self.notebook.tab(3, state="normal")

                    self.show_chat_screen()
                    self.notebook.select(1)
                except Exception as e:
                    self.logger.exception(
                        "Error parsing login response",
                        error_detail=str(e),
                        stop=False
                    )
                    messagebox.showerror("Error", "Failed to parse login response")
            else:
                error = response.get("data", "Unknown error")
                self.logger.warning(f"Login failed for {username}", extra_info=error)
                messagebox.showerror("Login Failed", error)
        except Exception as e:
            self.logger.exception(
                "Error handling login response",
                error_detail=str(e),
                stop=False
            )

    def handle_register(self, email, username, password, fullname):
        """Handle register"""
        self.logger.info("Register attempt initiated")
        
        if not all([email, username, password, fullname]):
            self.logger.warning("Register attempt with missing fields")
            messagebox.showerror("Error", "All fields are required")
            return

        self.logger.debug(f"Register attempt for: email={email}, username={username}")

        def register_thread():
            try:
                self.logger.info(f"Registering user: {email}")
                response = self.api.register(email, username, password, fullname)
                self.root.after(0, lambda: self._handle_register_response(response, email))
            except Exception as e:
                self.logger.exception(
                    "Error during registration",
                    error_detail=str(e),
                    stop=False
                )
                self.root.after(0, lambda: messagebox.showerror("Error", f"Registration failed: {str(e)}"))

        thread = threading.Thread(target=register_thread, daemon=True)
        thread.start()

    def _handle_register_response(self, response, email):
        """Handle register response"""
        try:
            if response.get("success"):
                self.logger.info(f"Registration successful for {email}")
                messagebox.showinfo("Success", "Account created! Please login.")
                self.show_login_screen()
            else:
                error = response.get("data", "Unknown error")
                self.logger.warning(f"Registration failed for {email}", extra_info=error)
                messagebox.showerror("Registration Failed", error)
        except Exception as e:
            self.logger.exception(
                "Error handling register response",
                error_detail=str(e),
                stop=False
            )

    def show_chat_screen(self):
        """Show chat screen"""
        self.logger.info("Displaying chat screen")
        
        try:
            # Clear
            for widget in self.chat_frame.winfo_children():
                widget.destroy()

            self.logger.debug("Chat frame cleared")

            self.chat_frame.configure(bg=self.get_color("bg_primary"))

            # Main layout
            container = tk.Frame(self.chat_frame, bg=self.get_color("bg_primary"))
            container.pack(fill=tk.BOTH, expand=True)

            # Sidebar
            sidebar = tk.Frame(container, bg=self.get_color("bg_secondary"), width=200)
            sidebar.pack(side=tk.LEFT, fill=tk.Y, padx=(0, 1))
            sidebar.pack_propagate(False)

            # Sidebar title
            sidebar_title = tk.Label(
                sidebar,
                text="Conversations",
                font=("Helvetica", 12, "bold"),
                bg=self.get_color("bg_secondary"),
                fg=self.get_color("fg_primary"),
            )
            sidebar_title.pack(padx=10, pady=10)

            # Load button
            load_btn = tk.Button(
                sidebar,
                text="Reload",
                command=self.load_conversations,
                bg=self.get_color("accent"),
                fg="#ffffff",
                relief=tk.FLAT,
                font=("Helvetica", 10),
                activebackground=self.get_color("accent_hover"),
                cursor="hand2",
            )
            load_btn.pack(fill=tk.X, padx=5, pady=5)

            # New conversation button
            new_btn = tk.Button(
                sidebar,
                text="+ New",
                command=self.new_conversation,
                bg=self.get_color("bg_primary"),
                fg=self.get_color("accent"),
                relief=tk.FLAT,
                font=("Helvetica", 10),
                activebackground=self.get_color("bg_tertiary"),
                cursor="hand2",
            )
            new_btn.pack(fill=tk.X, padx=5, pady=5)

            # Conversations list
            self.conv_frame = tk.Frame(sidebar, bg=self.get_color("bg_secondary"))
            self.conv_frame.pack(fill=tk.BOTH, expand=True, padx=5, pady=5)

            # Main chat area
            chat_area = tk.Frame(container, bg=self.get_color("bg_primary"))
            chat_area.pack(side=tk.RIGHT, fill=tk.BOTH, expand=True)

            # Messages area
            self.messages_frame = tk.Frame(chat_area, bg=self.get_color("bg_primary"))
            self.messages_frame.pack(fill=tk.BOTH, expand=True, padx=10, pady=10)

            # Input area
            input_frame = tk.Frame(chat_area, bg=self.get_color("bg_primary"))
            input_frame.pack(fill=tk.X, padx=10, pady=10)

            self.message_input = tk.Text(
                input_frame,
                height=3,
                bg=self.get_color("bg_secondary"),
                fg=self.get_color("fg_primary"),
                relief=tk.FLAT,
                border=1,
                font=("Helvetica", 10),
                wrap=tk.WORD,
            )
            self.message_input.pack(fill=tk.BOTH, expand=True, side=tk.LEFT, padx=(0, 10))
            self.message_input.bind("<Control-Return>", lambda e: self.send_message())

            send_btn = tk.Button(
                input_frame,
                text="Send\n(Ctrl+Enter)",
                command=self.send_message,
                bg=self.get_color("accent"),
                fg="#ffffff",
                relief=tk.FLAT,
                padx=15,
                font=("Helvetica", 10, "bold"),
                activebackground=self.get_color("accent_hover"),
                cursor="hand2",
            )
            send_btn.pack(side=tk.RIGHT)

            # Load conversations
            self.logger.debug("Loading conversations list")
            self.load_conversations()
            
            self.logger.info("Chat screen rendered successfully")
        except Exception as e:
            self.logger.exception(
                "Failed to render chat screen",
                error_detail=str(e),
                stop=False
            )

    def load_conversations(self):
        """Load conversations"""
        self.logger.info("Loading conversations")

        def load_thread():
            try:
                response = self.api.get_conversations()
                self.root.after(0, lambda: self._display_conversations(response))
            except Exception as e:
                self.logger.exception(
                    "Error loading conversations",
                    error_detail=str(e),
                    stop=False
                )

        thread = threading.Thread(target=load_thread, daemon=True)
        thread.start()

    def _display_conversations(self, response):
        """Display conversations"""
        try:
            # Clear
            for widget in self.conv_frame.winfo_children():
                widget.destroy()

            if not response.get("success"):
                self.logger.warning("Failed to load conversations")
                label = tk.Label(
                    self.conv_frame,
                    text="Failed to load conversations",
                    bg=self.get_color("bg_secondary"),
                    fg=self.get_color("danger"),
                )
                label.pack()
                return

            try:
                conversations = json.loads(response.get("data", "[]"))
                self.logger.info(
                    f"Conversations loaded",
                    extra_info=f"Count: {len(conversations)}"
                )
            except:
                conversations = []

            # Display conversations
            for conv in conversations:
                conv_btn = tk.Button(
                    self.conv_frame,
                    text=f"💬 {conv.get('name', 'Unknown')}",
                    command=lambda c=conv: self.select_conversation(c),
                    bg=self.get_color("bg_primary"),
                    fg=self.get_color("fg_primary"),
                    relief=tk.FLAT,
                    justify=tk.LEFT,
                    anchor=tk.W,
                    padx=10,
                    pady=8,
                    font=("Helvetica", 9),
                    activebackground=self.get_color("bg_tertiary"),
                    cursor="hand2",
                )
                conv_btn.pack(fill=tk.X, padx=0, pady=2)

        except Exception as e:
            self.logger.exception(
                "Error displaying conversations",
                error_detail=str(e),
                stop=False
            )

    def select_conversation(self, conversation):
        """Select a conversation"""
        self.logger.info(
            f"Conversation selected",
            extra_info=f"Conv: {conversation.get('name')}, ID: {conversation.get('id')}"
        )
        
        self.current_chat_id = conversation.get("id")

        def load_thread():
            try:
                response = self.api.get_messages(self.current_chat_id)
                self.root.after(0, lambda: self._display_messages(response))
            except Exception as e:
                self.logger.exception(
                    "Error loading messages",
                    error_detail=str(e),
                    stop=False
                )

        thread = threading.Thread(target=load_thread, daemon=True)
        thread.start()

    def _display_messages(self, response):
        """Display messages"""
        try:
            # Clear
            for widget in self.messages_frame.winfo_children():
                widget.destroy()

            if not response.get("success"):
                self.logger.warning(f"Failed to load messages for chat {self.current_chat_id}")
                label = tk.Label(
                    self.messages_frame,
                    text="Failed to load messages",
                    bg=self.get_color("bg_primary"),
                    fg=self.get_color("danger"),
                )
                label.pack()
                return

            try:
                messages = json.loads(response.get("data", "[]"))
                self.logger.info(
                    f"Messages loaded",
                    extra_info=f"Chat: {self.current_chat_id}, Count: {len(messages)}"
                )
            except:
                messages = []

            # Display messages
            for msg in messages:
                sender = msg.get("sender", "Unknown")
                content = msg.get("content", "")
                is_own = sender == self.current_user.get("username", "")

                frame = tk.Frame(self.messages_frame, bg=self.get_color("bg_primary"))
                frame.pack(fill=tk.X, pady=5, anchor=tk.E if is_own else tk.W)

                label = tk.Label(
                    frame,
                    text=f"{sender}: {content}",
                    bg=self.get_color("bg_secondary"),
                    fg=self.get_color("fg_primary"),
                    wraplength=300,
                    justify=tk.LEFT,
                    padx=10,
                    pady=5,
                    relief=tk.FLAT,
                )
                label.pack()

        except Exception as e:
            self.logger.exception(
                "Error displaying messages",
                error_detail=str(e),
                stop=False
            )

    def send_message(self):
        """Send message"""
        self.logger.debug("Send message initiated")
        
        if not self.current_chat_id:
            self.logger.warning("Attempted to send message without chat selected")
            messagebox.showwarning("Warning", "Please select a conversation first")
            return

        content = self.message_input.get("1.0", tk.END).strip()
        if not content:
            self.logger.debug("Empty message, ignoring")
            return

        self.logger.info(
            "Sending message",
            extra_info=f"Chat: {self.current_chat_id}, Length: {len(content)}"
        )

        def send_thread():
            try:
                response = self.api.send_message(self.current_chat_id, content)
                self.root.after(0, lambda: self._handle_send_response(response))
            except Exception as e:
                self.logger.exception(
                    "Error sending message",
                    error_detail=str(e),
                    stop=False
                )
                self.root.after(0, lambda: messagebox.showerror("Error", "Failed to send message"))

        thread = threading.Thread(target=send_thread, daemon=True)
        thread.start()

    def _handle_send_response(self, response):
        """Handle send response"""
        try:
            if response.get("success"):
                self.logger.info("Message sent successfully")
                self.message_input.delete("1.0", tk.END)
                # Reload messages
                self.select_conversation({"id": self.current_chat_id})
            else:
                error = response.get("data", "Unknown error")
                self.logger.warning("Failed to send message", extra_info=error)
                messagebox.showerror("Error", error)
        except Exception as e:
            self.logger.exception(
                "Error handling send response",
                error_detail=str(e),
                stop=False
            )

    def new_conversation(self):
        """Create new conversation"""
        self.logger.info("Create new conversation initiated")
        messagebox.showinfo("Info", "Feature coming soon!")

    def show_settings_screen(self):
        """Show settings screen"""
        self.logger.info("Displaying settings screen")
        
        try:
            # Clear
            for widget in self.settings_frame.winfo_children():
                widget.destroy()

            self.settings_frame.configure(bg=self.get_color("bg_primary"))

            # Scroll container
            canvas = tk.Canvas(
                self.settings_frame, bg=self.get_color("bg_primary"), highlightthickness=0
            )
            scrollbar = ttk.Scrollbar(
                self.settings_frame, orient="vertical", command=canvas.yview
            )
            scrollable_frame = tk.Frame(canvas, bg=self.get_color("bg_primary"))

            scrollable_frame.bind(
                "<Configure>", lambda e: canvas.configure(scrollregion=canvas.bbox("all"))
            )

            canvas.create_window((0, 0), window=scrollable_frame, anchor="nw")
            canvas.configure(yscrollcommand=scrollbar.set)

            canvas.pack(side="left", fill="both", expand=True)
            scrollbar.pack(side="right", fill="y")

            # Content
            container = tk.Frame(scrollable_frame, bg=self.get_color("bg_primary"))
            container.pack(fill=tk.BOTH, expand=True, padx=20, pady=20)

            # Title
            tk.Label(
                container,
                text="⚙️ Settings",
                font=("Helvetica", 20, "bold"),
                bg=self.get_color("bg_primary"),
                fg=self.get_color("accent"),
            ).pack(anchor=tk.W, pady=10)

            # Theme
            tk.Label(
                container,
                text="Appearance",
                font=("Helvetica", 14, "bold"),
                bg=self.get_color("bg_primary"),
                fg=self.get_color("fg_primary"),
            ).pack(anchor=tk.W, pady=(20, 10))

            theme_btn = tk.Button(
                container,
                text=f"🎨 Current Theme: {self.theme.theme.capitalize()}",
                command=self._toggle_theme,
                bg=self.get_color("accent"),
                fg="#ffffff",
                relief=tk.FLAT,
                padx=15,
                pady=10,
                font=("Helvetica", 11, "bold"),
                activebackground=self.get_color("accent_hover"),
                cursor="hand2",
            )
            theme_btn.pack(anchor=tk.W, pady=5)

            # User info
            tk.Label(
                container,
                text="Account",
                font=("Helvetica", 14, "bold"),
                bg=self.get_color("bg_primary"),
                fg=self.get_color("fg_primary"),
            ).pack(anchor=tk.W, pady=(20, 10))

            if self.current_user:
                info_text = f"""
👤 Name: {self.current_user.get('full_name', 'N/A')}
📧 Email: {self.current_user.get('email', 'N/A')}
🔑 ID: {self.current_user.get('id', 'N/A')}
👑 Admin: {'Yes ✓' if self.is_admin else 'No'}
            """

                tk.Label(
                    container,
                    text=info_text.strip(),
                    bg=self.get_color("bg_secondary"),
                    fg=self.get_color("fg_primary"),
                    justify=tk.LEFT,
                    font=("Helvetica", 10),
                ).pack(anchor=tk.W, padx=10, pady=10)

            # Logout
            tk.Label(
                container,
                text="Session",
                font=("Helvetica", 14, "bold"),
                bg=self.get_color("bg_primary"),
                fg=self.get_color("fg_primary"),
            ).pack(anchor=tk.W, pady=(20, 10))

            logout_btn = tk.Button(
                container,
                text="🚪 Logout",
                command=self.logout,
                bg=self.get_color("danger"),
                fg="#ffffff",
                relief=tk.FLAT,
                padx=20,
                pady=10,
                font=("Helvetica", 12, "bold"),
                activebackground=self.get_color("danger"),
                cursor="hand2",
            )
            logout_btn.pack(anchor=tk.W, pady=20)
            
            self.logger.info("Settings screen rendered successfully")
        except Exception as e:
            self.logger.exception(
                "Failed to render settings screen",
                error_detail=str(e),
                stop=False
            )

    def _toggle_theme(self):
        """Toggle theme"""
        self.logger.info("Theme toggle requested")
        try:
            self.theme.toggle()
            self.logger.info(f"Theme changed to: {self.theme.theme}")
        except Exception as e:
            self.logger.exception(
                "Error toggling theme",
                error_detail=str(e),
                stop=False
            )

    def show_admin_screen(self):
        """Show admin panel"""
        if not self.is_admin:
            self.logger.warning("Non-admin user attempted to access admin panel")
            messagebox.showerror("Error", "Admin access required")
            return

        self.logger.info("Displaying admin screen")
        
        try:
            # Clear
            for widget in self.admin_frame.winfo_children():
                widget.destroy()

            self.admin_frame.configure(bg=self.get_color("bg_primary"))

            # Scroll container
            canvas = tk.Canvas(
                self.admin_frame, bg=self.get_color("bg_primary"), highlightthickness=0
            )
            scrollbar = ttk.Scrollbar(
                self.admin_frame, orient="vertical", command=canvas.yview
            )
            scrollable_frame = tk.Frame(canvas, bg=self.get_color("bg_primary"))

            scrollable_frame.bind(
                "<Configure>", lambda e: canvas.configure(scrollregion=canvas.bbox("all"))
            )

            canvas.create_window((0, 0), window=scrollable_frame, anchor="nw")
            canvas.configure(yscrollcommand=scrollbar.set)

            canvas.pack(side="left", fill="both", expand=True)
            scrollbar.pack(side="right", fill="y")

            # Content
            container = tk.Frame(scrollable_frame, bg=self.get_color("bg_primary"))
            container.pack(fill=tk.BOTH, expand=True, padx=20, pady=20)

            # Title
            tk.Label(
                container,
                text="👑 Admin Panel",
                font=("Helvetica", 20, "bold"),
                bg=self.get_color("bg_primary"),
                fg=self.get_color("accent"),
            ).pack(anchor=tk.W, pady=10)

            # Stats section
            tk.Label(
                container,
                text="Server Statistics",
                font=("Helvetica", 14, "bold"),
                bg=self.get_color("bg_primary"),
                fg=self.get_color("fg_primary"),
            ).pack(anchor=tk.W, pady=(20, 10))

            stats_btn = tk.Button(
                container,
                text="📊 Load Stats",
                command=self.load_admin_stats,
                bg=self.get_color("accent"),
                fg="#ffffff",
                relief=tk.FLAT,
                padx=15,
                pady=10,
                font=("Helvetica", 11, "bold"),
                activebackground=self.get_color("accent_hover"),
                cursor="hand2",
            )
            stats_btn.pack(anchor=tk.W, pady=5)

            self.admin_stats_label = tk.Label(
                container,
                text="",
                bg=self.get_color("bg_primary"),
                fg=self.get_color("fg_primary"),
                justify=tk.LEFT,
            )
            self.admin_stats_label.pack(anchor=tk.W, padx=10, pady=10)

            # Users section
            tk.Label(
                container,
                text="User Management",
                font=("Helvetica", 14, "bold"),
                bg=self.get_color("bg_primary"),
                fg=self.get_color("fg_primary"),
            ).pack(anchor=tk.W, pady=(20, 10))

            users_btn = tk.Button(
                container,
                text="👥 Load Users",
                command=self.load_admin_users,
                bg=self.get_color("accent"),
                fg="#ffffff",
                relief=tk.FLAT,
                padx=15,
                pady=10,
                font=("Helvetica", 11, "bold"),
                activebackground=self.get_color("accent_hover"),
                cursor="hand2",
            )
            users_btn.pack(anchor=tk.W, pady=5)

            self.admin_users_label = tk.Label(
                container,
                text="",
                bg=self.get_color("bg_primary"),
                fg=self.get_color("fg_primary"),
                justify=tk.LEFT,
            )
            self.admin_users_label.pack(anchor=tk.W, padx=10, pady=10)
            
            self.logger.info("Admin screen rendered successfully")
        except Exception as e:
            self.logger.exception(
                "Failed to render admin screen",
                error_detail=str(e),
                stop=False
            )

    def load_admin_stats(self):
        """Load admin stats"""
        self.logger.info("Loading admin statistics")

        def load_thread():
            try:
                response = self.api.get_admin_stats()
                self.root.after(0, lambda: self._display_admin_stats(response))
            except Exception as e:
                self.logger.exception(
                    "Error loading admin stats",
                    error_detail=str(e),
                    stop=False
                )

        thread = threading.Thread(target=load_thread, daemon=True)
        thread.start()

    def _display_admin_stats(self, response):
        """Display admin stats"""
        try:
            if not response.get("success"):
                self.logger.warning("Failed to load admin stats")
                self.admin_stats_label.config(text="⚠️ Failed to load stats")
                return

            try:
                data = json.loads(response["data"])
                self.logger.info("Admin stats loaded successfully")
                
                stats_text = f"""
📈 Server Stats:
  • Total Users: {data.get('total_users', 'N/A')}
  • Active Users: {data.get('active_users', 'N/A')}
  • Total Messages: {data.get('total_messages', 'N/A')}
  • Total Conversations: {data.get('total_conversations', 'N/A')}
            """
                self.admin_stats_label.config(text=stats_text.strip())
            except Exception as e:
                self.logger.exception(
                    "Error parsing admin stats",
                    error_detail=str(e),
                    stop=False
                )
                self.admin_stats_label.config(text=f"⚠️ Error: {str(e)}")
        except Exception as e:
            self.logger.exception(
                "Error displaying admin stats",
                error_detail=str(e),
                stop=False
            )

    def load_admin_users(self):
        """Load admin users"""
        self.logger.info("Loading admin users")

        def load_thread():
            try:
                response = self.api.get_admin_users()
                self.root.after(0, lambda: self._display_admin_users(response))
            except Exception as e:
                self.logger.exception(
                    "Error loading admin users",
                    error_detail=str(e),
                    stop=False
                )

        thread = threading.Thread(target=load_thread, daemon=True)
        thread.start()

    def _display_admin_users(self, response):
        """Display admin users"""
        try:
            if not response.get("success"):
                self.logger.warning("Failed to load admin users")
                self.admin_users_label.config(text="⚠️ Failed to load users")
                return

            try:
                data = json.loads(response["data"])
                users = data.get("users", [])
                
                self.logger.info(
                    "Admin users loaded",
                    extra_info=f"Total users: {len(users)}"
                )

                users_text = f"👥 Total Users: {len(users)}\n\n"
                for user in users[:10]:  # Show first 10
                    users_text += f"  • {user.get('username')} ({user.get('email')})\n"

                if len(users) > 10:
                    users_text += f"\n  ... and {len(users) - 10} more"

                self.admin_users_label.config(text=users_text.strip())
            except Exception as e:
                self.logger.exception(
                    "Error parsing admin users",
                    error_detail=str(e),
                    stop=False
                )
                self.admin_users_label.config(text=f"⚠️ Error: {str(e)}")
        except Exception as e:
            self.logger.exception(
                "Error displaying admin users",
                error_detail=str(e),
                stop=False
            )

    def rebuild_ui(self):
        """Rebuild entire UI after theme change"""
        self.logger.info("Rebuilding UI after theme change")
        
        try:
            self.build_ui()
            if self.current_user:
                self.show_chat_screen()
                self.notebook.select(1)
            else:
                self.show_login_screen()
                self.notebook.select(0)
            
            self.logger.info("UI rebuilt successfully")
        except Exception as e:
            self.logger.exception(
                "Error rebuilding UI",
                error_detail=str(e),
                stop=False
            )

    def logout(self):
        """Logout"""
        self.logger.info(
            "Logout initiated",
            extra_info=f"User: {self.current_user.get('username') if self.current_user else 'Unknown'}"
        )
        
        try:
            self.current_user = None
            self.is_admin = False
            self.current_chat_id = None

            # Disable tabs
            self.notebook.tab(1, state="disabled")
            self.notebook.tab(2, state="disabled")
            self.notebook.tab(3, state="disabled")

            # Clear cookies
            self.api.cookie_jar.clear()
            self.api.message_cache.clear_all()
            
            self.logger.info("Logout successful - session cleared")

            self.show_login_screen()
            self.notebook.select(0)
        except Exception as e:
            self.logger.exception(
                "Error during logout",
                error_detail=str(e),
                stop=False
            )


def main():
    """Main entry point"""
    root = tk.Tk()
    root.iconphoto(False, tk.PhotoImage(file="favicon-32x32.png"))  # Set blank icon

    app = ChatClientApp(root)

    root.mainloop()


if __name__ == "__main__":
    main()
