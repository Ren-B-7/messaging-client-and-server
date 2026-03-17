"""
Chat Client Configuration - Aligned with Web UI Design
Colors and dimensions extracted from Rust server web interface
"""

# ============================================================================
# Server Configuration
# ============================================================================

DEFAULT_SERVER = "http://localhost:1337"
USER_AGENT = "ChatClient/2.0 (Python-Advanced)"

# ============================================================================
# Layout Dimensions (from web UI)
# ============================================================================

LAYOUT = {
    "window_width": 1200,
    "window_height": 800,
    "window_minsize_width": 900,
    "window_minsize_height": 600,
    
    # Header/Navbar
    "navbar_height": 64,
    
    # Sidebar
    "sidebar_width": 260,
    "sidebar_width_mobile": 200,
    "sidebar_width_collapsed": 60,
    
    # Chat
    "chat_input_height": 80,
    "message_avatar_size": 40,
}

# ============================================================================
# Spacing Scale (in pixels - from web var.css)
# ============================================================================

SPACING = {
    1: 4,      # 4px
    2: 8,      # 8px
    3: 12,     # 12px
    4: 16,     # 16px
    5: 20,     # 20px
    6: 24,     # 24px
    8: 32,     # 32px
    10: 40,    # 40px
    12: 48,    # 48px
    16: 64,    # 64px
}

# ============================================================================
# Border Radius (in pixels - from web var.css)
# ============================================================================

RADIUS = {
    "sm": 4,       # 4px
    "md": 8,       # 8px
    "lg": 12,      # 12px
    "xl": 16,      # 16px
    "2xl": 24,     # 24px
    "full": 9999,  # 9999px (circles)
}

# ============================================================================
# Font Configuration
# ============================================================================

FONTS = {
    "sans": ("Segoe UI", "Helvetica Neue", "Arial"),
    "mono": ("Courier New", "Monaco", "Consolas"),
    "size": {
        "xs": 10,    # 12px / 0.75rem
        "sm": 11,    # 14px / 0.875rem
        "base": 12,  # 16px / 1rem (tkinter uses points, not pixels)
        "lg": 13,    # 18px / 1.125rem
        "xl": 14,    # 20px / 1.25rem
        "2xl": 16,   # 24px / 1.5rem
        "3xl": 18,   # 30px / 1.875rem
        "4xl": 20,   # 36px / 2.25rem
    }
}

# ============================================================================
# Color Theme System - DARK MODE (default, matches web UI exactly)
# ============================================================================

