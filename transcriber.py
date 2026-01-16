"""Speech-to-text transcription using NVIDIA Parakeet TDT model."""

import os
import tempfile
import threading
from pathlib import Path
from typing import Callable, Optional

import numpy as np
import torch


class Transcriber:
    """Handles speech-to-text transcription using NeMo's Parakeet TDT model."""

    MODEL_NAME = "nvidia/parakeet-tdt-0.6b-v2"

    def __init__(self, on_model_loaded: Optional[Callable[[], None]] = None):
        """Initialize the transcriber.

        Args:
            on_model_loaded: Callback function called when model finishes loading.
        """
        self.model = None
        self.on_model_loaded = on_model_loaded
        self._loading = False
        self._load_thread: Optional[threading.Thread] = None

    def load_model_async(self) -> None:
        """Load the model in a background thread."""
        if self._loading or self.model is not None:
            return

        self._loading = True
        self._load_thread = threading.Thread(target=self._load_model, daemon=True)
        self._load_thread.start()

    def _load_model(self) -> None:
        """Load the NeMo ASR model."""
        try:
            import nemo.collections.asr as nemo_asr

            print(f"Loading model: {self.MODEL_NAME}")

            # Determine device
            if torch.cuda.is_available():
                device = "cuda"
                print("Using CUDA for inference")
            else:
                device = "cpu"
                print("Using CPU for inference (this will be slower)")

            # Load the model
            self.model = nemo_asr.models.ASRModel.from_pretrained(
                model_name=self.MODEL_NAME
            )
            self.model = self.model.to(device)
            self.model.eval()

            print("Model loaded successfully")

            if self.on_model_loaded:
                self.on_model_loaded()

        except Exception as e:
            print(f"Error loading model: {e}")
            raise
        finally:
            self._loading = False

    def is_ready(self) -> bool:
        """Check if the model is loaded and ready."""
        return self.model is not None

    def is_loading(self) -> bool:
        """Check if the model is currently loading."""
        return self._loading

    def transcribe(self, audio_data: np.ndarray, sample_rate: int = 16000) -> str:
        """Transcribe audio data to text.

        Args:
            audio_data: Audio samples as numpy array (float32, mono).
            sample_rate: Sample rate of the audio (default 16kHz).

        Returns:
            Transcribed text string.
        """
        if not self.is_ready():
            raise RuntimeError("Model not loaded. Call load_model_async() first.")

        # Create a temporary WAV file
        temp_dir = tempfile.gettempdir()
        temp_path = Path(temp_dir) / "freeflow_temp_audio.wav"

        try:
            # Save audio to WAV file
            self._save_wav(temp_path, audio_data, sample_rate)

            # Transcribe
            with torch.no_grad():
                transcriptions = self.model.transcribe([str(temp_path)])

            if transcriptions and len(transcriptions) > 0:
                # Handle different return formats
                result = transcriptions[0]
                if hasattr(result, "text"):
                    return result.text
                elif isinstance(result, str):
                    return result
                else:
                    return str(result)

            return ""

        except Exception as e:
            print(f"Transcription error: {e}")
            return ""

        finally:
            # Clean up temp file
            if temp_path.exists():
                try:
                    os.remove(temp_path)
                except OSError:
                    pass

    def _save_wav(
        self, path: Path, audio_data: np.ndarray, sample_rate: int
    ) -> None:
        """Save audio data to a WAV file.

        Args:
            path: Output file path.
            audio_data: Audio samples as numpy array.
            sample_rate: Sample rate in Hz.
        """
        from scipy.io import wavfile

        # Ensure audio is in the correct format
        if audio_data.dtype != np.float32:
            audio_data = audio_data.astype(np.float32)

        # Normalize if needed
        max_val = np.abs(audio_data).max()
        if max_val > 1.0:
            audio_data = audio_data / max_val

        # Convert to int16 for WAV
        audio_int16 = (audio_data * 32767).astype(np.int16)

        wavfile.write(str(path), sample_rate, audio_int16)

    def cleanup(self) -> None:
        """Clean up resources."""
        if self._load_thread and self._load_thread.is_alive():
            # Wait for loading to complete
            self._load_thread.join(timeout=5.0)

        self.model = None
