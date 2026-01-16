"""Floating indicator window using tkinter with premium UI."""

import threading
import tkinter as tk
from enum import Enum
from typing import Callable, List, Optional

from config import get_window_position, set_window_position


class Status(Enum):
    """Application status states."""

    LOADING = "Loading Model"
    READY = "Ready"
    RECORDING = "Recording"
    TRANSCRIBING = "Transcribing"


# Premium color scheme
COLORS = {
    "bg_dark": "#1a1a1a",
    "bg_card": "#252525",
    "bg_hover": "#2d2d2d",
    "border": "#3a3a3a",
    "border_light": "#454545",
    "text_primary": "#ffffff",
    "text_secondary": "#888888",
    "text_muted": "#666666",
    "accent_gray": "#6b6b6b",
    "accent_green": "#10b981",
    "accent_green_dim": "#065f46",
    "accent_red": "#ef4444",
    "accent_red_dim": "#7f1d1d",
    "accent_amber": "#f59e0b",
    "accent_amber_dim": "#78350f",
}

STATUS_COLORS = {
    Status.LOADING: (COLORS["accent_gray"], COLORS["text_muted"]),
    Status.READY: (COLORS["accent_green"], COLORS["accent_green_dim"]),
    Status.RECORDING: (COLORS["accent_red"], COLORS["accent_red_dim"]),
    Status.TRANSCRIBING: (COLORS["accent_amber"], COLORS["accent_amber_dim"]),
}


