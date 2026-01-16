"""FreeFlow - Local Speech-to-Text Dictation Tool.

Entry point that initializes and coordinates all components.
"""

import signal
import sys
import threading
import time
from typing import Optional

from audio_capture import AudioCapture
from config import get_activation_mode, get_audio_device, get_hotkey
from gui import FloatingWindow, Status
from hotkey_manager import HotkeyManager
from keyboard_output import KeyboardOutput
from settings_dialog import SettingsDialog
from transcriber import Transcriber


class FreeFlow:
    """Main application class that coordinates all components."""

    def __init__(self):
        """Initialize FreeFlow application."""
        self._running = False
        self._settings_open = False

        # Initialize components
        self._gui: Optional[FloatingWindow] = None
        self._transcriber: Optional[Transcriber] = None
        self._audio: Optional[AudioCapture] = None
        self._keyboard: Optional[KeyboardOutput] = None
        self._hotkey: Optional[HotkeyManager] = None

        # Locks for thread safety
        self._transcribe_lock = threading.Lock()

    def start(self) -> None:
        """Start the FreeFlow application."""
        print("Starting FreeFlow...")
        self._running = True

        # Load config
        hotkey_config = get_hotkey()
        activation_mode = get_activation_mode()

        # Initialize GUI first (it runs in its own thread)
        self._gui = FloatingWindow(
            on_settings=self._open_settings,
            on_quit=self._quit,
        )
        self._gui.set_status(Status.LOADING)
        self._gui.set_hotkey(hotkey_config)
        self._gui.set_mode(activation_mode)
        self._gui.start()

        # Wait for GUI to initialize
        time.sleep(0.5)

        # Initialize other components
        self._keyboard = KeyboardOutput()
        self._audio = AudioCapture(device_index=get_audio_device())

        # Initialize hotkey manager
        self._hotkey = HotkeyManager(
            hotkey=hotkey_config,
            on_press=self._on_hotkey_press,
            on_release=self._on_hotkey_release,
            mode=activation_mode,
        )
        self._hotkey.start()

        # Load transcription model (async)
        self._transcriber = Transcriber(on_model_loaded=self._on_model_loaded)
        self._transcriber.load_model_async()

        print(f"Hotkey: {self._hotkey.get_hotkey_string()}")
        mode_display = "Push-to-Talk" if activation_mode == "push_to_talk" else "Toggle"
        print(f"Mode: {mode_display}")
        print("Waiting for model to load...")

    def _on_model_loaded(self) -> None:
        """Called when the transcription model is loaded."""
        print("Model loaded. FreeFlow is ready!")
        if self._gui:
            self._gui.set_status(Status.READY)

    def _on_hotkey_press(self) -> None:
        """Called when the hotkey is pressed."""
        if self._settings_open:
            return

        if not self._transcriber or not self._transcriber.is_ready():
            print("Model not ready yet")
            return

        print("Recording started...")
        if self._gui:
            self._gui.set_status(Status.RECORDING)

        if self._audio:
            self._audio.start_recording()

    def _on_hotkey_release(self) -> None:
        """Called when the hotkey is released."""
        if self._settings_open:
            return

        if not self._transcriber or not self._transcriber.is_ready():
            return

        if not self._audio or not self._audio.is_recording():
            return

        print("Recording stopped, transcribing...")
        if self._gui:
            self._gui.set_status(Status.TRANSCRIBING)

        # Get recorded audio
        audio_data = self._audio.stop_recording()

        if audio_data is None or len(audio_data) == 0:
            print("No audio recorded")
            if self._gui:
                self._gui.set_status(Status.READY)
            return

        # Transcribe in a separate thread to avoid blocking
        threading.Thread(
            target=self._transcribe_and_type,
            args=(audio_data,),
            daemon=True,
        ).start()

    def _transcribe_and_type(self, audio_data) -> None:
        """Transcribe audio and type the result.

        Args:
            audio_data: Numpy array of audio samples.
        """
        with self._transcribe_lock:
            try:
                # Transcribe
                text = self._transcriber.transcribe(
                    audio_data,
                    sample_rate=AudioCapture.SAMPLE_RATE,
                )

                if text:
                    print(f"Transcribed: {text}")

                    # Type the text
                    if self._keyboard:
                        self._keyboard.type_text(text)
                else:
                    print("No speech detected")

            except Exception as e:
                print(f"Transcription error: {e}")

            finally:
                if self._gui:
                    self._gui.set_status(Status.READY)

    def _open_settings(self) -> None:
        """Open the settings dialog."""
        if self._settings_open:
            return

        self._settings_open = True

        # Disable hotkey while settings is open
        if self._hotkey:
            self._hotkey.disable()

        def on_close():
            self._settings_open = False
            if self._hotkey:
                self._hotkey.enable()

        def on_settings_changed(new_hotkey, new_mode):
            if self._hotkey:
                self._hotkey.set_hotkey(new_hotkey)
                self._hotkey.set_mode(new_mode)

            # Update GUI display
            if self._gui:
                self._gui.set_hotkey(new_hotkey)
                self._gui.set_mode(new_mode)

            # Update audio device if changed
            if self._audio:
                new_device = get_audio_device()
                self._audio.device_index = new_device

            mode_display = "Push-to-Talk" if new_mode == "push_to_talk" else "Toggle"
            print(f"Settings updated - Hotkey: {' + '.join(new_hotkey)}, Mode: {mode_display}")

        # Show settings dialog
        if self._gui:
            root = self._gui.get_root()
            self._gui.run_on_gui_thread(
                lambda: SettingsDialog(
                    parent=root,
                    current_hotkey=get_hotkey(),
                    current_mode=get_activation_mode(),
                    on_settings_changed=on_settings_changed,
                    on_close=on_close,
                )
            )

    def _quit(self) -> None:
        """Quit the application."""
        print("Shutting down FreeFlow...")
        self._running = False
        self.stop()

    def stop(self) -> None:
        """Stop the application and clean up resources."""
        self._running = False

        if self._hotkey:
            self._hotkey.stop()
            self._hotkey = None

        if self._audio:
            self._audio.cleanup()
            self._audio = None

        if self._transcriber:
            self._transcriber.cleanup()
            self._transcriber = None

        if self._gui:
            self._gui.stop()
            self._gui = None

        print("FreeFlow stopped")

    def run(self) -> None:
        """Run the application main loop."""
        self.start()

        # Set up signal handlers for graceful shutdown
        def signal_handler(sig, frame):
            print("\nReceived shutdown signal")
            self._quit()
            sys.exit(0)

        signal.signal(signal.SIGINT, signal_handler)
        signal.signal(signal.SIGTERM, signal_handler)

        # Keep main thread alive
        try:
            while self._running:
                time.sleep(0.1)
        except KeyboardInterrupt:
            print("\nInterrupted")
            self._quit()


def main():
    """Main entry point."""
    print("=" * 50)
    print("FreeFlow - Local Speech-to-Text Dictation")
    print("=" * 50)
    print()

    app = FreeFlow()
    app.run()


if __name__ == "__main__":
    main()
