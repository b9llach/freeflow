"""Keyboard output simulation using pynput."""

import time
from typing import Optional

from pynput.keyboard import Controller, Key


class KeyboardOutput:
    """Handles typing text into the focused application."""

    def __init__(self, typing_delay: float = 0.01):
        """Initialize keyboard output.

        Args:
            typing_delay: Delay between keystrokes in seconds.
        """
        self.controller = Controller()
        self.typing_delay = typing_delay

    def type_text(self, text: str, use_clipboard: bool = True) -> bool:
        """Type text into the currently focused application.

        Args:
            text: The text to type.
            use_clipboard: If True (default), use clipboard paste for instant output.
                          If False, type character by character.

        Returns:
            True if successful, False otherwise.
        """
        if not text:
            return False

        try:
            if use_clipboard:
                return self._paste_text(text)
            else:
                return self._type_text_directly(text)
        except Exception as e:
            print(f"Error typing text: {e}")
            return False

    def _type_text_directly(self, text: str) -> bool:
        """Type text character by character.

        Args:
            text: The text to type.

        Returns:
            True if successful.
        """
        for char in text:
            self.controller.type(char)
            if self.typing_delay > 0:
                time.sleep(self.typing_delay)
        return True

    def _paste_text(self, text: str) -> bool:
        """Paste text using clipboard.

        Args:
            text: The text to paste.

        Returns:
            True if successful, False otherwise.
        """
        try:
            import pyperclip

            # Save current clipboard content
            original_clipboard: Optional[str] = None
            try:
                original_clipboard = pyperclip.paste()
            except Exception:
                pass

            # Copy text to clipboard
            pyperclip.copy(text)

            # Small delay to ensure clipboard is updated
            time.sleep(0.05)

            # Paste (Ctrl+V on Windows/Linux, Cmd+V on macOS)
            import sys

            if sys.platform == "darwin":
                self.controller.press(Key.cmd)
                self.controller.press("v")
                self.controller.release("v")
                self.controller.release(Key.cmd)
            else:
                self.controller.press(Key.ctrl)
                self.controller.press("v")
                self.controller.release("v")
                self.controller.release(Key.ctrl)

            # Small delay after paste
            time.sleep(0.05)

            # Restore original clipboard content
            if original_clipboard is not None:
                try:
                    pyperclip.copy(original_clipboard)
                except Exception:
                    pass

            return True

        except ImportError:
            print("pyperclip not installed. Falling back to direct typing.")
            return self._type_text_directly(text)
        except Exception as e:
            print(f"Error pasting text: {e}")
            return False

    def press_key(self, key: Key) -> None:
        """Press a single key.

        Args:
            key: The key to press.
        """
        self.controller.press(key)
        self.controller.release(key)

    def press_enter(self) -> None:
        """Press the Enter key."""
        self.press_key(Key.enter)

    def press_tab(self) -> None:
        """Press the Tab key."""
        self.press_key(Key.tab)

    def press_backspace(self, count: int = 1) -> None:
        """Press Backspace key multiple times.

        Args:
            count: Number of times to press backspace.
        """
        for _ in range(count):
            self.press_key(Key.backspace)
            time.sleep(0.01)
