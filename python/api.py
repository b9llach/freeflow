"""FreeFlow API Server - FastAPI backend for Electron frontend."""

import asyncio
import json
import threading
import time
from contextlib import asynccontextmanager
from typing import Any, Dict, List, Optional, Set

from fastapi import FastAPI, HTTPException, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import uvicorn
import pyperclip
from pynput.keyboard import Controller as KeyboardController, Key as KeyboardKey

from audio_capture import AudioCapture
from config import (
    get_activation_mode,
    get_audio_device,
    get_hotkey,
    get_window_position,
    load_config,
    save_config,
    set_activation_mode,
    set_audio_device,
    set_hotkey,
    set_window_position,
)
from history import (
    add_to_history,
    clear_history,
    delete_history_entry,
    get_history,
    get_history_stats,
)
from replacements import (
    add_replacement,
    apply_replacements,
    delete_replacement,
    get_replacements,
    update_replacement,
)
from transcriber import Transcriber
from hotkey_manager import HotkeyManager


# Global instances
transcriber: Optional[Transcriber] = None
audio: Optional[AudioCapture] = None
hotkey_manager: Optional[HotkeyManager] = None
is_recording = False
recording_start_time: Optional[float] = None
current_partial_text = ""  # Current streaming transcription text


# WebSocket connection manager
class ConnectionManager:
    """Manages WebSocket connections for real-time status updates."""

    def __init__(self):
        self.active_connections: Set[WebSocket] = set()
        self._loop: Optional[asyncio.AbstractEventLoop] = None

    async def connect(self, websocket: WebSocket):
        await websocket.accept()
        self.active_connections.add(websocket)
        # Store the event loop for thread-safe broadcasting
        self._loop = asyncio.get_event_loop()
        print(f"WebSocket connected. Total connections: {len(self.active_connections)}")

    def disconnect(self, websocket: WebSocket):
        self.active_connections.discard(websocket)
        print(f"WebSocket disconnected. Total connections: {len(self.active_connections)}")

    async def broadcast(self, message: dict):
        """Send message to all connected clients."""
        if not self.active_connections:
            return

        disconnected = set()
        for connection in self.active_connections:
            try:
                await connection.send_json(message)
            except Exception:
                disconnected.add(connection)

        # Clean up disconnected clients
        self.active_connections -= disconnected

    def broadcast_sync(self, message: dict):
        """Thread-safe broadcast from non-async context."""
        if self._loop and self.active_connections:
            asyncio.run_coroutine_threadsafe(self.broadcast(message), self._loop)


ws_manager = ConnectionManager()


def broadcast_status(status: str, extra: dict = None):
    """Broadcast status update to all WebSocket clients."""
    message = {"type": "status", "status": status}
    if extra:
        message.update(extra)
    ws_manager.broadcast_sync(message)


def broadcast_partial_transcript(text: str):
    """Broadcast partial transcription to all WebSocket clients."""
    message = {"type": "partial_transcript", "text": text}
    ws_manager.broadcast_sync(message)


