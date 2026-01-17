# FreeFlow

A local speech-to-text dictation tool that captures speech, transcribes it using NVIDIA's Parakeet TDT model, and types the text into the currently focused application.

## Features

- **Local transcription** - Uses NVIDIA's parakeet-tdt-0.6b-v2 model, no cloud API needed
- **Push-to-talk or Toggle mode** - Hold hotkey to record, or press once to start/stop
- **Configurable hotkey** - Set any key combination including modifier-only keys (e.g., Right Ctrl)
- **Instant paste** - Transcribed text is automatically pasted via Ctrl+V simulation
- **Transcription history** - View and copy all previous transcriptions
- **Word replacements** - Auto-replace words/phrases (e.g., "omw" -> "on my way")
- **Live transcription** - See words appear as you speak
- **Real-time status** - WebSocket-based instant UI updates
- **Premium UI** - Apple-inspired frosted glass design with floating indicator

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

pip install -r python/requirements.txt
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

### Option 2: Run components separately (development)

Terminal 1 - Start Python API:
```bash
python python/api.py
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
6. Release the hotkey - your speech will be transcribed and pasted automatically

### Windows

FreeFlow has two windows:

**Floating Indicator** - Always-on-top status display
- Shows current status (Ready, Recording, Transcribing)
- Displays the current hotkey
- Drag to reposition

**Main Window** - Full application interface
- **History tab** - View all past transcriptions, copy text
- **Replacements tab** - Add word/phrase auto-replacements
- **Settings tab** - Configure hotkey, activation mode, audio device

### Settings

In the Settings tab you can:

- Change the hotkey (supports modifier-only keys like Right Ctrl)
- Switch between Push-to-Talk and Toggle mode
- Select audio input device

### Replacements

Set up automatic word/phrase replacements:
- "omw" -> "on my way"
- "brb" -> "be right back"
- Case-sensitive and whole-word options available

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
| `/ws` | WebSocket | Real-time status updates |
| `/recording/start` | POST | Start recording |
| `/recording/stop` | POST | Stop and transcribe |
| `/recording/cancel` | POST | Cancel recording |
| `/config` | GET/POST | Get or save config |
| `/audio-devices` | GET | List audio devices |
| `/history` | GET | Get transcription history |
| `/history/clear` | POST | Clear all history |
| `/replacements` | GET/POST | Get or add replacements |
| `/replacements/{id}` | PUT/DELETE | Update or delete replacement |
| `/paste` | POST | Simulate Ctrl+V paste |

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
├── electron/                    # Electron frontend
│   ├── main.js                  # Main process
│   ├── preload.js               # IPC bridge
│   ├── index.html               # Floating indicator markup
│   ├── styles.css               # Floating indicator styles
│   ├── renderer.js              # Floating indicator logic
│   ├── main-window.html         # Main window markup
│   ├── main-window.css          # Main window styles
│   ├── main-window-renderer.js  # Main window logic
│   ├── setup.html               # First-run setup screen
│   ├── assets/                  # App icons
│   └── package.json             # Electron dependencies
├── python/                      # Python backend
│   ├── api.py                   # FastAPI server with WebSocket
│   ├── audio_capture.py         # Microphone recording
│   ├── transcriber.py           # NeMo model wrapper
│   ├── hotkey_manager.py        # Global hotkey handling
│   ├── history.py               # Transcription history storage
│   ├── replacements.py          # Word/phrase replacement rules
│   ├── config.py                # Configuration management
│   └── requirements.txt         # Python dependencies
└── README.md
```

## Building for Distribution

The app can be packaged into a standalone executable that bundles the Python source code. On first run, it will create a virtual environment and install dependencies automatically.

### Prerequisites

- Python 3.10 or 3.11 must be installed and in PATH on the target system
- Node.js 18+ for building

### Build Steps

```bash
# 1. Install Electron dependencies
cd electron
npm install

# 2. Build for your platform
npm run build:win   # Windows (.exe installer)
npm run build:mac   # macOS (.dmg)
npm run build:linux # Linux (.AppImage)
```

### Output

After building, you'll find:

- `electron/dist/FreeFlow Setup X.X.X.exe` - Windows installer
- `electron/dist/win-unpacked/` - Portable version (no install needed)

### Running the Built App

**Option 1: Install via Setup**
Run `FreeFlow Setup X.X.X.exe` to install to Program Files with Start Menu shortcut.

**Option 2: Portable**
Run `electron/dist/win-unpacked/FreeFlow.exe` directly.

**First Run:**
On first launch, the app will:
1. Show a setup screen while installing dependencies
2. Create a Python virtual environment in `%APPDATA%\FreeFlow`
3. Install all Python dependencies (~2-5 minutes)
4. Download the Parakeet model (~600MB)

Subsequent launches will start immediately since the venv persists in AppData.

## License

MIT