COLORS = {
    # ========================================================================
    # DARK THEME (PRIMARY - extracted from base_dark.css)
    # ========================================================================
    "dark": {
        # ── Background Colors ──
        "bg_primary": "#0f1419",      # Main background
        "bg_secondary": "#1c2127",    # Cards, input fields
        "bg_tertiary": "#2d333b",     # Hover states
        "bg_elevated": "#1c2127",     # Elevated surfaces
        
        # ── Text/Foreground Colors ──
        "fg_primary": "#e6edf3",      # Primary text
        "fg_secondary": "#adb5bd",    # Secondary text
        "fg_tertiary": "#6c757d",     # Tertiary text
        "fg_muted": "#3a3f46",        # Muted text
        
        # ── Brand/Accent Colors ──
        "accent": "#3b8ff3",          # Primary accent (bright blue)
        "accent_hover": "#5ca3f5",    # Accent on hover
        "accent_light": "#1a3a5c",    # Light accent background
        "accent_dark": "#1e6fcc",     # Dark accent variant
        
        # ── Semantic Colors ──
        "success": "#3fb950",         # Success/positive
        "success_light": "#1a3f2b",   # Success background
        "danger": "#f85149",          # Danger/error
        "danger_light": "#4c1f1f",    # Danger background
        "warning": "#d29922",         # Warning
        "warning_light": "#3d2f1a",   # Warning background
        "info": "#58a6ff",            # Info
        "info_light": "#1a3f5c",      # Info background
        
        # ── Border Colors ──
        "border": "#3a3f46",          # Primary border
        "border_light": "#2d333b",    # Light border
        "border_dark": "#6c757d",     # Dark border
        
        # ── Interactive States ──
        "hover": "rgba(255, 255, 255, 0.05)",    # Hover overlay
        "active": "rgba(255, 255, 255, 0.1)",    # Active overlay
        "focus_ring": "rgba(59, 143, 243, 0.25)", # Focus indicator
        
        # ── Component Specific ──
        "input_bg": "#1c2127",
        "input_border": "#3a3f46",
        "input_focus_border": "#3b8ff3",
        
        "button_primary_bg": "#3b8ff3",
        "button_primary_color": "#ffffff",
        "button_primary_hover": "#5ca3f5",
        
        "card_bg": "#1c2127",
        "card_border": "#3a3f46",
        
        "overlay": "rgba(0, 0, 0, 0.7)",
        "backdrop": "rgba(0, 0, 0, 0.5)",
    },
    
    # ========================================================================
    # LIGHT THEME (ALTERNATIVE - extracted from base_light.css)
    # ========================================================================
    "light": {
        # ── Background Colors ──
        "bg_primary": "#ffffff",      # Main background
        "bg_secondary": "#f8f9fa",    # Cards, input fields
        "bg_tertiary": "#e9ecef",     # Hover states
        "bg_elevated": "#ffffff",     # Elevated surfaces
        
        # ── Text/Foreground Colors ──
        "fg_primary": "#212529",      # Primary text
        "fg_secondary": "#6c757d",    # Secondary text
        "fg_tertiary": "#adb5bd",     # Tertiary text
        "fg_muted": "#dee2e6",        # Muted text
        
        # ── Brand/Accent Colors ──
        "accent": "#0066cc",          # Primary accent (blue)
        "accent_hover": "#0052a3",    # Accent on hover
        "accent_light": "#e6f0ff",    # Light accent background
        "accent_dark": "#004080",     # Dark accent variant
        
        # ── Semantic Colors ──
        "success": "#28a745",         # Success/positive
        "success_light": "#d4edda",   # Success background
        "danger": "#dc3545",          # Danger/error
        "danger_light": "#f8d7da",    # Danger background
        "warning": "#ffc107",         # Warning
        "warning_light": "#fff3cd",   # Warning background
        "info": "#17a2b8",            # Info
        "info_light": "#d1ecf1",      # Info background
        
        # ── Border Colors ──
        "border": "#dee2e6",          # Primary border
        "border_light": "#e9ecef",    # Light border
        "border_dark": "#adb5bd",     # Dark border
        
        # ── Interactive States ──
        "hover": "rgba(0, 0, 0, 0.05)",       # Hover overlay
        "active": "rgba(0, 0, 0, 0.1)",       # Active overlay
        "focus_ring": "rgba(0, 102, 204, 0.25)", # Focus indicator
        
        # ── Component Specific ──
        "input_bg": "#ffffff",
        "input_border": "#ced4da",
        "input_focus_border": "#0066cc",
        
        "button_primary_bg": "#0066cc",
        "button_primary_color": "#ffffff",
        "button_primary_hover": "#0052a3",
        
        "card_bg": "#ffffff",
        "card_border": "#dee2e6",
        
        "overlay": "rgba(0, 0, 0, 0.5)",
        "backdrop": "rgba(0, 0, 0, 0.25)",
    }
}

# ============================================================================
# Performance Configuration
# ============================================================================

MESSAGE_CACHE_LIMIT = 500
RECONNECT_TIMEOUT = 5
CONNECTION_TIMEOUT = 10

# ============================================================================
# Animation Durations (from web var.css)
# ============================================================================

TRANSITIONS = {
    "fast": 150,       # 150ms
    "base": 200,       # 200ms
    "slow": 300,       # 300ms
    "slower": 500,     # 500ms
}

# ============================================================================
# Shadow System (for cards and elevated elements)
# ============================================================================

SHADOWS = {
    "dark": {
        "sm": "0 1px 2px 0 rgba(0, 0, 0, 0.3)",
        "md": "0 4px 6px -1px rgba(0, 0, 0, 0.4), 0 2px 4px -1px rgba(0, 0, 0, 0.3)",
        "lg": "0 10px 15px -3px rgba(0, 0, 0, 0.5), 0 4px 6px -2px rgba(0, 0, 0, 0.4)",
        "xl": "0 20px 25px -5px rgba(0, 0, 0, 0.6), 0 10px 10px -5px rgba(0, 0, 0, 0.5)",
    },
    "light": {
        "sm": "0 1px 2px 0 rgba(0, 0, 0, 0.05)",
        "md": "0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -1px rgba(0, 0, 0, 0.06)",
        "lg": "0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -2px rgba(0, 0, 0, 0.05)",
        "xl": "0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 10px 10px -5px rgba(0, 0, 0, 0.04)",
    }
}