class FloatingWindow:
    """Floating status indicator window with premium UI."""

    WINDOW_WIDTH = 240
    WINDOW_HEIGHT = 72
    CORNER_RADIUS = 16
    INDICATOR_RADIUS = 6
    PULSE_INTERVAL = 600
    GLOW_STEPS = 3

    def __init__(
        self,
        on_settings: Optional[Callable[[], None]] = None,
        on_quit: Optional[Callable[[], None]] = None,
    ):
        """Initialize the floating window.

        Args:
            on_settings: Callback when Settings is selected from menu.
            on_quit: Callback when Quit is selected from menu.
        """
        self.on_settings = on_settings
        self.on_quit = on_quit

        self._status = Status.LOADING
        self._hotkey: List[str] = ["Ctrl", "Shift", "Space"]
        self._mode: str = "push_to_talk"
        self._root: Optional[tk.Tk] = None
        self._canvas: Optional[tk.Canvas] = None
        self._pulse_visible = True
        self._pulse_job: Optional[str] = None
        self._glow_intensity = 0
        self._glow_direction = 1
        self._drag_data = {"x": 0, "y": 0}
        self._gui_thread: Optional[threading.Thread] = None
        self._running = False

    def start(self) -> None:
        """Start the GUI in a separate thread."""
        if self._running:
            return

        self._running = True
        self._gui_thread = threading.Thread(target=self._run_gui, daemon=True)
        self._gui_thread.start()

    def _run_gui(self) -> None:
        """Run the GUI main loop."""
        self._root = tk.Tk()
        self._setup_window()
        self._create_canvas()
        self._setup_bindings()
        self._draw_ui()

        try:
            self._root.mainloop()
        except Exception as e:
            print(f"GUI error: {e}")
        finally:
            self._running = False

    def _setup_window(self) -> None:
        """Configure the main window."""
        self._root.title("FreeFlow")
        self._root.overrideredirect(True)
        self._root.attributes("-topmost", True)
        self._root.geometry(f"{self.WINDOW_WIDTH}x{self.WINDOW_HEIGHT}")

        # Set position
        x, y = get_window_position()
        self._root.geometry(f"+{x}+{y}")

        # Transparent background for rounded corners effect
        self._root.configure(bg=COLORS["bg_dark"])

        # Windows-specific transparency
        try:
            self._root.attributes("-transparentcolor", COLORS["bg_dark"])
            self._root.attributes("-alpha", 0.98)
        except tk.TclError:
            pass

    def _create_canvas(self) -> None:
        """Create the main canvas for custom drawing."""
        self._canvas = tk.Canvas(
            self._root,
            width=self.WINDOW_WIDTH,
            height=self.WINDOW_HEIGHT,
            bg=COLORS["bg_dark"],
            highlightthickness=0,
        )
        self._canvas.pack(fill=tk.BOTH, expand=True)

    def _draw_rounded_rect(
        self,
        x1: int,
        y1: int,
        x2: int,
        y2: int,
        radius: int,
        fill: str,
        outline: str = "",
        outline_width: int = 1,
    ) -> int:
        """Draw a rounded rectangle on the canvas."""
        points = [
            x1 + radius, y1,
            x2 - radius, y1,
            x2, y1,
            x2, y1 + radius,
            x2, y2 - radius,
            x2, y2,
            x2 - radius, y2,
            x1 + radius, y2,
            x1, y2,
            x1, y2 - radius,
            x1, y1 + radius,
            x1, y1,
            x1 + radius, y1,
        ]
        return self._canvas.create_polygon(
            points,
            fill=fill,
            outline=outline,
            width=outline_width,
            smooth=True,
        )

    def _draw_ui(self) -> None:
        """Draw the complete UI."""
        if not self._canvas:
            return

        self._canvas.delete("all")

        # Draw background card with border
        self._draw_rounded_rect(
            1, 1,
            self.WINDOW_WIDTH - 1, self.WINDOW_HEIGHT - 1,
            self.CORNER_RADIUS,
            fill=COLORS["bg_card"],
            outline=COLORS["border"],
            outline_width=1,
        )

        # Inner subtle highlight at top
        self._draw_rounded_rect(
            2, 2,
            self.WINDOW_WIDTH - 2, self.WINDOW_HEIGHT // 3,
            self.CORNER_RADIUS - 1,
            fill=COLORS["bg_hover"],
            outline="",
        )

        # Re-draw main area to cover bottom of highlight
        self._draw_rounded_rect(
            2, 20,
            self.WINDOW_WIDTH - 2, self.WINDOW_HEIGHT - 2,
            self.CORNER_RADIUS - 2,
            fill=COLORS["bg_card"],
            outline="",
        )

        # Get status colors
        indicator_color, indicator_dim = STATUS_COLORS.get(
            self._status, (COLORS["accent_gray"], COLORS["text_muted"])
        )

        # Draw status indicator with glow effect
        indicator_x = 24
        indicator_y = 26

        # Glow layers (subtle)
        if self._status in (Status.RECORDING, Status.READY):
            for i in range(self.GLOW_STEPS, 0, -1):
                glow_radius = self.INDICATOR_RADIUS + i * 2
                alpha_color = indicator_dim if i > 1 else indicator_color
                self._canvas.create_oval(
                    indicator_x - glow_radius,
                    indicator_y - glow_radius,
                    indicator_x + glow_radius,
                    indicator_y + glow_radius,
                    fill=alpha_color if self._pulse_visible or self._status != Status.RECORDING else "",
                    outline="",
                )

        # Main indicator dot
        current_color = indicator_color if self._pulse_visible else indicator_dim
        self._canvas.create_oval(
            indicator_x - self.INDICATOR_RADIUS,
            indicator_y - self.INDICATOR_RADIUS,
            indicator_x + self.INDICATOR_RADIUS,
            indicator_y + self.INDICATOR_RADIUS,
            fill=current_color,
            outline="",
        )

        # Status text
        self._canvas.create_text(
            46, 26,
            text=self._status.value,
            font=("Segoe UI Semibold", 11),
            fill=COLORS["text_primary"],
            anchor="w",
        )

        # Hotkey display
        hotkey_str = self._format_hotkey()
        self._canvas.create_text(
            24, 52,
            text=hotkey_str,
            font=("Segoe UI", 9),
            fill=COLORS["text_secondary"],
            anchor="w",
        )

        # Mode indicator (small text)
        mode_text = "hold" if self._mode == "push_to_talk" else "toggle"
        self._canvas.create_text(
            self.WINDOW_WIDTH - 16, 52,
            text=mode_text,
            font=("Segoe UI", 8),
            fill=COLORS["text_muted"],
            anchor="e",
        )

    def _format_hotkey(self) -> str:
        """Format the hotkey for display."""
        formatted_keys = []
        for key in self._hotkey:
            key_lower = key.lower()
            if key_lower in ("ctrl", "ctrl_l", "ctrl_r"):
                formatted_keys.append("Ctrl")
            elif key_lower in ("shift", "shift_l", "shift_r"):
                formatted_keys.append("Shift")
            elif key_lower in ("alt", "alt_l", "alt_r"):
                formatted_keys.append("Alt")
            elif key_lower in ("cmd", "win"):
                formatted_keys.append("Win")
            elif key_lower == "space":
                formatted_keys.append("Space")
            else:
                formatted_keys.append(key.upper() if len(key) == 1 else key.title())

        return " + ".join(formatted_keys)

    def _setup_bindings(self) -> None:
        """Set up event bindings."""
        self._canvas.bind("<Button-1>", self._on_drag_start)
        self._canvas.bind("<B1-Motion>", self._on_drag_motion)
        self._canvas.bind("<ButtonRelease-1>", self._on_drag_end)
        self._canvas.bind("<Button-3>", self._show_context_menu)

    def _on_drag_start(self, event: tk.Event) -> None:
        """Handle drag start."""
        self._drag_data["x"] = event.x
        self._drag_data["y"] = event.y

    def _on_drag_motion(self, event: tk.Event) -> None:
        """Handle drag motion."""
        x = self._root.winfo_x() + (event.x - self._drag_data["x"])
        y = self._root.winfo_y() + (event.y - self._drag_data["y"])
        self._root.geometry(f"+{x}+{y}")

    def _on_drag_end(self, event: tk.Event) -> None:
        """Handle drag end - save position."""
        x = self._root.winfo_x()
        y = self._root.winfo_y()
        set_window_position(x, y)

    def _show_context_menu(self, event: tk.Event) -> None:
        """Show the right-click context menu."""
        menu = tk.Menu(
            self._root,
            tearoff=0,
            bg=COLORS["bg_card"],
            fg=COLORS["text_primary"],
            activebackground=COLORS["bg_hover"],
            activeforeground=COLORS["text_primary"],
            font=("Segoe UI", 9),
            relief=tk.FLAT,
            borderwidth=1,
        )
        menu.add_command(label="  Settings  ", command=self._on_settings_click)
        menu.add_separator()
        menu.add_command(label="  Quit  ", command=self._on_quit_click)

        try:
            menu.tk_popup(event.x_root, event.y_root)
        finally:
            menu.grab_release()

    def _on_settings_click(self) -> None:
        """Handle Settings menu click."""
        if self.on_settings:
            self.on_settings()

    def _on_quit_click(self) -> None:
        """Handle Quit menu click."""
        if self.on_quit:
            self.on_quit()
        self.stop()

    def set_status(self, status: Status) -> None:
        """Set the current status."""
        old_status = self._status
        self._status = status

        if self._root:
            # Handle pulse animation
            if status == Status.RECORDING and old_status != Status.RECORDING:
                self._root.after(0, self._start_pulse)
            elif status != Status.RECORDING and old_status == Status.RECORDING:
                self._stop_pulse()

            self._root.after(0, self._draw_ui)

    def set_hotkey(self, hotkey: List[str]) -> None:
        """Set the displayed hotkey."""
        self._hotkey = hotkey
        self._request_redraw()

    def set_mode(self, mode: str) -> None:
        """Set the activation mode display."""
        self._mode = mode
        self._request_redraw()

    def _request_redraw(self) -> None:
        """Request a UI redraw, handling cross-thread calls."""
        if self._root and self._canvas:
            try:
                self._draw_ui()
                self._canvas.update_idletasks()
            except Exception:
                # If direct call fails, schedule it
                self._root.after(0, self._draw_ui)

    def _start_pulse(self) -> None:
        """Start the pulse animation for recording."""
        if self._pulse_job is not None:
            return
        self._pulse_visible = True
        # Schedule first pulse via after() to avoid potential recursion
        if self._root:
            self._pulse_job = self._root.after(self.PULSE_INTERVAL, self._pulse)

    def _pulse(self) -> None:
        """Animate the indicator pulse."""
        if self._status != Status.RECORDING:
            self._stop_pulse()
            return

        self._pulse_visible = not self._pulse_visible

        # Redraw just the UI without triggering pulse logic again
        if self._canvas:
            self._draw_ui()

        if self._root and self._status == Status.RECORDING:
            self._pulse_job = self._root.after(self.PULSE_INTERVAL, self._pulse)

    def _stop_pulse(self) -> None:
        """Stop the pulse animation."""
        if self._pulse_job is not None and self._root:
            self._root.after_cancel(self._pulse_job)
            self._pulse_job = None
        self._pulse_visible = True

    def stop(self) -> None:
        """Stop the GUI."""
        self._running = False
        self._stop_pulse()

        if self._root:
            try:
                self._root.quit()
                self._root.destroy()
            except tk.TclError:
                pass
            self._root = None

    def is_running(self) -> bool:
        """Check if the GUI is running."""
        return self._running

    def get_root(self) -> Optional[tk.Tk]:
        """Get the root window for dialogs."""
        return self._root

    def run_on_gui_thread(self, func: Callable[[], None]) -> None:
        """Run a function on the GUI thread."""
        if self._root:
            self._root.after(0, func)
