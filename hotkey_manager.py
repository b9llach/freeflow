"""Global hotkey management using pynput."""

import threading
from typing import Callable, List, Optional, Set

from pynput import keyboard
from pynput.keyboard import Key, KeyCode


# Mapping from string names to pynput keys
KEY_MAP = {
    "ctrl": Key.ctrl_l,
    "ctrl_l": Key.ctrl_l,
    "ctrl_r": Key.ctrl_r,
    "shift": Key.shift_l,
    "shift_l": Key.shift_l,
    "shift_r": Key.shift_r,
    "alt": Key.alt_l,
    "alt_l": Key.alt_l,
    "alt_r": Key.alt_r,
    "cmd": Key.cmd,
    "win": Key.cmd,
    "space": Key.space,
    "tab": Key.tab,
    "enter": Key.enter,
    "esc": Key.esc,
    "escape": Key.esc,
    "backspace": Key.backspace,
    "delete": Key.delete,
    "insert": Key.insert,
    "home": Key.home,
    "end": Key.end,
    "page_up": Key.page_up,
    "page_down": Key.page_down,
    "up": Key.up,
    "down": Key.down,
    "left": Key.left,
    "right": Key.right,
    "f1": Key.f1,
    "f2": Key.f2,
    "f3": Key.f3,
    "f4": Key.f4,
    "f5": Key.f5,
    "f6": Key.f6,
    "f7": Key.f7,
    "f8": Key.f8,
    "f9": Key.f9,
    "f10": Key.f10,
    "f11": Key.f11,
    "f12": Key.f12,
}

# Reverse mapping from Key to string
KEY_TO_STRING = {v: k for k, v in KEY_MAP.items()}


def parse_key(key_str: str) -> Optional[keyboard.Key | KeyCode]:
    """Parse a key string to a pynput key.

    Args:
        key_str: String representation of the key.

    Returns:
        pynput Key or KeyCode, or None if invalid.
    """
    key_lower = key_str.lower().strip()

    # Check if it's a special key
    if key_lower in KEY_MAP:
        return KEY_MAP[key_lower]

    # Single character key
    if len(key_lower) == 1:
        return KeyCode.from_char(key_lower)

    return None


def key_to_string(key: keyboard.Key | KeyCode) -> str:
    """Convert a pynput key to a string representation.

    Args:
        key: pynput Key or KeyCode.

    Returns:
        String representation of the key.
    """
    if isinstance(key, Key):
        # Check our mapping
        if key in KEY_TO_STRING:
            return KEY_TO_STRING[key]
        # Fall back to name
        return key.name

    if isinstance(key, KeyCode):
        if key.char:
            return key.char
        if key.vk:
            return f"vk_{key.vk}"

    return str(key)


def normalize_key(key: keyboard.Key | KeyCode) -> keyboard.Key | KeyCode:
    """Normalize a key (e.g., treat left and right ctrl as the same).

    Args:
        key: The key to normalize.

    Returns:
        Normalized key.
    """
    # Normalize left/right variants of modifier keys
    if key in (Key.ctrl_l, Key.ctrl_r):
        return Key.ctrl_l
    if key in (Key.shift_l, Key.shift_r):
        return Key.shift_l
    if key in (Key.alt_l, Key.alt_r, Key.alt_gr):
        return Key.alt_l

    return key


