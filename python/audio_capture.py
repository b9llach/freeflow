"""Audio capture from microphone using sounddevice."""

import threading
from typing import Callable, List, Optional, Tuple

import numpy as np
import sounddevice as sd


class AudioCapture:
    """Handles microphone audio recording with streaming support."""

    SAMPLE_RATE = 16000  # 16kHz for speech recognition
    CHANNELS = 1  # Mono
    DTYPE = np.float32
    CHUNK_SIZE = 1024  # Samples per callback

    def __init__(
        self,
        device_index: Optional[int] = None,
        on_audio_chunk: Optional[Callable[[np.ndarray], None]] = None,
    ):
        """Initialize the audio capture.

        Args:
            device_index: Specific input device index, or None for default.
            on_audio_chunk: Callback called with each new audio chunk during recording.
        """
        self.device_index = device_index
        self.on_audio_chunk = on_audio_chunk
        self._recording = False
        self._audio_buffer: List[np.ndarray] = []
        self._lock = threading.Lock()
        self._stream: Optional[sd.InputStream] = None

    def set_chunk_callback(self, callback: Optional[Callable[[np.ndarray], None]]) -> None:
        """Set the callback for audio chunks.

        Args:
            callback: Function called with each new audio chunk, or None to disable.
        """
        self.on_audio_chunk = callback

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
                blocksize=self.CHUNK_SIZE,
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

    def get_current_buffer(self) -> Optional[np.ndarray]:
        """Get the current audio buffer without stopping recording.

        Returns:
            Copy of current audio buffer, or None if empty.
        """
        with self._lock:
            if not self._audio_buffer:
                return None

            # Concatenate all audio chunks
            audio_data = np.concatenate(self._audio_buffer, axis=0)

        # Flatten to 1D if needed
        if len(audio_data.shape) > 1:
            audio_data = audio_data.flatten()

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
            chunk = indata.copy()

            with self._lock:
                self._audio_buffer.append(chunk)

            # Call the chunk callback if set
            if self.on_audio_chunk:
                # Flatten for the callback
                flat_chunk = chunk.flatten() if len(chunk.shape) > 1 else chunk
                try:
                    self.on_audio_chunk(flat_chunk)
                except Exception as e:
                    print(f"Error in audio chunk callback: {e}")

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