def on_audio_chunk(chunk):
    """Callback for processing audio chunks during streaming transcription."""
    global current_partial_text

    if not transcriber or not transcriber.is_ready():
        return

    if not is_recording:
        return

    # Process chunk through transcriber - returns full transcription of buffer so far
    full_text = transcriber.transcribe_chunk(chunk, sample_rate=AudioCapture.SAMPLE_RATE)

    # Broadcast if we got new/updated transcription
    if full_text and full_text.strip() and full_text != current_partial_text:
        current_partial_text = full_text.strip()
        broadcast_partial_transcript(current_partial_text)
        print(f"Live: {current_partial_text}")


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Initialize components on startup."""
    global transcriber, audio

    print("Starting FreeFlow API...")

    # Initialize audio capture with streaming callback
    audio = AudioCapture(device_index=get_audio_device(), on_audio_chunk=on_audio_chunk)

    # Initialize transcriber and start loading model
    transcriber = Transcriber(on_model_loaded=on_model_loaded)
    transcriber.load_model_async()

    print("Model loading in background...")

    yield

    # Cleanup on shutdown
    print("Shutting down FreeFlow API...")
    if hotkey_manager:
        hotkey_manager.stop()
    if audio:
        audio.cleanup()
    if transcriber:
        transcriber.cleanup()


def on_model_loaded():
    """Callback when model finishes loading."""
    print("Model loaded and ready!")
    broadcast_status("ready", {"model_ready": True})


app = FastAPI(title="FreeFlow API", lifespan=lifespan)

# Allow Electron to connect
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


# === WebSocket Endpoint ===

@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    """WebSocket endpoint for real-time status updates."""
    await ws_manager.connect(websocket)

    # Send initial status
    if transcriber is None:
        status = "initializing"
    elif transcriber.is_loading():
        status = "loading"
    elif transcriber.is_ready():
        status = "recording" if is_recording else "ready"
    else:
        status = "error"

    await websocket.send_json({
        "type": "status",
        "status": status,
        "model_ready": transcriber.is_ready() if transcriber else False,
        "is_recording": is_recording
    })

    try:
        while True:
            # Keep connection alive, handle any incoming messages
            data = await websocket.receive_text()
            # Could handle client messages here if needed
    except WebSocketDisconnect:
        ws_manager.disconnect(websocket)


# === Pydantic Models ===

class ConfigModel(BaseModel):
    hotkey: list[str]
    activation_mode: str
    window_position: list[int]
    audio_device: Optional[int] = None


class HotkeyModel(BaseModel):
    hotkey: list[str]


class ModeModel(BaseModel):
    mode: str


class PositionModel(BaseModel):
    x: int
    y: int


class AudioDeviceModel(BaseModel):
    device_index: Optional[int] = None


class TranscriptionResult(BaseModel):
    text: str
    original_text: str
    success: bool
    duration_seconds: Optional[float] = None


class ReplacementModel(BaseModel):
    find: str
    replace: str
    case_sensitive: bool = False
    whole_word: bool = True
    enabled: bool = True


class ReplacementUpdateModel(BaseModel):
    find: Optional[str] = None
    replace: Optional[str] = None
    case_sensitive: Optional[bool] = None
    whole_word: Optional[bool] = None
    enabled: Optional[bool] = None


# === Status Endpoints ===

@app.get("/status")
def get_status():
    """Get current application status."""
    global is_recording

    if transcriber is None:
        status = "initializing"
    elif transcriber.is_loading():
        status = "loading"
    elif transcriber.is_ready():
        if is_recording:
            status = "recording"
        else:
            status = "ready"
    else:
        status = "error"

    return {
        "status": status,
        "model_ready": transcriber.is_ready() if transcriber else False,
        "is_recording": is_recording,
    }


@app.get("/health")
def health_check():
    """Simple health check endpoint."""
    return {"healthy": True}


# === Recording Endpoints ===

@app.post("/recording/start")
def start_recording():
    """Start audio recording with streaming transcription."""
    global is_recording, recording_start_time, current_partial_text

    if not transcriber or not transcriber.is_ready():
        raise HTTPException(status_code=503, detail="Model not ready yet")

    if is_recording:
        raise HTTPException(status_code=400, detail="Already recording")

    if audio:
        # Reset streaming state
        current_partial_text = ""
        transcriber.reset_streaming()

        success = audio.start_recording()
        if success:
            is_recording = True
            recording_start_time = time.time()
            broadcast_status("recording", {"is_recording": True})
            return {"recording": True, "message": "Recording started"}
        else:
            raise HTTPException(status_code=500, detail="Failed to start recording")

    raise HTTPException(status_code=500, detail="Audio capture not initialized")


@app.post("/recording/stop")
def stop_recording() -> TranscriptionResult:
    """Stop recording, transcribe, apply replacements, and save to history."""
    global is_recording, recording_start_time, current_partial_text

    if not is_recording:
        raise HTTPException(status_code=400, detail="Not currently recording")

    if not audio:
        raise HTTPException(status_code=500, detail="Audio capture not initialized")

    # Calculate duration
    duration = None
    if recording_start_time:
        duration = time.time() - recording_start_time

    # Flush any remaining audio in streaming buffer for live view
    if transcriber and transcriber.is_ready():
        flush_text = transcriber.flush_streaming(sample_rate=AudioCapture.SAMPLE_RATE)
        if flush_text and flush_text.strip():
            current_partial_text = flush_text.strip()
            broadcast_partial_transcript(current_partial_text)
            print(f"Flush: {current_partial_text}")

    # Stop recording
    audio_data = audio.stop_recording()
    is_recording = False
    recording_start_time = None

    broadcast_status("transcribing", {"is_recording": False})

    if audio_data is None or len(audio_data) == 0:
        broadcast_status("ready")
        return TranscriptionResult(text="", original_text="", success=False)

    # Transcribe
    if transcriber and transcriber.is_ready():
        original_text = transcriber.transcribe(audio_data, sample_rate=AudioCapture.SAMPLE_RATE)

        # Apply replacements
        final_text = apply_replacements(original_text)

        # Save to history
        add_to_history(
            original_text=original_text,
            final_text=final_text,
            duration_seconds=duration
        )

        broadcast_status("ready", {"transcription": final_text})

        return TranscriptionResult(
            text=final_text,
            original_text=original_text,
            success=True,
            duration_seconds=duration
        )

    broadcast_status("error")
    raise HTTPException(status_code=503, detail="Transcriber not ready")


@app.post("/recording/cancel")
def cancel_recording():
    """Cancel recording without transcribing."""
    global is_recording, recording_start_time

    if is_recording and audio:
        audio.stop_recording()  # Discard the audio
        is_recording = False
        recording_start_time = None
        broadcast_status("ready", {"is_recording": False})

    return {"cancelled": True}


# === Configuration Endpoints ===

@app.get("/config")
def get_config_endpoint():
    """Get current configuration."""
    return load_config()


@app.post("/config")
def save_config_endpoint(config: ConfigModel):
    """Save configuration."""
    config_dict = config.model_dump()
    save_config(config_dict)
    return {"saved": True}


@app.get("/config/hotkey")
def get_hotkey_endpoint():
    """Get current hotkey."""
    return {"hotkey": get_hotkey()}


@app.post("/config/hotkey")
def set_hotkey_endpoint(data: HotkeyModel):
    """Set hotkey."""
    set_hotkey(data.hotkey)
    return {"hotkey": data.hotkey}


@app.get("/config/mode")
def get_mode_endpoint():
    """Get activation mode."""
    return {"mode": get_activation_mode()}


@app.post("/config/mode")
def set_mode_endpoint(data: ModeModel):
    """Set activation mode."""
    set_activation_mode(data.mode)
    return {"mode": data.mode}


@app.get("/config/position")
def get_position_endpoint():
    """Get window position."""
    pos = get_window_position()
    return {"x": pos[0], "y": pos[1]}


@app.post("/config/position")
def set_position_endpoint(data: PositionModel):
    """Set window position."""
    set_window_position(data.x, data.y)
    return {"x": data.x, "y": data.y}


@app.get("/config/audio-device")
def get_audio_device_endpoint():
    """Get audio device."""
    return {"device_index": get_audio_device()}


@app.post("/config/audio-device")
def set_audio_device_endpoint(data: AudioDeviceModel):
    """Set audio device."""
    global audio
    set_audio_device(data.device_index)
    # Update the audio capture instance
    if audio:
        audio.device_index = data.device_index
    return {"device_index": data.device_index}


@app.get("/audio-devices")
def list_audio_devices():
    """List available audio input devices."""
    devices = AudioCapture.list_devices()
    return {
        "devices": [
            {"index": idx, "name": name, "channels": channels}
            for idx, name, channels in devices
        ]
    }


# === History Endpoints ===

@app.get("/history")
def get_history_endpoint(limit: Optional[int] = None, offset: int = 0):
    """Get transcription history."""
    entries = get_history(limit=limit, offset=offset)
    stats = get_history_stats()
    return {
        "entries": entries,
        "stats": stats
    }


@app.delete("/history")
def clear_history_endpoint():
    """Clear all transcription history."""
    success = clear_history()
    return {"cleared": success}


@app.delete("/history/{entry_id}")
def delete_history_entry_endpoint(entry_id: int):
    """Delete a specific history entry."""
    success = delete_history_entry(entry_id)
    if not success:
        raise HTTPException(status_code=404, detail="History entry not found")
    return {"deleted": True}


# === Replacements Endpoints ===

@app.get("/replacements")
def get_replacements_endpoint():
    """Get all replacement rules."""
    return {"replacements": get_replacements()}


@app.post("/replacements")
def add_replacement_endpoint(data: ReplacementModel):
    """Add a new replacement rule."""
    rule = add_replacement(
        find=data.find,
        replace=data.replace,
        case_sensitive=data.case_sensitive,
        whole_word=data.whole_word,
        enabled=data.enabled
    )
    return {"replacement": rule}


@app.put("/replacements/{rule_id}")
def update_replacement_endpoint(rule_id: str, data: ReplacementUpdateModel):
    """Update an existing replacement rule."""
    updates = {k: v for k, v in data.model_dump().items() if v is not None}
    rule = update_replacement(rule_id, updates)
    if not rule:
        raise HTTPException(status_code=404, detail="Replacement rule not found")
    return {"replacement": rule}


@app.delete("/replacements/{rule_id}")
def delete_replacement_endpoint(rule_id: str):
    """Delete a replacement rule."""
    success = delete_replacement(rule_id)
    if not success:
        raise HTTPException(status_code=404, detail="Replacement rule not found")
    return {"deleted": True}


@app.post("/replacements/test")
def test_replacements(text: str):
    """Test replacements on sample text."""
    result = apply_replacements(text)
    return {"original": text, "result": result}


# === Hotkey Endpoints (for modifier-only hotkeys) ===

class HotkeyEnableModel(BaseModel):
    hotkey: list[str]
    mode: str


def on_hotkey_press():
    """Called when hotkey is pressed (Python-side detection)."""
    global is_recording, recording_start_time, current_partial_text

    print("Hotkey pressed!")

    if not transcriber or not transcriber.is_ready():
        print("Model not ready yet")
        return

    if is_recording:
        print("Already recording")
        return

    if audio:
        print("Starting recording...")
        # Reset streaming state
        current_partial_text = ""
        transcriber.reset_streaming()

        success = audio.start_recording()
        if success:
            is_recording = True
            recording_start_time = time.time()
            broadcast_status("recording", {"is_recording": True})
            print("Recording started")
        else:
            print("Failed to start recording")


def transcribe_and_paste(audio_data, duration):
    """Transcribe audio and paste result in background thread."""
    global transcriber

    if not transcriber or not transcriber.is_ready():
        print("Transcriber not ready")
        broadcast_status("error")
        return

    broadcast_status("transcribing", {"is_recording": False})
    print("Transcribing...")

    original_text = transcriber.transcribe(audio_data, sample_rate=AudioCapture.SAMPLE_RATE)

    if not original_text or not original_text.strip():
        print("No speech detected")
        broadcast_status("ready")
        return

    # Apply replacements
    final_text = apply_replacements(original_text)

    # Save to history
    add_to_history(
        original_text=original_text,
        final_text=final_text,
        duration_seconds=duration
    )

    print(f"Transcribed: {final_text}")

    # Copy to clipboard
    pyperclip.copy(final_text)

    # Delay to ensure hotkey is fully released and clipboard is ready
    time.sleep(0.15)

    # Simulate Ctrl+V to paste
    keyboard = KeyboardController()
    keyboard.press(KeyboardKey.ctrl)
    keyboard.press('v')
    keyboard.release('v')
    keyboard.release(KeyboardKey.ctrl)

    print("Text pasted")
    broadcast_status("ready", {"transcription": final_text})


def on_hotkey_release():
    """Called when hotkey is released (Python-side detection)."""
    global is_recording, recording_start_time, current_partial_text

    if not is_recording:
        return

    if not audio:
        return

    print("Stopping recording...")

    # Calculate duration
    duration = None
    if recording_start_time:
        duration = time.time() - recording_start_time

    # Flush any remaining audio in streaming buffer for live view
    if transcriber and transcriber.is_ready():
        flush_text = transcriber.flush_streaming(sample_rate=AudioCapture.SAMPLE_RATE)
        if flush_text and flush_text.strip():
            current_partial_text = flush_text.strip()
            broadcast_partial_transcript(current_partial_text)
            print(f"Flush: {current_partial_text}")

    # Stop recording
    audio_data = audio.stop_recording()
    is_recording = False
    recording_start_time = None

    if audio_data is None or len(audio_data) == 0:
        print("No audio recorded")
        broadcast_status("ready")
        return

    # Run transcription and paste in background thread to not block hotkey listener
    thread = threading.Thread(target=transcribe_and_paste, args=(audio_data, duration))
    thread.daemon = True
    thread.start()


@app.post("/hotkey/enable")
def enable_hotkey(data: HotkeyEnableModel):
    """Enable Python-side hotkey detection (for modifier-only hotkeys)."""
    global hotkey_manager

    print(f"Enabling Python hotkey detection: {data.hotkey}")

    if hotkey_manager:
        hotkey_manager.stop()

    hotkey_manager = HotkeyManager(
        hotkey=data.hotkey,
        on_press=on_hotkey_press,
        on_release=on_hotkey_release,
        mode=data.mode
    )
    hotkey_manager.start()

    return {"enabled": True, "hotkey": data.hotkey, "mode": data.mode}


@app.post("/hotkey/disable")
def disable_hotkey():
    """Disable Python-side hotkey detection."""
    global hotkey_manager

    if hotkey_manager:
        hotkey_manager.stop()
        hotkey_manager = None
        print("Python hotkey detection disabled")

    return {"disabled": True}


@app.post("/paste")
def paste_from_clipboard():
    """Simulate Ctrl+V to paste from clipboard."""
    keyboard = KeyboardController()
    keyboard.press(KeyboardKey.ctrl)
    keyboard.press('v')
    keyboard.release('v')
    keyboard.release(KeyboardKey.ctrl)
    return {"pasted": True}


# === Main Entry Point ===

def main():
    """Run the API server."""
    print("=" * 50)
    print("FreeFlow API Server")
    print("=" * 50)
    print()

    uvicorn.run(
        app,
        host="127.0.0.1",
        port=5000,
        log_level="info",
    )


if __name__ == "__main__":
    main()
