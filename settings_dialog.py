"""Settings dialog for hotkey configuration with premium UI."""

import tkinter as tk
from tkinter import ttk
from typing import Callable, List, Optional

from audio_capture import AudioCapture
from config import (
    MODE_PUSH_TO_TALK,
    MODE_TOGGLE,
    get_activation_mode,
    get_audio_device,
    get_hotkey,
    set_activation_mode,
    set_audio_device,
    set_hotkey,
)
from hotkey_manager import HotkeyRecorder


# Premium color scheme (matching gui.py)
COLORS = {
    "bg_dark": "#1a1a1a",
    "bg_card": "#252525",
    "bg_input": "#2d2d2d",
    "bg_hover": "#353535",
    "border": "#3a3a3a",
    "text_primary": "#ffffff",
    "text_secondary": "#aaaaaa",
    "text_muted": "#666666",
    "accent": "#10b981",
    "accent_hover": "#059669",
}


class SettingsDialog:
    """Settings dialog for configuring FreeFlow."""

    def __init__(
        self,
        parent: Optional[tk.Tk],
        current_hotkey: List[str],
        current_mode: str = MODE_PUSH_TO_TALK,
        on_settings_changed: Optional[Callable[[List[str], str], None]] = None,
        on_close: Optional[Callable[[], None]] = None,
    ):
        """Initialize the settings dialog."""
        self.current_hotkey = current_hotkey.copy()
        self.current_mode = current_mode
        self.on_settings_changed = on_settings_changed
        self.on_close = on_close

        self._dialog: Optional[tk.Toplevel] = None
        self._hotkey_var: Optional[tk.StringVar] = None
        self._mode_var: Optional[tk.StringVar] = None
        self._recording = False
        self._recorder: Optional[HotkeyRecorder] = None
        self._record_button: Optional[tk.Button] = None
        self._device_var: Optional[tk.StringVar] = None
        self._devices: List[tuple] = []

        self._create_dialog(parent)

    def _create_dialog(self, parent: Optional[tk.Tk]) -> None:
        """Create the settings dialog."""
        self._dialog = tk.Toplevel(parent) if parent else tk.Tk()
        self._dialog.title("FreeFlow Settings")
        self._dialog.geometry("420x500")
        self._dialog.resizable(False, False)
        self._dialog.configure(bg=COLORS["bg_dark"])

        # Make dialog modal if parent exists
        if parent:
            self._dialog.transient(parent)
            self._dialog.grab_set()

        # Configure custom styles
        self._configure_styles()

        # Main container
        main_frame = tk.Frame(self._dialog, bg=COLORS["bg_dark"], padx=24, pady=20)
        main_frame.pack(fill=tk.BOTH, expand=True)

        # Title
        title_label = tk.Label(
            main_frame,
            text="Settings",
            font=("Segoe UI Semibold", 16),
            fg=COLORS["text_primary"],
            bg=COLORS["bg_dark"],
        )
        title_label.pack(anchor="w", pady=(0, 20))

        # Hotkey section
        self._create_section(main_frame, "Hotkey")
        hotkey_frame = tk.Frame(main_frame, bg=COLORS["bg_card"], padx=16, pady=12)
        hotkey_frame.pack(fill=tk.X, pady=(0, 16))

        self._hotkey_var = tk.StringVar(value=self._format_hotkey(self.current_hotkey))
        hotkey_label = tk.Label(
            hotkey_frame,
            textvariable=self._hotkey_var,
            font=("Segoe UI Semibold", 12),
            fg=COLORS["text_primary"],
            bg=COLORS["bg_card"],
        )
        hotkey_label.pack(side=tk.LEFT)

        self._record_button = tk.Button(
            hotkey_frame,
            text="Record",
            font=("Segoe UI", 9),
            fg=COLORS["text_primary"],
            bg=COLORS["bg_input"],
            activeforeground=COLORS["text_primary"],
            activebackground=COLORS["bg_hover"],
            relief=tk.FLAT,
            padx=12,
            pady=4,
            cursor="hand2",
            command=self._toggle_recording,
        )
        self._record_button.pack(side=tk.RIGHT)

        # Activation mode section
        self._create_section(main_frame, "Activation Mode")
        mode_frame = tk.Frame(main_frame, bg=COLORS["bg_card"], padx=16, pady=12)
        mode_frame.pack(fill=tk.X, pady=(0, 16))

        self._mode_var = tk.StringVar(value=self.current_mode)

        ptt_frame = tk.Frame(mode_frame, bg=COLORS["bg_card"])
        ptt_frame.pack(fill=tk.X, pady=(0, 8))

        ptt_radio = tk.Radiobutton(
            ptt_frame,
            text="Push-to-Talk",
            variable=self._mode_var,
            value=MODE_PUSH_TO_TALK,
            font=("Segoe UI", 10),
            fg=COLORS["text_primary"],
            bg=COLORS["bg_card"],
            activeforeground=COLORS["text_primary"],
            activebackground=COLORS["bg_card"],
            selectcolor=COLORS["bg_input"],
            cursor="hand2",
        )
        ptt_radio.pack(side=tk.LEFT)

        ptt_desc = tk.Label(
            ptt_frame,
            text="Hold hotkey to record",
            font=("Segoe UI", 9),
            fg=COLORS["text_muted"],
            bg=COLORS["bg_card"],
        )
        ptt_desc.pack(side=tk.RIGHT)

        toggle_frame = tk.Frame(mode_frame, bg=COLORS["bg_card"])
        toggle_frame.pack(fill=tk.X)

        toggle_radio = tk.Radiobutton(
            toggle_frame,
            text="Toggle",
            variable=self._mode_var,
            value=MODE_TOGGLE,
            font=("Segoe UI", 10),
            fg=COLORS["text_primary"],
            bg=COLORS["bg_card"],
            activeforeground=COLORS["text_primary"],
            activebackground=COLORS["bg_card"],
            selectcolor=COLORS["bg_input"],
            cursor="hand2",
        )
        toggle_radio.pack(side=tk.LEFT)

        toggle_desc = tk.Label(
            toggle_frame,
            text="Press to start/stop",
            font=("Segoe UI", 9),
            fg=COLORS["text_muted"],
            bg=COLORS["bg_card"],
        )
        toggle_desc.pack(side=tk.RIGHT)

        # Audio device section
        self._create_section(main_frame, "Audio Input")
        device_frame = tk.Frame(main_frame, bg=COLORS["bg_card"], padx=16, pady=12)
        device_frame.pack(fill=tk.X, pady=(0, 20))

        self._devices = AudioCapture.list_devices()
        device_names = ["System Default"] + [d[1] for d in self._devices]

        self._device_var = tk.StringVar()
        current_device = get_audio_device()
        if current_device is None:
            self._device_var.set("System Default")
        else:
            for idx, name, _ in self._devices:
                if idx == current_device:
                    self._device_var.set(name)
                    break
            else:
                self._device_var.set("System Default")

        device_combo = ttk.Combobox(
            device_frame,
            textvariable=self._device_var,
            values=device_names,
            state="readonly",
            font=("Segoe UI", 10),
        )
        device_combo.pack(fill=tk.X)

        # Buttons
        button_frame = tk.Frame(main_frame, bg=COLORS["bg_dark"])
        button_frame.pack(fill=tk.X, pady=(10, 0))

        save_btn = tk.Button(
            button_frame,
            text="Save",
            font=("Segoe UI Semibold", 10),
            fg=COLORS["bg_dark"],
            bg=COLORS["accent"],
            activeforeground=COLORS["bg_dark"],
            activebackground=COLORS["accent_hover"],
            relief=tk.FLAT,
            padx=20,
            pady=8,
            cursor="hand2",
            command=self._save_and_close,
        )
        save_btn.pack(side=tk.RIGHT)

        cancel_btn = tk.Button(
            button_frame,
            text="Cancel",
            font=("Segoe UI", 10),
            fg=COLORS["text_secondary"],
            bg=COLORS["bg_input"],
            activeforeground=COLORS["text_primary"],
            activebackground=COLORS["bg_hover"],
            relief=tk.FLAT,
            padx=16,
            pady=8,
            cursor="hand2",
            command=self._cancel,
        )
        cancel_btn.pack(side=tk.RIGHT, padx=(0, 10))

        # Handle window close
        self._dialog.protocol("WM_DELETE_WINDOW", self._cancel)

        # Center on screen
        self._dialog.update_idletasks()
        x = (self._dialog.winfo_screenwidth() - 420) // 2
        y = (self._dialog.winfo_screenheight() - 500) // 2
        self._dialog.geometry(f"+{x}+{y}")

    def _configure_styles(self) -> None:
        """Configure ttk styles for dark theme."""
        style = ttk.Style()

        # Combobox style
        style.configure(
            "TCombobox",
            fieldbackground=COLORS["bg_input"],
            background=COLORS["bg_input"],
            foreground=COLORS["text_primary"],
        )

    def _create_section(self, parent: tk.Frame, title: str) -> None:
        """Create a section header."""
        label = tk.Label(
            parent,
            text=title,
            font=("Segoe UI", 9),
            fg=COLORS["text_muted"],
            bg=COLORS["bg_dark"],
        )
        label.pack(anchor="w", pady=(0, 6))

    def _format_hotkey(self, hotkey: List[str]) -> str:
        """Format hotkey list as readable string."""
        formatted = []
        for key in hotkey:
            key_lower = key.lower()
            if key_lower == "ctrl_l":
                formatted.append("LCtrl")
            elif key_lower == "ctrl_r":
                formatted.append("RCtrl")
            elif key_lower == "shift_l":
                formatted.append("LShift")
            elif key_lower == "shift_r":
                formatted.append("RShift")
            elif key_lower == "alt_l":
                formatted.append("LAlt")
            elif key_lower == "alt_r":
                formatted.append("RAlt")
            elif key_lower == "alt_gr":
                formatted.append("AltGr")
            elif key_lower in ("cmd", "cmd_l"):
                formatted.append("Win")
            elif key_lower == "cmd_r":
                formatted.append("RWin")
            elif key_lower == "space":
                formatted.append("Space")
            else:
                formatted.append(key.upper() if len(key) == 1 else key.title())
        return " + ".join(formatted)

    def _toggle_recording(self) -> None:
        """Toggle hotkey recording mode."""
        if self._recording:
            self._stop_recording()
        else:
            self._start_recording()

    def _start_recording(self) -> None:
        """Start recording a new hotkey."""
        self._recording = True
        self._hotkey_var.set("Press keys...")
        self._record_button.configure(
            text="Stop",
            bg=COLORS["accent"],
            fg=COLORS["bg_dark"],
        )

        self._recorder = HotkeyRecorder(on_hotkey_recorded=self._on_hotkey_recorded)
        self._recorder.start_recording()

    def _stop_recording(self) -> None:
        """Stop recording and capture the hotkey."""
        self._recording = False
        self._record_button.configure(
            text="Record",
            bg=COLORS["bg_input"],
            fg=COLORS["text_primary"],
        )

        if self._recorder:
            recorded = self._recorder.stop_recording()
            if recorded:
                self.current_hotkey = recorded
            self._recorder = None

        self._hotkey_var.set(self._format_hotkey(self.current_hotkey))

    def _on_hotkey_recorded(self, hotkey: List[str]) -> None:
        """Handle recorded hotkey update."""
        if self._recording and self._dialog:
            self.current_hotkey = hotkey
            self._dialog.after(0, lambda: self._hotkey_var.set(self._format_hotkey(hotkey)))

    def _save_and_close(self) -> None:
        """Save settings and close dialog."""
        if self._recording:
            self._stop_recording()

        set_hotkey(self.current_hotkey)

        self.current_mode = self._mode_var.get()
        set_activation_mode(self.current_mode)

        device_name = self._device_var.get()
        if device_name == "System Default":
            set_audio_device(None)
        else:
            for idx, name, _ in self._devices:
                if name == device_name:
                    set_audio_device(idx)
                    break

        if self.on_settings_changed:
            self.on_settings_changed(self.current_hotkey, self.current_mode)

        self._close()

    def _cancel(self) -> None:
        """Cancel and close dialog."""
        if self._recording:
            self._stop_recording()
        self._close()

    def _close(self) -> None:
        """Close the dialog."""
        if self._dialog:
            self._dialog.destroy()
            self._dialog = None

        if self.on_close:
            self.on_close()

    def show(self) -> None:
        """Show the dialog and wait for it to close."""
        if self._dialog:
            self._dialog.wait_window()