# ============================================================================
# Z-Index Scale
# ============================================================================

Z_INDEX = {
    "dropdown": 1000,
    "sticky": 1020,
    "fixed": 1030,
    "modal_backdrop": 1040,
    "modal": 1050,
    "popover": 1060,
    "tooltip": 1070,
}

# ============================================================================
# Responsive Breakpoints (in pixels)
# ============================================================================

BREAKPOINTS = {
    "sm": 640,      # Small devices
    "md": 768,      # Medium devices
    "lg": 1024,     # Large devices
    "xl": 1280,     # Extra large devices
    "2xl": 1536,    # 2x large devices
}

# ============================================================================
# Button Styles (pre-defined sizes)
# ============================================================================

BUTTON_SIZES = {
    "sm": {
        "padding": (SPACING[2], SPACING[4]),
        "font_size": "sm",
    },
    "md": {
        "padding": (SPACING[3], SPACING[6]),
        "font_size": "base",
    },
    "lg": {
        "padding": (SPACING[4], SPACING[8]),
        "font_size": "lg",
    },
}

# ============================================================================
# Avatar Sizes
# ============================================================================

AVATAR_SIZES = {
    "sm": 32,
    "md": 40,
    "lg": 56,
    "xl": 80,
}

# ============================================================================
# Input Field Configuration
# ============================================================================

INPUT_PADDING = (SPACING[3], SPACING[4])  # vertical, horizontal
INPUT_BORDER_WIDTH = 1
INPUT_CORNER_RADIUS = RADIUS["md"]

# ============================================================================
# Card Configuration
# ============================================================================

CARD_PADDING = SPACING[6]
CARD_BORDER_WIDTH = 1
CARD_CORNER_RADIUS = RADIUS["lg"]

# ============================================================================
# Utility Functions
# ============================================================================

def get_spacing(scale):
    """Get spacing value by scale (1-16)"""
    return SPACING.get(scale, SPACING[4])

def hex_to_rgb(hex_color):
    """Convert hex color to RGB tuple"""
    hex_color = hex_color.lstrip('#')
    if len(hex_color) == 6:
        return tuple(int(hex_color[i:i+2], 16) for i in (0, 2, 4))
    return (0, 0, 0)

def rgb_to_hex(r, g, b):
    """Convert RGB tuple to hex color"""
    return f"#{r:02x}{g:02x}{b:02x}"

# ============================================================================
# Help Text
# ============================================================================

"""
COLOR USAGE GUIDE:

For backgrounds:
  - bg_primary: Main page/window background
  - bg_secondary: Cards, input fields, secondary containers
  - bg_tertiary: Hover states, separator backgrounds
  - bg_elevated: Floating elements, modals

For text:
  - fg_primary: Main body text
  - fg_secondary: Secondary text, labels
  - fg_tertiary: Subtle text, placeholders
  - fg_muted: Very faint text, disabled states

For accents:
  - accent: Primary button, links, active states
  - accent_hover: Hover state for accent elements
  - accent_light: Light background for accent areas
  - accent_dark: Darker variant of accent

For semantic:
  - success: Green for success messages
  - danger: Red for errors/destructive actions
  - warning: Yellow/orange for warnings
  - info: Blue for informational messages

EXAMPLE USAGE in app.py:

    from config import COLORS
    
    # Get colors based on current theme
    theme_colors = COLORS["dark"]  # or "light"
    
    # Apply to tkinter widgets
    frame = tk.Frame(root, bg=theme_colors["bg_secondary"])
    button = tk.Button(root, bg=theme_colors["button_primary_bg"],
                              fg=theme_colors["button_primary_color"])
    input_field = tk.Entry(root, bg=theme_colors["input_bg"],
                                  fg=theme_colors["fg_primary"],
                                  insertbackground=theme_colors["accent"])
"""
