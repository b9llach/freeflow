# FreeFlow

A local speech-to-text dictation tool that captures speech, transcribes it using NVIDIA's Parakeet TDT model, and types the text into the currently focused application.

## Features

- **Local transcription** - Uses NVIDIA's parakeet-tdt-0.6b-v2 model, no cloud API needed
- **Push-to-talk or Toggle mode** - Hold hotkey to record, or press once to start/stop
- **Configurable hotkey** - Set any key combination with left/right modifier distinction
- **Instant paste** - Transcribed text is pasted instantly via clipboard
- **Cross-platform UI** - Electron-based floating indicator window
- **Native performance** - Python backend handles audio capture and transcription

## Architecture

FreeFlow uses a hybrid architecture:
- **Electron frontend** - Cross-platform desktop UI with a premium dark theme
- **Python backend** - FastAPI server handling audio capture and NeMo transcription

## Requirements

- Python 3.10 or 3.11
- Node.js 18+ and npm
- Windows 10/11 (macOS/Linux support planned)
- NVIDIA GPU recommended (works on CPU but slower)
- ~2GB disk space for the model

## Installation

### 1. Clone the repository

```bash
git clone <repository-url>
cd freeflow
```

### 2. Set up Python backend

Create a virtual environment and install dependencies:

```bash
python -m venv .venv

# Windows
.venv\Scripts\activate

# macOS/Linux
source .venv/bin/activate

pip install -r requirements.txt
```

Note: The first install may take a while as it downloads PyTorch and NeMo.

### 3. Set up Electron frontend

```bash
cd electron
npm install
```

## Running FreeFlow

### Option 1: Run Electron app (recommended)

The Electron app will automatically start the Python backend:

```bash
cd electron
npm start
```

### Option 2: Run Python standalone (tkinter UI)

For the original Python-only version with tkinter UI:

```bash
python main.py
```

### Option 3: Run components separately (development)

Terminal 1 - Start Python API:
```bash
python api.py
```

Terminal 2 - Start Electron:
```bash
cd electron
npm start
```

## First Run

On the first run, FreeFlow will download the Parakeet model from NVIDIA (~600MB). This only happens once - the model is cached locally afterward.

## Usage

1. Start FreeFlow using one of the methods above
2. Wait for the model to load (status shows "Ready")
3. Open any text editor or input field
4. Press and hold the hotkey (default: `LCtrl+LShift+Space`) to record
5. Speak into your microphone
6. Release the hotkey - your speech will be transcribed and pasted

### Floating Window

- **Drag** to reposition
- **Right-click** to access Settings

### Settings

Right-click the floating window to:

- Change the hotkey (with left/right modifier support)
- Switch between Push-to-Talk and Toggle mode
- Select audio input device

## Configuration

Settings are saved to:
- Windows: `%APPDATA%\freeflow\config.json`
- macOS: `~/Library/Application Support/freeflow/config.json`
- Linux: `~/.config/freeflow/config.json`

Default configuration:
```json
{
  "hotkey": ["ctrl_l", "shift_l", "space"],
  "activation_mode": "push_to_talk",
  "window_position": [100, 100],
  "audio_device": null
}
```

## Modes

- **Push-to-Talk**: Hold the hotkey to record, release to transcribe
- **Toggle**: Press hotkey once to start recording, press again to stop and transcribe

## API Endpoints

When running, the Python backend exposes these endpoints at `http://127.0.0.1:5000`:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/status` | GET | Get current status |
| `/recording/start` | POST | Start recording |
| `/recording/stop` | POST | Stop and transcribe |
| `/recording/cancel` | POST | Cancel recording |
| `/config` | GET/POST | Get or save config |
| `/audio-devices` | GET | List audio devices |

## Troubleshooting

### "Model not ready yet"
Wait for the model to finish loading. First run downloads ~600MB.

### No audio recorded
Check your microphone is connected and selected in Settings.

### Text not appearing
Make sure the target application is focused before releasing the hotkey.

### Slow transcription
GPU is recommended. On CPU, transcription may take longer.

### Python API not starting
Ensure Python is in your PATH and the virtual environment is activated.

### Electron window not showing
Check the developer tools (Ctrl+Shift+I) for errors.

## File Structure

```
freeflow/
├── electron/                # Electron frontend
│   ├── main.js              # Main process
│   ├── preload.js           # IPC bridge
│   ├── index.html           # UI markup
│   ├── styles.css           # Styling
│   ├── renderer.js          # UI logic
│   └── package.json         # Electron dependencies
├── api.py                   # FastAPI backend
├── audio_capture.py         # Microphone recording
├── transcriber.py           # NeMo model wrapper
├── keyboard_output.py       # Text pasting
├── hotkey_manager.py        # Global hotkey handling
├── config.py                # Configuration management
├── main.py                  # Python-only entry (tkinter)
├── gui.py                   # Tkinter floating window
├── settings_dialog.py       # Tkinter settings UI
└── requirements.txt         # Python dependencies
```

## Building for Distribution

(Coming soon)

The Electron app can be packaged for distribution using electron-builder:

```bash
cd electron
npm run build:win   # Windows
npm run build:mac   # macOS
npm run build:linux # Linux
```

## License

MIT
