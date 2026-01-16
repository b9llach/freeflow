"""Audio capture from microphone using sounddevice."""

import threading
from typing import List, Optional, Tuple

import numpy as np
import sounddevice as sd


class AudioCapture:
    """Handles microphone audio recording."""

    SAMPLE_RATE = 16000  # 16kHz for speech recognition
    CHANNELS = 1  # Mono
    DTYPE = np.float32

    def __init__(self, device_index: Optional[int] = None):
        """Initialize the audio capture.

        Args:
            device_index: Specific input device index, or None for default.
        """
        self.device_index = device_index
        self._recording = False
        self._audio_buffer: List[np.ndarray] = []
        self._lock = threading.Lock()
        self._stream: Optional[sd.InputStream] = None

    def start_recording(self) -> bool:
        """Start recording from the microphone.

        Returns:
            True if recording started successfully, False otherwise.
        """
        if self._recording:
            return False

        with self._lock:
            self._audio_buffer = []

        try:
            self._stream = sd.InputStream(
                samplerate=self.SAMPLE_RATE,
                channels=self.CHANNELS,
                dtype=self.DTYPE,
                device=self.device_index,
                callback=self._audio_callback,
                blocksize=1024,
            )
            self._stream.start()
            self._recording = True
            print("Recording started")
            return True

        except Exception as e:
            print(f"Error starting recording: {e}")
            return False

    def stop_recording(self) -> Optional[np.ndarray]:
        """Stop recording and return the captured audio.

        Returns:
            Numpy array of audio samples, or None if no audio was captured.
        """
        if not self._recording:
            return None

        self._recording = False

        if self._stream:
            try:
                self._stream.stop()
                self._stream.close()
            except Exception as e:
                print(f"Error stopping stream: {e}")
            finally:
                self._stream = None

        with self._lock:
            if not self._audio_buffer:
                return None

            # Concatenate all audio chunks
            audio_data = np.concatenate(self._audio_buffer, axis=0)
            self._audio_buffer = []

        # Flatten to 1D if needed
        if len(audio_data.shape) > 1:
            audio_data = audio_data.flatten()

        print(f"Recording stopped. Captured {len(audio_data)} samples ({len(audio_data) / self.SAMPLE_RATE:.2f}s)")
        return audio_data

    def _audio_callback(
        self,
        indata: np.ndarray,
        frames: int,
        time_info: dict,
        status: sd.CallbackFlags,
    ) -> None:
        """Callback for audio stream.

        Args:
            indata: Input audio data.
            frames: Number of frames.
            time_info: Time information.
            status: Status flags.
        """
        if status:
            print(f"Audio callback status: {status}")

        if self._recording:
            with self._lock:
                self._audio_buffer.append(indata.copy())

    def is_recording(self) -> bool:
        """Check if currently recording."""
        return self._recording

    @staticmethod
    def list_devices() -> List[Tuple[int, str, int]]:
        """List available audio input devices.

        Returns:
            List of tuples: (device_index, device_name, max_input_channels)
        """
        devices = []
        try:
            device_list = sd.query_devices()
            for i, device in enumerate(device_list):
                if device["max_input_channels"] > 0:
                    devices.append((
                        i,
                        device["name"],
                        device["max_input_channels"],
                    ))
        except Exception as e:
            print(f"Error listing devices: {e}")

        return devices

    @staticmethod
    def get_default_device() -> Optional[int]:
        """Get the default input device index.

        Returns:
            Default input device index, or None if not available.
        """
        try:
            default = sd.query_devices(kind="input")
            if default:
                return sd.default.device[0]
        except Exception:
            pass
        return None

    def cleanup(self) -> None:
        """Clean up resources."""
        if self._recording:
            self.stop_recording()
