"""
Chat Client App - Web-Aligned UI
Redesigned to exactly match the web frontend's design system:
  - Navbar (64px) with title and theme toggle
  - 260px sidebar with card-style conversation items
  - Card-based message bubbles (sent/received)
  - Proper spacing, border-radius, and color tokens from config.py
  - Custom tab-bar replacing ttk.Notebook for pixel-perfect styling
  - All fonts from config.FONTS, all colors from config.COLORS
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

# ── Font helpers ──────────────────────────────────────────────────────────────
FONT_SANS   = config.FONTS["sans"][0]   # "Segoe UI"
FONT_MONO   = config.FONTS["mono"][0]   # "Courier New"
FS = config.FONTS["size"]               # size dict

# ── Radius helpers ────────────────────────────────────────────────────────────
R = config.RADIUS   # sm=4, md=8, lg=12, xl=16, 2xl=24

# ── Spacing helpers ───────────────────────────────────────────────────────────
SP = config.SPACING  # {1:4, 2:8, 3:12, 4:16, 5:20, 6:24, 8:32, 10:40, 12:48, 16:64}


# ─────────────────────────────────────────────────────────────────────────────
#  Helper widgets
# ─────────────────────────────────────────────────────────────────────────────

def make_scrollable(parent, bg):
    """Return (canvas, scrollable_frame) packed inside parent."""
    canvas = tk.Canvas(parent, bg=bg, highlightthickness=0)
    sb = ttk.Scrollbar(parent, orient="vertical", command=canvas.yview)
    inner = tk.Frame(canvas, bg=bg)
    inner.bind("<Configure>", lambda e: canvas.configure(scrollregion=canvas.bbox("all")))
    canvas.create_window((0, 0), window=inner, anchor="nw")
    canvas.configure(yscrollcommand=sb.set)
    canvas.pack(side="left", fill="both", expand=True)
    sb.pack(side="right", fill="y")
    # Mouse-wheel scrolling
    def _on_wheel(event):
        canvas.yview_scroll(int(-1 * (event.delta / 120)), "units")
    canvas.bind_all("<MouseWheel>", _on_wheel)
    return canvas, inner


def card_frame(parent, bg_key, border_key, app, padx=SP[6], pady=SP[4], radius=R["lg"]):
    """A Frame that looks like a CSS card (flat bg + 1-px border emulated by outer Frame)."""
    outer = tk.Frame(parent, bg=app.get_color(border_key))
    outer.pack(fill=tk.X, padx=0, pady=(0, SP[3]))
    inner = tk.Frame(outer, bg=app.get_color(bg_key))
    inner.pack(fill=tk.X, padx=1, pady=1)
    return inner


class Separator(tk.Frame):
    """Thin horizontal rule."""
    def __init__(self, parent, color, **kw):
        super().__init__(parent, height=1, bg=color, **kw)
        self.pack(fill=tk.X)


# ─────────────────────────────────────────────────────────────────────────────
#  Styled Button factory  (replaces bare tk.Button throughout)
# ─────────────────────────────────────────────────────────────────────────────

def make_btn(parent, text, command, app, variant="primary",
             size="md", full_width=False, **kw):
    """
    variant: "primary" | "ghost" | "danger" | "secondary"
    size:    "sm" | "md" | "lg"
    """
    c = app.get_color

    size_cfg = {
        "sm": (SP[2], SP[4], FS["sm"]),
        "md": (SP[3], SP[6], FS["base"]),
        "lg": (SP[4], SP[8], FS["lg"]),
    }
    py, px, fs = size_cfg.get(size, size_cfg["md"])

    style = {
        "primary":   dict(bg=c("accent"),        fg="#ffffff",        activebackground=c("accent_hover"),   activeforeground="#ffffff"),
        "ghost":     dict(bg=c("bg_secondary"),   fg=c("fg_secondary"),activebackground=c("bg_tertiary"),    activeforeground=c("fg_primary")),
        "danger":    dict(bg=c("danger"),         fg="#ffffff",        activebackground=c("danger"),          activeforeground="#ffffff"),
        "secondary": dict(bg=c("bg_tertiary"),    fg=c("fg_primary"),  activebackground=c("border"),          activeforeground=c("fg_primary")),
    }[variant]

    btn = tk.Button(
        parent, text=text, command=command,
        relief=tk.FLAT, cursor="hand2",
        font=(FONT_SANS, fs, "bold"),
        padx=px, pady=py,
        bd=0, highlightthickness=0,
        **style, **kw,
    )
    if full_width:
        btn.pack(fill=tk.X)
    return btn


def make_label(parent, text, app, style="primary", size="base", bold=False, **kw):
    """Convenience label with theme-aware colors."""
    fg_map = {
        "primary":   "fg_primary",
        "secondary": "fg_secondary",
        "tertiary":  "fg_tertiary",
        "accent":    "accent",
        "danger":    "danger",
    }
    weight = "bold" if bold else "normal"
    return tk.Label(
        parent, text=text,
        font=(FONT_SANS, FS.get(size, FS["base"]), weight),
        fg=app.get_color(fg_map.get(style, "fg_primary")),
        bg=kw.pop("bg", parent.cget("bg")),
        **kw,
    )


def make_entry(parent, app, show=None, width=None, **kw):
    """Styled Entry with input_bg + 1-px border emulated via outer Frame."""
    c = app.get_color
    outer = tk.Frame(parent, bg=c("input_border"), pady=1, padx=1)
    outer.pack(fill=tk.X, pady=(0, SP[3]))
    entry = tk.Entry(
        outer,
        bg=c("input_bg"), fg=c("fg_primary"),
        insertbackground=c("accent"),
        relief=tk.FLAT, bd=0,
        font=(FONT_SANS, FS["base"]),
        show=show or "",
        highlightthickness=0,
        **({"width": width} if width else {}),
        **kw,
    )
    entry.pack(fill=tk.X, ipady=SP[2], ipadx=SP[3])
    return entry


# ─────────────────────────────────────────────────────────────────────────────
#  Navbar (64 px — matches --navbar-height)
# ─────────────────────────────────────────────────────────────────────────────

class Navbar(tk.Frame):
    TABS = ["Login", "Chat", "Settings", "Admin"]

    def __init__(self, parent, app):
        super().__init__(parent, bg=app.get_color("bg_secondary"),
                         height=config.LAYOUT["navbar_height"])
        self.pack_propagate(False)
        self.pack(fill=tk.X)
        self.app = app
        self._build()

    def _build(self):
        app = self.app
        c = app.get_color

        # Bottom border
        border = tk.Frame(self, bg=c("border"), height=1)
        border.pack(side=tk.BOTTOM, fill=tk.X)

        # Logo / title
        logo = tk.Label(self, text="💬", font=(FONT_SANS, 20),
                        bg=c("bg_secondary"), fg=c("accent"), padx=SP[4])
        logo.pack(side=tk.LEFT)

        title = tk.Label(self, text="Chat App",
                         font=(FONT_SANS, FS["lg"], "bold"),
                         bg=c("bg_secondary"), fg=c("fg_primary"), padx=0)
        title.pack(side=tk.LEFT)

        # Right side: theme toggle + user info
        right = tk.Frame(self, bg=c("bg_secondary"))
        right.pack(side=tk.RIGHT, padx=SP[4])

        self.theme_btn = tk.Button(
            right, text="🌙" if app.theme.theme == "light" else "☀️",
            command=app._toggle_theme,
            bg=c("bg_tertiary"), fg=c("fg_primary"),
            relief=tk.FLAT, cursor="hand2",
            font=(FONT_SANS, FS["lg"]),
            padx=SP[2], pady=SP[1],
            bd=0, highlightthickness=0,
            activebackground=c("border"),
        )
        self.theme_btn.pack(side=tk.RIGHT, padx=(SP[2], 0))

        self.user_label = tk.Label(right, text="",
                                   font=(FONT_SANS, FS["sm"]),
                                   bg=c("bg_secondary"), fg=c("fg_secondary"))
        self.user_label.pack(side=tk.RIGHT, padx=SP[4])

    def update(self):
        """Refresh navbar colors/icons after theme change."""
        app = self.app
        c = app.get_color
        self.configure(bg=c("bg_secondary"))
        for w in self.winfo_children():
            if isinstance(w, (tk.Label, tk.Button, tk.Frame)):
                try:
                    w.configure(bg=c("bg_secondary"))
                except Exception:
                    pass
        self.theme_btn.configure(
            text="🌙" if app.theme.theme == "light" else "☀️",
            bg=c("bg_tertiary"),
        )

    def set_user(self, username):
        self.user_label.configure(text=username or "")


# ─────────────────────────────────────────────────────────────────────────────
#  Custom Tab Bar (replaces ttk.Notebook for exact style match)
# ─────────────────────────────────────────────────────────────────────────────

class TabBar(tk.Frame):
    """A horizontal tab bar that swaps content frames, styled like panel-tabs."""

    def __init__(self, parent, app):
        super().__init__(parent, bg=app.get_color("bg_secondary"))
        self.pack(fill=tk.X)
        self.app = app
        self._tabs = {}     # name -> (btn, frame)
        self._active = None
        self._bottom_border = tk.Frame(self, bg=app.get_color("border"), height=1)
        self._bottom_border.pack(side=tk.BOTTOM, fill=tk.X)

    def add(self, name, frame, enabled=True):
        c = self.app.get_color
        btn = tk.Button(
            self, text=name,
            command=lambda n=name: self.select(n),
            relief=tk.FLAT, bd=0, highlightthickness=0,
            font=(FONT_SANS, FS["sm"], "bold"),
            bg=c("bg_secondary"), fg=c("fg_secondary"),
            activebackground=c("bg_secondary"),
            padx=SP[4], pady=SP[3],
            cursor="hand2" if enabled else "arrow",
            state=tk.NORMAL if enabled else tk.DISABLED,
        )
        btn.pack(side=tk.LEFT)
        self._tabs[name] = (btn, frame)
        # place frame but hide it
        frame.place_forget()

    def select(self, name):
        if name not in self._tabs:
            return
        btn, frame = self._tabs[name]
        if btn["state"] == tk.DISABLED:
            return
        c = self.app.get_color
        # deactivate old
        if self._active and self._active != name:
            old_btn, old_frame = self._tabs[self._active]
            old_btn.configure(fg=c("fg_secondary"),
                              bg=c("bg_secondary"),
                              activebackground=c("bg_secondary"))
            old_frame.pack_forget()
        # activate new
        btn.configure(fg=c("accent"),
                      bg=c("bg_secondary"),
                      activebackground=c("bg_secondary"))
        frame.pack(fill=tk.BOTH, expand=True)
        self._active = name
        # Fire on_tab_changed
        self.app.on_tab_changed(name)

    def set_enabled(self, name, enabled):
        if name in self._tabs:
            btn, _ = self._tabs[name]
            btn.configure(
                state=tk.NORMAL if enabled else tk.DISABLED,
                cursor="hand2" if enabled else "arrow",
            )

    def refresh_colors(self):
        c = self.app.get_color
        self.configure(bg=c("bg_secondary"))
        self._bottom_border.configure(bg=c("border"))
        for name, (btn, _) in self._tabs.items():
            is_active = name == self._active
            btn.configure(
                bg=c("bg_secondary"),
                fg=c("accent") if is_active else c("fg_secondary"),
                activebackground=c("bg_secondary"),
            )

    @property
    def active(self):
        return self._active


# ─────────────────────────────────────────────────────────────────────────────
#  Main Application
# ─────────────────────────────────────────────────────────────────────────────

class ChatClientApp:
    """Chat client redesigned to match web frontend design system."""

    def __init__(self, root):
        dev_mode = os.getenv("CHAT_DEV_MODE", "false").lower() == "true"
        self.logger = Logger(dev_mode=dev_mode)
        self.logger.separator("CHAT CLIENT INITIALIZATION")

        self.root = root
        self.root.title("Chat App")
        self.root.geometry(
            f"{config.LAYOUT['window_width']}x{config.LAYOUT['window_height']}"
        )
        self.root.minsize(
            config.LAYOUT["window_minsize_width"],
            config.LAYOUT["window_minsize_height"],
        )

        # API
        try:
            self.api = ChatAPIClient(config.DEFAULT_SERVER)
        except Exception as e:
            self.logger.exception("Failed to initialize API client", str(e), stop=True)
            raise

        # State
        self.current_user = None
        self.is_admin = False
        self.current_chat_id = None
        self.stream_thread = None
        self.typing_users = set()
        self.unread_counts = defaultdict(int)

        # Theme
        self.theme = ThemeManager(root)
        self.theme.subscribe(self.rebuild_ui)

        # Favicon
        try:
            self.load_favicon()
        except Exception as e:
            self.logger.warning("Favicon loading failed", str(e))

        # Build
        self.build_ui()
        self.show_login_screen()
        self.logger.separator("CHAT CLIENT READY")

    # ── Color shortcut ────────────────────────────────────────────────────────

    def get_color(self, name):
        return self.theme.colors.get(name, "#000000")

    # ── Favicon ───────────────────────────────────────────────────────────────

    def load_favicon(self):
        favicon_path = os.path.expanduser("~/.chat_client_favicon.png")
        favicon_url = f"{self.api.server_url}/static/icons/favicons/favicon-32x32.png"
        if os.path.exists(favicon_path):
            try:
                icon = tk.PhotoImage(file=favicon_path)
                self.root.iconphoto(False, icon)
                return
            except Exception:
                pass
        try:
            result = subprocess.run(
                ["curl", "-s", "-o", favicon_path, favicon_url],
                capture_output=True, timeout=3,
            )
            if result.returncode == 0 and os.path.exists(favicon_path):
                icon = tk.PhotoImage(file=favicon_path)
                self.root.iconphoto(False, icon)
                return
        except Exception:
            pass
        try:
            icon = tk.PhotoImage(width=32, height=32)
            accent = self.get_color("accent")
            for i in range(32):
                for j in range(32):
                    icon.put(accent, (i, j))
            self.root.iconphoto(False, icon)
        except Exception:
            pass

    # ── Build UI ──────────────────────────────────────────────────────────────

    def build_ui(self):
        """Construct the full window layout with Navbar + TabBar + content."""
        for w in self.root.winfo_children():
            w.destroy()

        c = self.get_color
        self.root.configure(bg=c("bg_primary"))

        # Wrapper
        self.root_frame = tk.Frame(self.root, bg=c("bg_primary"))
        self.root_frame.pack(fill=tk.BOTH, expand=True)

        # ── Navbar ────────────────────────────────────────────────────────────
        self.navbar = Navbar(self.root_frame, self)

        # ── Tab bar + content area ─────────────────────────────────────────
        self.tab_bar = TabBar(self.root_frame, self)

        self.content_area = tk.Frame(self.root_frame, bg=c("bg_primary"))
        self.content_area.pack(fill=tk.BOTH, expand=True)

        # Tab content frames
        self.login_frame    = tk.Frame(self.content_area, bg=c("bg_secondary"))
        self.chat_frame     = tk.Frame(self.content_area, bg=c("bg_primary"))
        self.settings_frame = tk.Frame(self.content_area, bg=c("bg_primary"))
        self.admin_frame    = tk.Frame(self.content_area, bg=c("bg_primary"))

        self.tab_bar.add("Login",    self.login_frame,    enabled=True)
        self.tab_bar.add("Chat",     self.chat_frame,     enabled=False)
        self.tab_bar.add("Settings", self.settings_frame, enabled=False)
        self.tab_bar.add("Admin",    self.admin_frame,    enabled=False)

        # ttk style (used only by Scrollbar)
        style = ttk.Style()
        style.theme_use("clam")
        style.configure("Vertical.TScrollbar",
                         background=c("bg_tertiary"),
                         troughcolor=c("bg_secondary"),
                         bordercolor=c("bg_secondary"),
                         arrowcolor=c("fg_secondary"))

        self.tab_bar.select("Login")

    def on_tab_changed(self, name):
        if name == "Chat":
            self.show_chat_screen()
        elif name == "Settings":
            self.show_settings_screen()
        elif name == "Admin":
            self.show_admin_screen()

    def rebuild_ui(self):
        """Full rebuild after theme change."""
        try:
            was_logged_in = self.current_user is not None
            self.build_ui()
            if was_logged_in:
                self.tab_bar.set_enabled("Chat", True)
                self.tab_bar.set_enabled("Settings", True)
                if self.is_admin:
                    self.tab_bar.set_enabled("Admin", True)
                self.navbar.set_user(
                    f"@{self.current_user.get('username', '')}"
                )
                self.tab_bar.select("Chat")
            else:
                self.show_login_screen()
                self.tab_bar.select("Login")
        except Exception as e:
            self.logger.exception("Error rebuilding UI", str(e), stop=False)

    def _toggle_theme(self):
        self.theme.toggle()

    # ─────────────────────────────────────────────────────────────────────────
    #  Login / Register
    # ─────────────────────────────────────────────────────────────────────────

    def show_login_screen(self):
        """Auth card centred on a bg_secondary canvas — mirrors index.html."""
        for w in self.login_frame.winfo_children():
            w.destroy()
        c = self.get_color
        self.login_frame.configure(bg=c("bg_secondary"))

        # Centred single-column layout
        outer = tk.Frame(self.login_frame, bg=c("bg_secondary"))
        outer.place(relx=0.5, rely=0.5, anchor="center")

        # ── Auth card ────────────────────────────────────────────────────────
        card_border = tk.Frame(outer, bg=c("border"))
        card_border.pack()
        card = tk.Frame(card_border, bg=c("bg_primary"),
                        padx=SP[8], pady=SP[8])
        card.pack(padx=1, pady=1)

        # Logo
        tk.Label(card, text="💬", font=(FONT_SANS, 48),
                 bg=c("bg_primary"), fg=c("accent")).pack(pady=(0, SP[4]))

        # Title
        make_label(card, "Welcome Back", self, style="primary",
                   size="3xl", bold=True, bg=c("bg_primary")).pack()

        # Subtitle
        make_label(card, "Sign in to continue to your account", self,
                   style="secondary", size="sm", bg=c("bg_primary")).pack(pady=(SP[1], SP[6]))

        Separator(card, c("border"))

        form = tk.Frame(card, bg=c("bg_primary"))
        form.pack(fill=tk.X, pady=SP[5])

        # Server URL
        make_label(form, "Server URL", self, style="secondary",
                   size="sm", bold=True, bg=c("bg_primary")).pack(anchor=tk.W, pady=(0, SP[1]))
        server_var = tk.StringVar(value=config.DEFAULT_SERVER)
        server_entry = make_entry(form, self)
        server_entry.insert(0, config.DEFAULT_SERVER)

        # Username
        make_label(form, "Username", self, style="secondary",
                   size="sm", bold=True, bg=c("bg_primary")).pack(anchor=tk.W, pady=(0, SP[1]))
        self.username_input = make_entry(form, self)
        self.username_input.bind("<Return>", lambda e: self.handle_login())

        # Password
        make_label(form, "Password", self, style="secondary",
                   size="sm", bold=True, bg=c("bg_primary")).pack(anchor=tk.W, pady=(0, SP[1]))
        self.password_input = make_entry(form, self, show="•")
        self.password_input.bind("<Return>", lambda e: self.handle_login())

        # Buttons
        btn_row = tk.Frame(card, bg=c("bg_primary"))
        btn_row.pack(fill=tk.X, pady=(SP[2], 0))

        sign_in = make_btn(btn_row, "Sign In", self.handle_login, self,
                           variant="primary", size="lg")
        sign_in.pack(fill=tk.X, pady=(0, SP[2]))

        make_btn(btn_row, "Create Account", self.show_register_screen, self,
                 variant="ghost", size="md").pack(fill=tk.X)

    def show_register_screen(self):
        for w in self.login_frame.winfo_children():
            w.destroy()
        c = self.get_color
        self.login_frame.configure(bg=c("bg_secondary"))

        outer = tk.Frame(self.login_frame, bg=c("bg_secondary"))
        outer.place(relx=0.5, rely=0.5, anchor="center")

        card_border = tk.Frame(outer, bg=c("border"))
        card_border.pack()
        card = tk.Frame(card_border, bg=c("bg_primary"), padx=SP[8], pady=SP[8])
        card.pack(padx=1, pady=1)

        tk.Label(card, text="💬", font=(FONT_SANS, 48),
                 bg=c("bg_primary"), fg=c("accent")).pack(pady=(0, SP[3]))
        make_label(card, "Create Account", self, style="primary",
                   size="3xl", bold=True, bg=c("bg_primary")).pack()
        make_label(card, "Join our community today", self,
                   style="secondary", size="sm", bg=c("bg_primary")).pack(pady=(SP[1], SP[5]))

        Separator(card, c("border"))

        form = tk.Frame(card, bg=c("bg_primary"))
        form.pack(fill=tk.X, pady=SP[5])

        fields = [
            ("Email Address", False),
            ("Full Name",     False),
            ("Username",      False),
            ("Password",      True),
        ]
        entries = {}
        for label, is_pw in fields:
            make_label(form, label, self, style="secondary",
                       size="sm", bold=True, bg=c("bg_primary")).pack(anchor=tk.W, pady=(0, SP[1]))
            e = make_entry(form, self, show="•" if is_pw else None)
            entries[label] = e

        btn_row = tk.Frame(card, bg=c("bg_primary"))
        btn_row.pack(fill=tk.X, pady=(SP[2], 0))

        def _register():
            self.handle_register(
                entries["Email Address"].get(),
                entries["Username"].get(),
                entries["Password"].get(),
                entries["Full Name"].get(),
            )

        make_btn(btn_row, "Create Account", _register, self,
                 variant="primary", size="lg").pack(fill=tk.X, pady=(0, SP[2]))
        make_btn(btn_row, "← Back to Sign In", self.show_login_screen, self,
                 variant="ghost", size="md").pack(fill=tk.X)

    # ─────────────────────────────────────────────────────────────────────────
    #  Login / Register handlers
    # ─────────────────────────────────────────────────────────────────────────

    def handle_login(self):
        username = self.username_input.get()
        password = self.password_input.get()
        if not username or not password:
            messagebox.showerror("Error", "Please enter username and password")
            return

        def t():
            try:
                r = self.api.login(username, password)
                self.root.after(0, lambda: self._handle_login_response(r, username))
            except Exception as e:
                self.root.after(0, lambda: messagebox.showerror("Error", f"Login failed: {e}"))

        threading.Thread(target=t, daemon=True).start()

    def _handle_login_response(self, response, username):
        if response.get("success"):
            try:
                user_data = self._unwrap(response) or {}
                if not isinstance(user_data, dict):
                    user_data = {}
                self.current_user = user_data
                self.is_admin = user_data.get("is_admin", False)
                self.navbar.set_user(f"@{user_data.get('username', username)}")
                self.tab_bar.set_enabled("Chat", True)
                self.tab_bar.set_enabled("Settings", True)
                if self.is_admin:
                    self.tab_bar.set_enabled("Admin", True)
                self.tab_bar.select("Chat")
            except Exception as e:
                messagebox.showerror("Error", f"Failed to parse login response: {e}")
        else:
            payload = self._unwrap(response)
            msg = (payload.get("message") if isinstance(payload, dict) else str(payload)) or "Login failed"
            messagebox.showerror("Login Failed", msg)

    def handle_register(self, email, username, password, fullname):
        if not all([email, username, password, fullname]):
            messagebox.showerror("Error", "All fields are required")
            return

        def t():
            try:
                r = self.api.register(email, username, password, fullname)
                self.root.after(0, lambda: self._handle_register_response(r, email))
            except Exception as e:
                self.root.after(0, lambda: messagebox.showerror("Error", f"Registration failed: {e}"))

        threading.Thread(target=t, daemon=True).start()

    def _handle_register_response(self, response, email):
        if response.get("success"):
            messagebox.showinfo("Success", "Account created! Please sign in.")
            self.show_login_screen()
        else:
            messagebox.showerror("Registration Failed", response.get("data", "Unknown error"))

    # ─────────────────────────────────────────────────────────────────────────
    #  Chat screen
    # ─────────────────────────────────────────────────────────────────────────

    def show_chat_screen(self):
        for w in self.chat_frame.winfo_children():
            w.destroy()
        c = self.get_color
        self.chat_frame.configure(bg=c("bg_primary"))

        container = tk.Frame(self.chat_frame, bg=c("bg_primary"))
        container.pack(fill=tk.BOTH, expand=True)

        # ── Sidebar (260px — matches --sidebar-width) ─────────────────────
        sidebar = tk.Frame(container, bg=c("bg_secondary"),
                           width=config.LAYOUT["sidebar_width"])
        sidebar.pack(side=tk.LEFT, fill=tk.Y)
        sidebar.pack_propagate(False)

        # Sidebar header
        sidebar_hdr = tk.Frame(sidebar, bg=c("bg_secondary"), height=SP[12])
        sidebar_hdr.pack(fill=tk.X, padx=SP[4], pady=SP[4])
        sidebar_hdr.pack_propagate(False)

        make_label(sidebar_hdr, "Conversations", self, style="primary",
                   size="base", bold=True, bg=c("bg_secondary")).pack(side=tk.LEFT, anchor="w")

        btn_row = tk.Frame(sidebar_hdr, bg=c("bg_secondary"))
        btn_row.pack(side=tk.RIGHT)

        make_btn(btn_row, "↺", self.load_conversations, self,
                 variant="ghost", size="sm").pack(side=tk.LEFT, padx=(0, SP[1]))
        make_btn(btn_row, "+ New", self.new_conversation, self,
                 variant="primary", size="sm").pack(side=tk.LEFT)

        Separator(sidebar, c("border"))

        # Scrollable conv list
        _, self.conv_frame = make_scrollable(sidebar, c("bg_secondary"))

        # ── Right border ──────────────────────────────────────────────────
        tk.Frame(container, bg=c("border"), width=1).pack(side=tk.LEFT, fill=tk.Y)

        # ── Chat area ────────────────────────────────────────────────────
        chat_area = tk.Frame(container, bg=c("bg_primary"))
        chat_area.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

        # Empty state header
        self.chat_header = tk.Frame(chat_area, bg=c("bg_secondary"),
                                    height=SP[12])
        self.chat_header.pack(fill=tk.X)
        self.chat_header.pack_propagate(False)
        self.chat_title_label = make_label(
            self.chat_header, "Select a conversation", self,
            style="secondary", size="sm", bg=c("bg_secondary"))
        self.chat_title_label.pack(side=tk.LEFT, padx=SP[4], pady=SP[3])

        Separator(chat_area, c("border"))

        # Messages scroll area
        msg_canvas = tk.Canvas(chat_area, bg=c("bg_primary"), highlightthickness=0)
        msg_sb = ttk.Scrollbar(chat_area, orient="vertical", command=msg_canvas.yview)
        self.messages_frame = tk.Frame(msg_canvas, bg=c("bg_primary"))
        self.messages_frame.bind(
            "<Configure>",
            lambda e: msg_canvas.configure(scrollregion=msg_canvas.bbox("all"))
        )
        msg_canvas.create_window((0, 0), window=self.messages_frame, anchor="nw")
        msg_canvas.configure(yscrollcommand=msg_sb.set)
        msg_sb.pack(side=tk.RIGHT, fill=tk.Y)
        msg_canvas.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)
        self._msg_canvas = msg_canvas

        # ── Input area (chat_input_height = 80px) ──────────────────────
        Separator(chat_area, c("border"))
        input_area = tk.Frame(chat_area, bg=c("bg_secondary"),
                              height=config.LAYOUT["chat_input_height"])
        input_area.pack(fill=tk.X, side=tk.BOTTOM)
        input_area.pack_propagate(False)

        input_inner = tk.Frame(input_area, bg=c("bg_secondary"))
        input_inner.pack(fill=tk.BOTH, expand=True, padx=SP[4], pady=SP[3])

        # Text input with border
        txt_border = tk.Frame(input_inner, bg=c("input_border"), padx=1, pady=1)
        txt_border.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)
        self.message_input = tk.Text(
            txt_border, height=2,
            bg=c("input_bg"), fg=c("fg_primary"),
            insertbackground=c("accent"),
            relief=tk.FLAT, bd=0,
            font=(FONT_SANS, FS["base"]),
            wrap=tk.WORD,
        )
        self.message_input.pack(fill=tk.BOTH, expand=True,
                                 ipady=SP[2], ipadx=SP[3])
        self.message_input.bind("<Control-Return>", lambda e: self.send_message())

        send_btn = make_btn(input_inner, "Send ↑", self.send_message, self,
                            variant="primary", size="md")
        send_btn.pack(side=tk.RIGHT, padx=(SP[3], 0))

        # Hint text
        hint = make_label(input_area, "Ctrl+Enter to send", self,
                          style="tertiary", size="xs", bg=c("bg_secondary"))
        hint.pack(side=tk.BOTTOM, padx=SP[4], pady=(0, SP[1]))

        self.load_conversations()

    # ── Response parsing helpers ──────────────────────────────────────────────

    @staticmethod
    def _unwrap(response):
        """
        The Rust server always wraps payloads in a JSON envelope:
            { "status": "success", "data": <actual_payload> }

        The HTTP client stores that envelope string in response["data"].
        This helper unwraps it and returns the inner payload, or None on error.

        It also handles the case where the inner data is itself a JSON string
        (double-encoded) by attempting one additional parse.
        """
        try:
            outer = json.loads(response.get("data", "null"))
        except (json.JSONDecodeError, TypeError):
            return None

        # Already the real payload (list / dict not wrapped in envelope)
        if isinstance(outer, (list, int, float, bool)):
            return outer

        if isinstance(outer, dict):
            # Standard server envelope: {"status": ..., "data": ...}
            if "data" in outer:
                inner = outer["data"]
                # Inner value might itself be a JSON string (double-encoded)
                if isinstance(inner, str):
                    try:
                        return json.loads(inner)
                    except (json.JSONDecodeError, TypeError):
                        return inner
                return inner
            # Dict without "data" key — return as-is (e.g. login user object)
            return outer

        return outer

    @staticmethod
    def _normalise_chat(raw):
        """
        Normalise a chat/group entry regardless of how the server returns it.

        Accepted shapes:
          • dict with at least "id" and optionally "name" / "chat_name" / "group_name"
          • str  — treated as the chat name; id will be None (best-effort)
          • int  — treated as the chat id; name will be "Chat <id>"
        """
        if isinstance(raw, dict):
            chat_id = raw.get("id") or raw.get("chat_id") or raw.get("group_id")
            name = (
                raw.get("name")
                or raw.get("chat_name")
                or raw.get("group_name")
                or raw.get("username")       # DM chats may use the other user's name
                or f"Chat {chat_id}"
            )
            return {"id": chat_id, "name": name, "_raw": raw}
        if isinstance(raw, str):
            return {"id": None, "name": raw, "_raw": raw}
        if isinstance(raw, int):
            return {"id": raw, "name": f"Chat {raw}", "_raw": raw}
        return {"id": None, "name": "Unknown", "_raw": raw}

    @staticmethod
    def _normalise_message(raw):
        """
        Normalise a message entry regardless of server shape.

        Accepted shapes:
          • dict with content / body / message, sender / sender_id / username
          • str — treated as content with unknown sender
        """
        if isinstance(raw, dict):
            content = (
                raw.get("content")
                or raw.get("body")
                or raw.get("message")
                or ""
            )
            sender = (
                raw.get("sender")
                or raw.get("sender_username")
                or raw.get("username")
                or str(raw.get("sender_id", "Unknown"))
            )
            return {
                "id":       raw.get("id"),
                "sender":   sender,
                "content":  content,
                "sent_at":  raw.get("sent_at") or raw.get("timestamp") or "",
                "_raw":     raw,
            }
        if isinstance(raw, str):
            return {"id": None, "sender": "Unknown", "content": raw, "sent_at": "", "_raw": raw}
        return {"id": None, "sender": "Unknown", "content": str(raw), "sent_at": "", "_raw": raw}

    # ─────────────────────────────────────────────────────────────────────────

    def load_conversations(self):
        def t():
            try:
                r = self.api.get_conversations()
                self.root.after(0, lambda: self._display_conversations(r))
            except Exception as e:
                self.logger.exception("Error loading conversations", str(e), stop=False)
        threading.Thread(target=t, daemon=True).start()

    def _display_conversations(self, response):
        for w in self.conv_frame.winfo_children():
            w.destroy()
        c = self.get_color

        if not response.get("success"):
            raw_err = response.get("data", "")
            try:
                err_obj = json.loads(raw_err)
                msg = err_obj.get("message") or err_obj.get("error") or raw_err
            except Exception:
                msg = raw_err or "Failed to load"
            make_label(self.conv_frame, f"⚠ {msg[:60]}", self,
                       style="danger", size="sm", bg=c("bg_secondary")).pack(padx=SP[4], pady=SP[4])
            return

        payload = self._unwrap(response)

        # payload may be a list directly, or a dict with a "chats"/"conversations" key
        if isinstance(payload, dict):
            raw_list = (
                payload.get("chats")
                or payload.get("conversations")
                or payload.get("groups")
                or []
            )
        elif isinstance(payload, list):
            raw_list = payload
        else:
            raw_list = []

        conversations = [self._normalise_chat(item) for item in raw_list]

        if not conversations:
            make_label(self.conv_frame, "No conversations yet", self,
                       style="tertiary", size="sm", bg=c("bg_secondary")).pack(padx=SP[4], pady=SP[6])
            return

        for conv in conversations:
            self._make_conv_item(conv, conv["name"])

    def _make_conv_item(self, conv, name):
        """Render a sidebar conversation item — mirrors .conversation-item."""
        c = self.get_color
        safe_name = name if name else "?"

        item = tk.Frame(self.conv_frame, bg=c("bg_secondary"), cursor="hand2")
        item.pack(fill=tk.X)

        inner = tk.Frame(item, bg=c("bg_secondary"))
        inner.pack(fill=tk.X, padx=SP[3], pady=SP[2])

        # Avatar circle (first letter)
        avatar_bg = c("accent_light")
        avatar_fg = c("accent")
        avatar = tk.Label(inner, text=safe_name[0].upper(),
                          bg=avatar_bg, fg=avatar_fg,
                          font=(FONT_SANS, FS["base"], "bold"),
                          width=2, height=1)
        avatar.pack(side=tk.LEFT, padx=(0, SP[3]))

        info = tk.Frame(inner, bg=c("bg_secondary"))
        info.pack(side=tk.LEFT, fill=tk.X, expand=True)

        make_label(info, safe_name, self, style="primary",
                   size="sm", bold=True, bg=c("bg_secondary")).pack(anchor=tk.W)

        Separator(self.conv_frame, c("border_light"))

        def on_click(e=None, cv=conv):
            self.select_conversation(cv)
            self.chat_title_label.configure(text=cv.get("name", "Chat"))

        for w in [item, inner, avatar, info]:
            w.bind("<Button-1>", on_click)
        for lbl in info.winfo_children():
            lbl.bind("<Button-1>", on_click)

        def on_enter(e, w=item): w.configure(bg=c("bg_tertiary"))
        def on_leave(e, w=item): w.configure(bg=c("bg_secondary"))
        item.bind("<Enter>", on_enter)
        item.bind("<Leave>", on_leave)

    def select_conversation(self, conversation):
        chat_id = conversation.get("id")
        if not chat_id:
            self.logger.warning("select_conversation called with no id", extra_info=str(conversation))
            return
        self.current_chat_id = chat_id

        def t():
            try:
                r = self.api.get_messages(self.current_chat_id)
                self.root.after(0, lambda: self._display_messages(r))
            except Exception as e:
                self.logger.exception("Error loading messages", str(e), stop=False)
        threading.Thread(target=t, daemon=True).start()

    def _display_messages(self, response):
        for w in self.messages_frame.winfo_children():
            w.destroy()
        c = self.get_color

        if not response.get("success"):
            make_label(self.messages_frame, "⚠ Failed to load messages", self,
                       style="danger", bg=c("bg_primary")).pack(padx=SP[4], pady=SP[4])
            return

        payload = self._unwrap(response)

        if isinstance(payload, dict):
            raw_list = (
                payload.get("messages")
                or payload.get("items")
                or []
            )
        elif isinstance(payload, list):
            raw_list = payload
        else:
            raw_list = []

        messages = [self._normalise_message(m) for m in raw_list]

        if not messages:
            make_label(self.messages_frame, "No messages yet. Say hello! 👋", self,
                       style="tertiary", bg=c("bg_primary")).pack(expand=True, pady=SP[16])
            return

        own_username = (self.current_user or {}).get("username", "")
        own_id = str((self.current_user or {}).get("id", ""))
        for msg in messages:
            sender = msg["sender"]
            content = msg["content"]
            is_own = (sender == own_username) or (sender == own_id)
            self._render_message(sender, content, is_own)

        # Scroll to bottom
        self.messages_frame.update_idletasks()
        self._msg_canvas.yview_moveto(1.0)

    def _render_message(self, sender, content, is_own):
        """Render a single message bubble — mirrors .message sent/received."""
        c = self.get_color

        row = tk.Frame(self.messages_frame, bg=c("bg_primary"))
        row.pack(fill=tk.X, padx=SP[4], pady=SP[2])

        # Bubble color: sent = accent, received = bg_secondary
        bubble_bg = c("accent") if is_own else c("bg_secondary")
        bubble_fg = "#ffffff" if is_own else c("fg_primary")
        anchor = tk.E if is_own else tk.W

        bubble_outer = tk.Frame(row, bg=c("bg_primary"))
        bubble_outer.pack(anchor=anchor)

        if not is_own:
            # Sender label above bubble
            make_label(bubble_outer, sender, self, style="tertiary",
                       size="xs", bg=c("bg_primary")).pack(anchor=tk.W, pady=(0, 2))

        # Message bubble (border frame)
        bdr_color = c("accent") if is_own else c("border")
        bdr = tk.Frame(bubble_outer, bg=bdr_color)
        bdr.pack()
        bubble = tk.Label(
            bdr, text=content,
            bg=bubble_bg, fg=bubble_fg,
            font=(FONT_SANS, FS["base"]),
            wraplength=320,
            justify=tk.LEFT,
            padx=SP[4], pady=SP[3],
        )
        bubble.pack(padx=1, pady=1)

    def send_message(self):
        if not self.current_chat_id:
            messagebox.showwarning("Warning", "Please select a conversation first")
            return
        content = self.message_input.get("1.0", tk.END).strip()
        if not content:
            return

        def t():
            try:
                r = self.api.send_message(self.current_chat_id, content)
                self.root.after(0, lambda: self._handle_send_response(r))
            except Exception as e:
                self.root.after(0, lambda: messagebox.showerror("Error", "Failed to send message"))
        threading.Thread(target=t, daemon=True).start()

    def _handle_send_response(self, response):
        if response.get("success"):
            self.message_input.delete("1.0", tk.END)
            # Reload messages for the current chat
            if self.current_chat_id:
                self.select_conversation({"id": self.current_chat_id, "name": ""})
        else:
            payload = self._unwrap(response)
            msg = (payload.get("message") if isinstance(payload, dict) else str(payload)) or "Send failed"
            messagebox.showerror("Error", msg)

    def new_conversation(self):
        """Open the 'New Conversation' modal — lets user pick Direct Chat or Group."""
        self._show_new_conv_modal()

    # ─────────────────────────────────────────────────────────────────────────
    #  New Conversation / New Group modals
    # ─────────────────────────────────────────────────────────────────────────

    def _make_modal(self, title, width=420, height=None):
        """
        Create and return a styled Toplevel modal window centred over the root.
        Also returns the inner content Frame the caller should populate.
        """
        c = self.get_color
        modal = tk.Toplevel(self.root)
        modal.title(title)
        modal.configure(bg=c("bg_primary"))
        modal.resizable(False, False)
        modal.transient(self.root)
        modal.grab_set()

        # Centre over root
        self.root.update_idletasks()
        rx, ry = self.root.winfo_rootx(), self.root.winfo_rooty()
        rw, rh = self.root.winfo_width(), self.root.winfo_height()
        x = rx + (rw - width) // 2
        y = ry + (rh - (height or 400)) // 2
        modal.geometry(f"{width}x{height or 400}+{x}+{y}")

        # Border frame
        bdr = tk.Frame(modal, bg=c("border"))
        bdr.pack(fill=tk.BOTH, expand=True, padx=1, pady=1)
        card = tk.Frame(bdr, bg=c("bg_primary"))
        card.pack(fill=tk.BOTH, expand=True, padx=1, pady=1)

        # Header bar
        hdr = tk.Frame(card, bg=c("bg_secondary"))
        hdr.pack(fill=tk.X)
        make_label(hdr, title, self, style="primary", size="base",
                   bold=True, bg=c("bg_secondary")).pack(side=tk.LEFT,
                                                         padx=SP[5], pady=SP[4])
        close_btn = tk.Button(hdr, text="✕", command=modal.destroy,
                              bg=c("bg_secondary"), fg=c("fg_tertiary"),
                              relief=tk.FLAT, bd=0, highlightthickness=0,
                              font=(FONT_SANS, FS["base"]), cursor="hand2",
                              activebackground=c("bg_tertiary"),
                              padx=SP[3], pady=SP[2])
        close_btn.pack(side=tk.RIGHT, padx=SP[2], pady=SP[2])
        Separator(card, c("border"))

        content = tk.Frame(card, bg=c("bg_primary"), padx=SP[5], pady=SP[5])
        content.pack(fill=tk.BOTH, expand=True)

        return modal, content

    def _show_new_conv_modal(self):
        """
        Step 1 — pick: Direct Message or Group chat.
        """
        modal, content = self._make_modal("New Conversation", width=380, height=220)
        c = self.get_color

        make_label(content, "What would you like to start?", self,
                   style="secondary", size="sm",
                   bg=c("bg_primary")).pack(pady=(SP[3], SP[5]))

        def _open_direct():
            modal.destroy()
            self._show_new_direct_modal()

        def _open_group():
            modal.destroy()
            self._show_new_group_modal()

        make_btn(content, "💬  Direct Message", _open_direct, self,
                 variant="primary", size="md").pack(fill=tk.X, pady=(0, SP[2]))
        make_btn(content, "👥  Group Chat", _open_group, self,
                 variant="secondary", size="md").pack(fill=tk.X)

    def _show_new_direct_modal(self):
        """
        New Direct Message — search for a user then create a 1-on-1 chat.

        POST /api/chats  {user_id: <id>}
        GET  /api/users/search?q=<query>
        """
        modal, content = self._make_modal("New Direct Message", width=420, height=360)
        c = self.get_color

        make_label(content, "Search for a user", self, style="secondary",
                   size="sm", bold=True, bg=c("bg_primary")).pack(anchor=tk.W, pady=(0, SP[1]))

        # Search row
        search_row = tk.Frame(content, bg=c("bg_primary"))
        search_row.pack(fill=tk.X, pady=(0, SP[3]))

        search_bdr = tk.Frame(search_row, bg=c("input_border"), padx=1, pady=1)
        search_bdr.pack(side=tk.LEFT, fill=tk.X, expand=True, padx=(0, SP[2]))
        search_entry = tk.Entry(search_bdr, bg=c("input_bg"), fg=c("fg_primary"),
                                insertbackground=c("accent"),
                                relief=tk.FLAT, bd=0,
                                font=(FONT_SANS, FS["base"]),
                                highlightthickness=0)
        search_entry.pack(fill=tk.X, ipady=SP[2], ipadx=SP[3])
        search_entry.focus_set()

        # Results list frame
        results_outer = tk.Frame(content, bg=c("border"))
        results_outer.pack(fill=tk.BOTH, expand=True, pady=(0, SP[3]))
        results_inner = tk.Frame(results_outer, bg=c("bg_secondary"))
        results_inner.pack(fill=tk.BOTH, expand=True, padx=1, pady=1)

        # Scrollable results
        results_canvas = tk.Canvas(results_inner, bg=c("bg_secondary"),
                                   highlightthickness=0, height=140)
        results_sb = ttk.Scrollbar(results_inner, orient="vertical",
                                   command=results_canvas.yview)
        results_frame = tk.Frame(results_canvas, bg=c("bg_secondary"))
        results_frame.bind("<Configure>",
                           lambda e: results_canvas.configure(
                               scrollregion=results_canvas.bbox("all")))
        results_canvas.create_window((0, 0), window=results_frame, anchor="nw")
        results_canvas.configure(yscrollcommand=results_sb.set)
        results_sb.pack(side=tk.RIGHT, fill=tk.Y)
        results_canvas.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

        status_lbl = make_label(content, "", self, style="tertiary",
                                size="xs", bg=c("bg_primary"))
        status_lbl.pack(anchor=tk.W)

        selected_user = {"id": None, "username": None}

        def _render_results(users):
            for w in results_frame.winfo_children():
                w.destroy()
            if not users:
                make_label(results_frame, "No users found", self,
                           style="tertiary", size="sm",
                           bg=c("bg_secondary")).pack(padx=SP[4], pady=SP[4])
                return
            for u in users:
                uid = u.get("id")
                uname = u.get("username") or u.get("name") or str(uid)
                row = tk.Frame(results_frame, bg=c("bg_secondary"), cursor="hand2")
                row.pack(fill=tk.X)
                inner = tk.Frame(row, bg=c("bg_secondary"))
                inner.pack(fill=tk.X, padx=SP[3], pady=SP[2])
                # Avatar
                av = tk.Label(inner, text=uname[0].upper(),
                              bg=c("accent_light"), fg=c("accent"),
                              font=(FONT_SANS, FS["sm"], "bold"), width=2)
                av.pack(side=tk.LEFT, padx=(0, SP[2]))
                nm = make_label(inner, uname, self, style="primary",
                                size="sm", bg=c("bg_secondary"))
                nm.pack(side=tk.LEFT, anchor=tk.W)
                Separator(results_frame, c("border_light"))

                def _select(e=None, u_id=uid, u_name=uname):
                    selected_user["id"] = u_id
                    selected_user["username"] = u_name
                    status_lbl.configure(
                        text=f"Selected: @{u_name}",
                        fg=c("accent"),
                    )
                    # Highlight selected
                    for child in results_frame.winfo_children():
                        if isinstance(child, tk.Frame):
                            child.configure(bg=c("bg_secondary"))
                    row.configure(bg=c("accent_light"))

                for w2 in [row, inner, av, nm]:
                    w2.bind("<Button-1>", _select)

                def _enter(e, r=row): r.configure(bg=c("bg_tertiary"))
                def _leave(e, r=row): r.configure(bg=c("bg_secondary"))
                row.bind("<Enter>", _enter)
                row.bind("<Leave>", _leave)

        def _do_search(*_):
            q = search_entry.get().strip()
            if not q:
                return
            status_lbl.configure(text="Searching…", fg=c("fg_tertiary"))

            def t():
                try:
                    r = self.api.search_users(q)
                    payload = self._unwrap(r)
                    if isinstance(payload, dict):
                        users = payload.get("users") or payload.get("results") or []
                    elif isinstance(payload, list):
                        users = payload
                    else:
                        users = []
                    self.root.after(0, lambda: _render_results(users))
                    self.root.after(0, lambda: status_lbl.configure(text=""))
                except Exception as e:
                    self.root.after(0, lambda: status_lbl.configure(
                        text=f"Search error: {e}", fg=c("danger")))
            threading.Thread(target=t, daemon=True).start()

        search_entry.bind("<Return>", _do_search)
        search_entry.bind("<KP_Enter>", _do_search)

        search_btn = make_btn(search_row, "Search", _do_search, self,
                              variant="primary", size="sm")
        search_btn.pack(side=tk.LEFT)

        # Footer
        Separator(modal.winfo_children()[0].winfo_children()[0], c("border"))
        footer = tk.Frame(modal.winfo_children()[0].winfo_children()[0],
                          bg=c("bg_secondary"))
        footer.pack(fill=tk.X, padx=1, pady=1, side=tk.BOTTOM)
        footer_inner = tk.Frame(footer, bg=c("bg_secondary"))
        footer_inner.pack(fill=tk.X, padx=SP[5], pady=SP[4])

        def _start_chat():
            uid = selected_user["id"]
            if not uid:
                status_lbl.configure(text="Please select a user first.", fg=c("warning"))
                return
            status_lbl.configure(text="Creating chat…", fg=c("fg_tertiary"))

            def t():
                try:
                    r = self.api.create_chat(uid)
                    if r.get("success"):
                        payload = self._unwrap(r)
                        new_chat = self._normalise_chat(payload if payload else {})
                        self.root.after(0, lambda: modal.destroy())
                        self.root.after(0, lambda: self.load_conversations())
                        if new_chat.get("id"):
                            self.root.after(100, lambda: self.select_conversation(new_chat))
                    else:
                        err = self._unwrap(r)
                        msg = (err.get("message") if isinstance(err, dict) else str(err)) or "Failed"
                        self.root.after(0, lambda: status_lbl.configure(
                            text=f"Error: {msg}", fg=c("danger")))
                except Exception as e:
                    self.root.after(0, lambda: status_lbl.configure(
                        text=f"Error: {e}", fg=c("danger")))
            threading.Thread(target=t, daemon=True).start()

        make_btn(footer_inner, "Cancel", modal.destroy, self,
                 variant="ghost", size="sm").pack(side=tk.RIGHT, padx=(SP[2], 0))
        make_btn(footer_inner, "Start Chat", _start_chat, self,
                 variant="primary", size="sm").pack(side=tk.RIGHT)

    def _show_new_group_modal(self):
        """
        New Group Chat — name the group, search & pick members, create it.

        POST /api/groups  {name: str, member_ids: [int, ...]}
        GET  /api/users/search?q=<query>
        """
        modal, content = self._make_modal("New Group Chat", width=460, height=480)
        c = self.get_color

        # Group name
        make_label(content, "Group Name", self, style="secondary", size="sm",
                   bold=True, bg=c("bg_primary")).pack(anchor=tk.W, pady=(0, SP[1]))
        name_bdr = tk.Frame(content, bg=c("input_border"), padx=1, pady=1)
        name_bdr.pack(fill=tk.X, pady=(0, SP[4]))
        name_entry = tk.Entry(name_bdr, bg=c("input_bg"), fg=c("fg_primary"),
                              insertbackground=c("accent"),
                              relief=tk.FLAT, bd=0,
                              font=(FONT_SANS, FS["base"]),
                              highlightthickness=0)
        name_entry.pack(fill=tk.X, ipady=SP[2], ipadx=SP[3])
        name_entry.focus_set()

        # Member search
        make_label(content, "Add Members", self, style="secondary", size="sm",
                   bold=True, bg=c("bg_primary")).pack(anchor=tk.W, pady=(0, SP[1]))

        search_row = tk.Frame(content, bg=c("bg_primary"))
        search_row.pack(fill=tk.X, pady=(0, SP[2]))
        search_bdr = tk.Frame(search_row, bg=c("input_border"), padx=1, pady=1)
        search_bdr.pack(side=tk.LEFT, fill=tk.X, expand=True, padx=(0, SP[2]))
        search_entry = tk.Entry(search_bdr, bg=c("input_bg"), fg=c("fg_primary"),
                                insertbackground=c("accent"),
                                relief=tk.FLAT, bd=0,
                                font=(FONT_SANS, FS["base"]),
                                highlightthickness=0)
        search_entry.pack(fill=tk.X, ipady=SP[2], ipadx=SP[3])

        # Search results (compact)
        res_outer = tk.Frame(content, bg=c("border"))
        res_outer.pack(fill=tk.X, pady=(0, SP[3]))
        res_inner = tk.Frame(res_outer, bg=c("bg_secondary"))
        res_inner.pack(fill=tk.X, padx=1, pady=1)
        res_canvas = tk.Canvas(res_inner, bg=c("bg_secondary"),
                               highlightthickness=0, height=90)
        res_sb = ttk.Scrollbar(res_inner, orient="vertical", command=res_canvas.yview)
        res_frame = tk.Frame(res_canvas, bg=c("bg_secondary"))
        res_frame.bind("<Configure>",
                       lambda e: res_canvas.configure(scrollregion=res_canvas.bbox("all")))
        res_canvas.create_window((0, 0), window=res_frame, anchor="nw")
        res_canvas.configure(yscrollcommand=res_sb.set)
        res_sb.pack(side=tk.RIGHT, fill=tk.Y)
        res_canvas.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

        # Selected members chips area
        make_label(content, "Selected Members", self, style="secondary", size="xs",
                   bg=c("bg_primary")).pack(anchor=tk.W, pady=(0, SP[1]))
        chips_outer = tk.Frame(content, bg=c("border"))
        chips_outer.pack(fill=tk.X, pady=(0, SP[3]))
        chips_inner = tk.Frame(chips_outer, bg=c("bg_secondary"), pady=SP[2], padx=SP[2])
        chips_inner.pack(fill=tk.X, padx=1, pady=1)

        status_lbl = make_label(content, "", self, style="tertiary",
                                size="xs", bg=c("bg_primary"))
        status_lbl.pack(anchor=tk.W)

        selected_members = {}  # id -> username

        def _refresh_chips():
            for w in chips_inner.winfo_children():
                w.destroy()
            if not selected_members:
                make_label(chips_inner, "No members selected yet", self,
                           style="tertiary", size="xs",
                           bg=c("bg_secondary")).pack(padx=SP[2], pady=SP[1])
                return
            for uid, uname in list(selected_members.items()):
                chip = tk.Frame(chips_inner, bg=c("accent_light"))
                chip.pack(side=tk.LEFT, padx=(0, SP[1]), pady=SP[1])
                tk.Label(chip, text=f"@{uname}",
                         bg=c("accent_light"), fg=c("accent"),
                         font=(FONT_SANS, FS["xs"]),
                         padx=SP[2], pady=1).pack(side=tk.LEFT)
                def _remove(e=None, u=uid):
                    selected_members.pop(u, None)
                    _refresh_chips()
                tk.Button(chip, text="✕", command=lambda u=uid: [
                    selected_members.pop(u, None), _refresh_chips()],
                    bg=c("accent_light"), fg=c("accent"),
                    relief=tk.FLAT, bd=0, cursor="hand2",
                    font=(FONT_SANS, FS["xs"]),
                    highlightthickness=0).pack(side=tk.LEFT)

        _refresh_chips()

        def _render_search_results(users):
            for w in res_frame.winfo_children():
                w.destroy()
            if not users:
                make_label(res_frame, "No users found", self,
                           style="tertiary", size="xs",
                           bg=c("bg_secondary")).pack(padx=SP[3], pady=SP[2])
                return
            for u in users[:8]:
                uid = u.get("id")
                uname = u.get("username") or u.get("name") or str(uid)
                already = uid in selected_members
                row = tk.Frame(res_frame, bg=c("bg_secondary"), cursor="hand2")
                row.pack(fill=tk.X)
                inner = tk.Frame(row, bg=c("bg_secondary"))
                inner.pack(fill=tk.X, padx=SP[2], pady=2)
                tk.Label(inner, text=uname[0].upper(),
                         bg=c("accent_light"), fg=c("accent"),
                         font=(FONT_SANS, FS["xs"], "bold"), width=2
                         ).pack(side=tk.LEFT, padx=(0, SP[2]))
                make_label(inner, uname, self, style="primary",
                           size="xs", bg=c("bg_secondary")).pack(side=tk.LEFT)
                if already:
                    tk.Label(inner, text="✓", bg=c("bg_secondary"),
                             fg=c("success"),
                             font=(FONT_SANS, FS["xs"])).pack(side=tk.RIGHT, padx=SP[2])
                Separator(res_frame, c("border_light"))

                def _toggle(e=None, u_id=uid, u_name=uname):
                    if u_id in selected_members:
                        selected_members.pop(u_id)
                    else:
                        selected_members[u_id] = u_name
                    _refresh_chips()
                    # Re-render results to update checkmarks
                    _render_search_results(users)

                for w2 in [row, inner]:
                    w2.bind("<Button-1>", _toggle)

                def _e(e, r=row): r.configure(bg=c("bg_tertiary"))
                def _l(e, r=row): r.configure(bg=c("bg_secondary"))
                row.bind("<Enter>", _e)
                row.bind("<Leave>", _l)

        def _do_search(*_):
            q = search_entry.get().strip()
            if not q:
                return
            def t():
                try:
                    r = self.api.search_users(q)
                    payload = self._unwrap(r)
                    if isinstance(payload, dict):
                        users = payload.get("users") or payload.get("results") or []
                    elif isinstance(payload, list):
                        users = payload
                    else:
                        users = []
                    self.root.after(0, lambda: _render_search_results(users))
                except Exception as e:
                    self.root.after(0, lambda: status_lbl.configure(
                        text=f"Search error: {e}", fg=c("danger")))
            threading.Thread(target=t, daemon=True).start()

        search_entry.bind("<Return>", _do_search)
        search_entry.bind("<KP_Enter>", _do_search)
        make_btn(search_row, "Search", _do_search, self,
                 variant="primary", size="sm").pack(side=tk.LEFT)

        # Footer
        card_widget = modal.winfo_children()[0].winfo_children()[0]
        Separator(card_widget, c("border"))
        footer = tk.Frame(card_widget, bg=c("bg_secondary"))
        footer.pack(fill=tk.X, padx=1, pady=1, side=tk.BOTTOM)
        footer_inner = tk.Frame(footer, bg=c("bg_secondary"))
        footer_inner.pack(fill=tk.X, padx=SP[5], pady=SP[4])

        def _create_group():
            gname = name_entry.get().strip()
            if not gname:
                status_lbl.configure(text="Please enter a group name.", fg=c("warning"))
                return
            member_ids = list(selected_members.keys())
            status_lbl.configure(text="Creating group…", fg=c("fg_tertiary"))

            def t():
                try:
                    r = self.api.create_group(gname, member_ids)
                    if r.get("success"):
                        payload = self._unwrap(r)
                        new_chat = self._normalise_chat(payload if payload else {})
                        self.root.after(0, lambda: modal.destroy())
                        self.root.after(0, lambda: self.load_conversations())
                        if new_chat.get("id"):
                            self.root.after(100, lambda: self.select_conversation(new_chat))
                    else:
                        err = self._unwrap(r)
                        msg = (err.get("message") if isinstance(err, dict) else str(err)) or "Failed"
                        self.root.after(0, lambda: status_lbl.configure(
                            text=f"Error: {msg}", fg=c("danger")))
                except Exception as e:
                    self.root.after(0, lambda: status_lbl.configure(
                        text=f"Error: {e}", fg=c("danger")))
            threading.Thread(target=t, daemon=True).start()

        make_btn(footer_inner, "Cancel", modal.destroy, self,
                 variant="ghost", size="sm").pack(side=tk.RIGHT, padx=(SP[2], 0))
        make_btn(footer_inner, "Create Group", _create_group, self,
                 variant="primary", size="sm").pack(side=tk.RIGHT)

    # ─────────────────────────────────────────────────────────────────────────
    #  Settings screen
    # ─────────────────────────────────────────────────────────────────────────

    def show_settings_screen(self):
        for w in self.settings_frame.winfo_children():
            w.destroy()
        c = self.get_color
        self.settings_frame.configure(bg=c("bg_primary"))

        # Settings layout: 280px sidebar + content (mirrors settings.light.css)
        container = tk.Frame(self.settings_frame, bg=c("bg_primary"))
        container.pack(fill=tk.BOTH, expand=True)

        # Settings sidebar
        s_sidebar = tk.Frame(container, bg=c("bg_secondary"), width=280)
        s_sidebar.pack(side=tk.LEFT, fill=tk.Y)
        s_sidebar.pack_propagate(False)

        # Section label
        sec_lbl = tk.Label(s_sidebar, text="SETTINGS",
                           font=(FONT_SANS, FS["xs"], "bold"),
                           bg=c("bg_secondary"), fg=c("fg_tertiary"),
                           padx=SP[6])
        sec_lbl.pack(anchor=tk.W, pady=(SP[6], SP[2]))

        nav_items = [
            ("👤  Account", "account"),
            ("🎨  Appearance", "appearance"),
            ("🔐  Session", "session"),
        ]
        self._settings_section = tk.StringVar(value="account")
        self._settings_panels = {}

        # Content area
        tk.Frame(container, bg=c("border"), width=1).pack(side=tk.LEFT, fill=tk.Y)
        content_scroll_outer = tk.Frame(container, bg=c("bg_primary"))
        content_scroll_outer.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)
        _, content = make_scrollable(content_scroll_outer, c("bg_primary"))

        def show_section(key):
            self._settings_section.set(key)
            for k, panel in self._settings_panels.items():
                panel.pack_forget()
            self._settings_panels[key].pack(fill=tk.BOTH, expand=True,
                                             padx=SP[8], pady=SP[8])
            # Update nav highlight
            for nm, k2, btn in nav_btns:
                if k2 == key:
                    btn.configure(fg=c("accent"), bg=c("accent_light"))
                else:
                    btn.configure(fg=c("fg_secondary"), bg=c("bg_secondary"))

        nav_btns = []
        for label, key in nav_items:
            btn = tk.Button(
                s_sidebar, text=label,
                command=lambda k=key: show_section(k),
                bg=c("bg_secondary"), fg=c("fg_secondary"),
                relief=tk.FLAT, bd=0, highlightthickness=0,
                font=(FONT_SANS, FS["base"]), cursor="hand2",
                anchor=tk.W, padx=SP[6], pady=SP[3],
                activebackground=c("bg_tertiary"),
            )
            btn.pack(fill=tk.X)
            nav_btns.append((label, key, btn))

        Separator(s_sidebar, c("border"))

        # ── Account panel ─────────────────────────────────────────────────
        acc_panel = tk.Frame(content, bg=c("bg_primary"))
        self._settings_panels["account"] = acc_panel

        make_label(acc_panel, "Account", self, style="primary",
                   size="3xl", bold=True, bg=c("bg_primary")).pack(anchor=tk.W)
        make_label(acc_panel, "Manage your account information", self,
                   style="secondary", size="sm", bg=c("bg_primary")).pack(anchor=tk.W, pady=(SP[1], SP[6]))

        # Card
        acc_card_bdr = tk.Frame(acc_panel, bg=c("border"))
        acc_card_bdr.pack(fill=tk.X, pady=(0, SP[4]))
        acc_card = tk.Frame(acc_card_bdr, bg=c("card_bg"), padx=SP[6], pady=SP[6])
        acc_card.pack(fill=tk.X, padx=1, pady=1)

        make_label(acc_card, "Profile", self, style="primary",
                   size="xl", bold=True, bg=c("card_bg")).pack(anchor=tk.W, pady=(0, SP[2]))
        Separator(acc_card, c("border"))

        if self.current_user:
            fields = [
                ("Full Name", self.current_user.get("full_name", "N/A")),
                ("Username",  f"@{self.current_user.get('username', 'N/A')}"),
                ("Email",     self.current_user.get("email", "N/A")),
                ("User ID",   str(self.current_user.get("id", "N/A"))),
                ("Role",      "Administrator 👑" if self.is_admin else "Member"),
            ]
            for lbl, val in fields:
                row = tk.Frame(acc_card, bg=c("card_bg"))
                row.pack(fill=tk.X, pady=SP[2])
                make_label(row, lbl, self, style="secondary",
                           size="sm", bg=c("card_bg")).pack(side=tk.LEFT, anchor=tk.W)
                make_label(row, val, self, style="primary",
                           size="sm", bold=True, bg=c("card_bg")).pack(side=tk.RIGHT, anchor=tk.E)

        # ── Appearance panel ──────────────────────────────────────────────
        app_panel = tk.Frame(content, bg=c("bg_primary"))
        self._settings_panels["appearance"] = app_panel

        make_label(app_panel, "Appearance", self, style="primary",
                   size="3xl", bold=True, bg=c("bg_primary")).pack(anchor=tk.W)
        make_label(app_panel, "Customize how Chat App looks", self,
                   style="secondary", size="sm", bg=c("bg_primary")).pack(anchor=tk.W, pady=(SP[1], SP[6]))

        theme_card_bdr = tk.Frame(app_panel, bg=c("border"))
        theme_card_bdr.pack(fill=tk.X)
        theme_card = tk.Frame(theme_card_bdr, bg=c("card_bg"), padx=SP[6], pady=SP[6])
        theme_card.pack(fill=tk.X, padx=1, pady=1)

        make_label(theme_card, "Theme", self, style="primary",
                   size="xl", bold=True, bg=c("card_bg")).pack(anchor=tk.W, pady=(0, SP[2]))
        make_label(theme_card, "Choose between light and dark mode", self,
                   style="secondary", size="sm", bg=c("card_bg")).pack(anchor=tk.W, pady=(0, SP[4]))
        Separator(theme_card, c("border"))

        cur = self.theme.theme.capitalize()
        make_label(theme_card, f"Current theme: {cur}", self,
                   style="secondary", size="sm", bg=c("card_bg")).pack(anchor=tk.W, pady=SP[4])

        make_btn(theme_card,
                 f"Switch to {'Dark 🌙' if self.theme.theme == 'light' else 'Light ☀️'} Mode",
                 self._toggle_theme, self, variant="primary", size="md"
                 ).pack(anchor=tk.W)

        # ── Session panel ─────────────────────────────────────────────────
        sess_panel = tk.Frame(content, bg=c("bg_primary"))
        self._settings_panels["session"] = sess_panel

        make_label(sess_panel, "Session", self, style="primary",
                   size="3xl", bold=True, bg=c("bg_primary")).pack(anchor=tk.W)
        make_label(sess_panel, "Manage your active sessions", self,
                   style="secondary", size="sm", bg=c("bg_primary")).pack(anchor=tk.W, pady=(SP[1], SP[6]))

        sess_card_bdr = tk.Frame(sess_panel, bg=c("border"))
        sess_card_bdr.pack(fill=tk.X, pady=(0, SP[4]))
        sess_card = tk.Frame(sess_card_bdr, bg=c("card_bg"), padx=SP[6], pady=SP[6])
        sess_card.pack(fill=tk.X, padx=1, pady=1)

        make_label(sess_card, "Current Session", self, style="primary",
                   size="xl", bold=True, bg=c("card_bg")).pack(anchor=tk.W, pady=(0, SP[2]))
        Separator(sess_card, c("border"))

        sess_inner = tk.Frame(sess_card, bg=c("bg_tertiary"))
        sess_inner.pack(fill=tk.X, pady=SP[4], padx=0)
        sess_inner2 = tk.Frame(sess_inner, bg=c("bg_tertiary"), padx=SP[4], pady=SP[4])
        sess_inner2.pack(fill=tk.X, padx=1, pady=1)

        if self.current_user:
            make_label(sess_inner2,
                       f"Logged in as @{self.current_user.get('username', '')}",
                       self, style="primary", size="base", bold=True,
                       bg=c("bg_tertiary")).pack(anchor=tk.W)
            make_label(sess_inner2, "This device · Active now", self,
                       style="secondary", size="xs", bg=c("bg_tertiary")).pack(anchor=tk.W)

        # Danger zone
        dz_bdr = tk.Frame(sess_panel, bg=c("danger"))
        dz_bdr.pack(fill=tk.X)
        dz = tk.Frame(dz_bdr, bg=c("danger_light"), padx=SP[6], pady=SP[6])
        dz.pack(fill=tk.X, padx=1, pady=1)

        make_label(dz, "Danger Zone", self, style="danger",
                   size="xl", bold=True, bg=c("danger_light")).pack(anchor=tk.W, pady=(0, SP[2]))
        make_label(dz, "This action will end your session immediately.", self,
                   style="secondary", size="sm", bg=c("danger_light")).pack(anchor=tk.W, pady=(0, SP[4]))

        make_btn(dz, "🚪  Sign Out", self.logout, self,
                 variant="danger", size="md").pack(anchor=tk.W)

        # Default section
        show_section("account")
        # Highlight first nav
        for _, k, btn in nav_btns:
            if k == "account":
                btn.configure(fg=c("accent"), bg=c("accent_light"))
                break

    # ─────────────────────────────────────────────────────────────────────────
    #  Admin screen
    # ─────────────────────────────────────────────────────────────────────────

    def show_admin_screen(self):
        if not self.is_admin:
            messagebox.showerror("Error", "Admin access required")
            return

        for w in self.admin_frame.winfo_children():
            w.destroy()
        c = self.get_color
        self.admin_frame.configure(bg=c("bg_primary"))

        _, scroll = make_scrollable(self.admin_frame, c("bg_primary"))
        inner = tk.Frame(scroll, bg=c("bg_primary"))
        inner.pack(fill=tk.BOTH, expand=True, padx=SP[8], pady=SP[8])

        make_label(inner, "Admin Panel 👑", self, style="accent",
                   size="3xl", bold=True, bg=c("bg_primary")).pack(anchor=tk.W)
        make_label(inner, "Server management and statistics", self,
                   style="secondary", size="sm", bg=c("bg_primary")).pack(anchor=tk.W, pady=(SP[1], SP[6]))

        # Stats card
        def _make_card(parent, title, subtitle):
            bdr = tk.Frame(parent, bg=c("border"))
            bdr.pack(fill=tk.X, pady=(0, SP[4]))
            card = tk.Frame(bdr, bg=c("card_bg"), padx=SP[6], pady=SP[6])
            card.pack(fill=tk.X, padx=1, pady=1)
            make_label(card, title, self, style="primary",
                       size="xl", bold=True, bg=c("card_bg")).pack(anchor=tk.W)
            make_label(card, subtitle, self, style="secondary",
                       size="sm", bg=c("card_bg")).pack(anchor=tk.W, pady=(SP[1], SP[4]))
            Separator(card, c("border"))
            return card

        # Server stats
        stats_card = _make_card(inner, "Server Statistics",
                                "Live metrics from the server")

        self.admin_stats_label = make_label(stats_card, "Click 'Load Stats' to fetch data",
                                            self, style="tertiary", size="sm",
                                            bg=c("card_bg"), justify=tk.LEFT)
        self.admin_stats_label.pack(anchor=tk.W, pady=SP[4])

        make_btn(stats_card, "📊  Load Stats", self.load_admin_stats, self,
                 variant="primary", size="md").pack(anchor=tk.W, pady=(SP[3], 0))

        # Users card
        users_card = _make_card(inner, "User Management",
                                "Browse and manage registered users")

        self.admin_users_label = make_label(users_card, "Click 'Load Users' to fetch data",
                                            self, style="tertiary", size="sm",
                                            bg=c("card_bg"), justify=tk.LEFT)
        self.admin_users_label.pack(anchor=tk.W, pady=SP[4])

        make_btn(users_card, "👥  Load Users", self.load_admin_users, self,
                 variant="primary", size="md").pack(anchor=tk.W, pady=(SP[3], 0))

    def load_admin_stats(self):
        def t():
            try:
                r = self.api.get_admin_stats()
                self.root.after(0, lambda: self._display_admin_stats(r))
            except Exception as e:
                self.logger.exception("Error loading admin stats", str(e), stop=False)
        threading.Thread(target=t, daemon=True).start()

    def _display_admin_stats(self, response):
        if not response.get("success"):
            self.admin_stats_label.config(text="⚠ Failed to load stats")
            return
        try:
            data = json.loads(response["data"])
            txt = (
                f"Total Users:         {data.get('total_users', 'N/A')}\n"
                f"Active Users:        {data.get('active_users', 'N/A')}\n"
                f"Total Messages:      {data.get('total_messages', 'N/A')}\n"
                f"Total Conversations: {data.get('total_conversations', 'N/A')}"
            )
            self.admin_stats_label.config(
                text=txt,
                font=(FONT_MONO, FS["sm"]),
                fg=self.get_color("fg_primary"),
            )
        except Exception as e:
            self.admin_stats_label.config(text=f"⚠ Error: {e}")

    def load_admin_users(self):
        def t():
            try:
                r = self.api.get_admin_users()
                self.root.after(0, lambda: self._display_admin_users(r))
            except Exception as e:
                self.logger.exception("Error loading admin users", str(e), stop=False)
        threading.Thread(target=t, daemon=True).start()

    def _display_admin_users(self, response):
        if not response.get("success"):
            self.admin_users_label.config(text="⚠ Failed to load users")
            return
        try:
            data = json.loads(response["data"])
            users = data.get("users", [])
            lines = [f"Total: {len(users)} users\n"]
            for u in users[:10]:
                lines.append(f"  {u.get('username')}  ·  {u.get('email')}")
            if len(users) > 10:
                lines.append(f"\n  … and {len(users) - 10} more")
            self.admin_users_label.config(
                text="\n".join(lines),
                font=(FONT_MONO, FS["sm"]),
                fg=self.get_color("fg_primary"),
            )
        except Exception as e:
            self.admin_users_label.config(text=f"⚠ Error: {e}")

    # ─────────────────────────────────────────────────────────────────────────
    #  Logout
    # ─────────────────────────────────────────────────────────────────────────

    def logout(self):
        self.current_user = None
        self.is_admin = False
        self.current_chat_id = None
        try:
            self.api.cookie_jar.clear()
            self.api.message_cache.clear_all()
        except Exception:
            pass
        self.navbar.set_user("")
        self.tab_bar.set_enabled("Chat", False)
        self.tab_bar.set_enabled("Settings", False)
        self.tab_bar.set_enabled("Admin", False)
        self.show_login_screen()
        self.tab_bar.select("Login")


# ─────────────────────────────────────────────────────────────────────────────

def main():
    root = tk.Tk()
    app = ChatClientApp(root)
    root.mainloop()


if __name__ == "__main__":
    main()