class HotkeyManager:
    """Manages global hotkey detection for push-to-talk and toggle modes."""

    MODE_PUSH_TO_TALK = "push_to_talk"
    MODE_TOGGLE = "toggle"

    def __init__(
        self,
        hotkey: List[str],
        on_press: Optional[Callable[[], None]] = None,
        on_release: Optional[Callable[[], None]] = None,
        mode: str = MODE_PUSH_TO_TALK,
    ):
        """Initialize the hotkey manager.

        Args:
            hotkey: List of key strings (e.g., ["ctrl", "shift", "space"]).
            on_press: Callback when hotkey is pressed (or toggle starts recording).
            on_release: Callback when hotkey is released (or toggle stops recording).
            mode: Either MODE_PUSH_TO_TALK or MODE_TOGGLE.
        """
        self.on_press = on_press
        self.on_release = on_release
        self._mode = mode

        self._hotkey_keys: Set[keyboard.Key | KeyCode] = set()
        self._pressed_keys: Set[keyboard.Key | KeyCode] = set()
        self._hotkey_active = False
        self._toggle_recording = False  # For toggle mode: tracks if currently recording
        self._listener: Optional[keyboard.Listener] = None
        self._enabled = True
        self._lock = threading.Lock()

        self.set_hotkey(hotkey)

    def set_mode(self, mode: str) -> None:
        """Set the activation mode.

        Args:
            mode: Either MODE_PUSH_TO_TALK or MODE_TOGGLE.
        """
        with self._lock:
            self._mode = mode
            # Reset toggle state when changing modes
            self._toggle_recording = False
            self._hotkey_active = False
        print(f"Activation mode set to: {mode}")

    def set_hotkey(self, hotkey: List[str]) -> bool:
        """Set the hotkey combination.

        Args:
            hotkey: List of key strings.

        Returns:
            True if hotkey was set successfully.
        """
        new_keys: Set[keyboard.Key | KeyCode] = set()

        for key_str in hotkey:
            key = parse_key(key_str)
            if key is not None:
                new_keys.add(normalize_key(key))
            else:
                print(f"Warning: Unknown key '{key_str}'")

        if not new_keys:
            print("Error: No valid keys in hotkey")
            return False

        with self._lock:
            self._hotkey_keys = new_keys
            self._hotkey_active = False

        print(f"Hotkey set to: {' + '.join(hotkey)}")
        return True

    def get_hotkey_string(self) -> str:
        """Get the current hotkey as a readable string.

        Returns:
            Hotkey string like "Ctrl + Shift + Space".
        """
        with self._lock:
            keys = [key_to_string(k).title() for k in self._hotkey_keys]
        return " + ".join(sorted(keys))

    def start(self) -> None:
        """Start listening for hotkeys."""
        if self._listener is not None:
            return

        self._listener = keyboard.Listener(
            on_press=self._on_key_press,
            on_release=self._on_key_release,
        )
        self._listener.start()
        print("Hotkey listener started")

    def stop(self) -> None:
        """Stop listening for hotkeys."""
        if self._listener is not None:
            self._listener.stop()
            self._listener = None
        print("Hotkey listener stopped")

    def enable(self) -> None:
        """Enable hotkey detection."""
        self._enabled = True

    def disable(self) -> None:
        """Disable hotkey detection (e.g., when in settings dialog)."""
        self._enabled = False
        with self._lock:
            self._hotkey_active = False
            self._toggle_recording = False
            self._pressed_keys.clear()

    def _on_key_press(self, key: keyboard.Key | KeyCode) -> None:
        """Handle key press event.

        Args:
            key: The pressed key.
        """
        if not self._enabled:
            return

        normalized = normalize_key(key)

        with self._lock:
            self._pressed_keys.add(normalized)

            # Check if all hotkey keys are pressed
            if (
                not self._hotkey_active
                and self._hotkey_keys
                and self._hotkey_keys.issubset(self._pressed_keys)
            ):
                self._hotkey_active = True

                if self._mode == self.MODE_TOGGLE:
                    # Toggle mode: alternate between recording and not recording
                    if not self._toggle_recording:
                        # Start recording
                        self._toggle_recording = True
                        if self.on_press:
                            try:
                                self.on_press()
                            except Exception as e:
                                print(f"Error in on_press callback: {e}")
                    else:
                        # Stop recording
                        self._toggle_recording = False
                        if self.on_release:
                            try:
                                self.on_release()
                            except Exception as e:
                                print(f"Error in on_release callback: {e}")
                else:
                    # Push-to-talk mode: call on_press when hotkey is pressed
                    if self.on_press:
                        try:
                            self.on_press()
                        except Exception as e:
                            print(f"Error in on_press callback: {e}")

    def _on_key_release(self, key: keyboard.Key | KeyCode) -> None:
        """Handle key release event.

        Args:
            key: The released key.
        """
        if not self._enabled:
            return

        normalized = normalize_key(key)

        with self._lock:
            self._pressed_keys.discard(normalized)

            # Check if hotkey was active and a hotkey key was released
            if self._hotkey_active and normalized in self._hotkey_keys:
                self._hotkey_active = False

                # Only call on_release in push-to-talk mode
                # In toggle mode, on_release is called on the second press
                if self._mode == self.MODE_PUSH_TO_TALK:
                    if self.on_release:
                        try:
                            self.on_release()
                        except Exception as e:
                            print(f"Error in on_release callback: {e}")

    def is_hotkey_active(self) -> bool:
        """Check if the hotkey is currently being held.

        Returns:
            True if hotkey is active.
        """
        with self._lock:
            return self._hotkey_active

    def is_recording(self) -> bool:
        """Check if currently recording (for toggle mode).

        Returns:
            True if recording in toggle mode.
        """
        with self._lock:
            return self._toggle_recording

    def get_mode(self) -> str:
        """Get the current activation mode.

        Returns:
            The current mode (MODE_PUSH_TO_TALK or MODE_TOGGLE).
        """
        return self._mode

    def cancel_recording(self) -> None:
        """Cancel ongoing recording in toggle mode without triggering callback."""
        with self._lock:
            self._toggle_recording = False
            self._hotkey_active = False


class HotkeyRecorder:
    """Records a hotkey combination from user input."""

    def __init__(
        self,
        on_hotkey_recorded: Optional[Callable[[List[str]], None]] = None,
    ):
        """Initialize the hotkey recorder.

        Args:
            on_hotkey_recorded: Callback with the recorded hotkey.
        """
        self.on_hotkey_recorded = on_hotkey_recorded
        self._pressed_keys: Set[keyboard.Key | KeyCode] = set()
        self._recorded_keys: List[str] = []
        self._listener: Optional[keyboard.Listener] = None
        self._recording = False
        self._lock = threading.Lock()

    def start_recording(self) -> None:
        """Start recording hotkey input."""
        with self._lock:
            self._pressed_keys.clear()
            self._recorded_keys = []
            self._recording = True

        self._listener = keyboard.Listener(
            on_press=self._on_key_press,
            on_release=self._on_key_release,
        )
        self._listener.start()

    def stop_recording(self) -> List[str]:
        """Stop recording and return the recorded hotkey.

        Returns:
            List of key strings.
        """
        with self._lock:
            self._recording = False
            result = self._recorded_keys.copy()

        if self._listener:
            self._listener.stop()
            self._listener = None

        return result

    def _on_key_press(self, key: keyboard.Key | KeyCode) -> None:
        """Handle key press during recording."""
        if not self._recording:
            return

        normalized = normalize_key(key)

        with self._lock:
            self._pressed_keys.add(normalized)

    def _on_key_release(self, key: keyboard.Key | KeyCode) -> None:
        """Handle key release during recording."""
        if not self._recording:
            return

        with self._lock:
            # When a key is released, capture the current combination
            if self._pressed_keys:
                self._recorded_keys = [
                    key_to_string(k) for k in self._pressed_keys
                ]

                if self.on_hotkey_recorded:
                    try:
                        self.on_hotkey_recorded(self._recorded_keys.copy())
                    except Exception as e:
                        print(f"Error in hotkey recorded callback: {e}")
