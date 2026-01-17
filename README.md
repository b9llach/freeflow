# FreeFlow

A local speech-to-text dictation tool that captures speech, transcribes it using NVIDIA's Parakeet TDT model, and types the text into the currently focused application.

## Features

- **Local transcription** - Uses NVIDIA's parakeet-tdt-0.6b-v2 model, no cloud API needed
- **Push-to-talk or Toggle mode** - Hold hotkey to record, or press once to start/stop
- **Configurable hotkey** - Set any key combination via settings
- **Instant paste** - Transcribed text is pasted instantly via clipboard
- **Floating indicator** - Small always-on-top window showing status

## Requirements

- Python 3.10 or 3.11
- Windows 10/11
- NVIDIA GPU recommended (works on CPU but slower)
- ~2GB disk space for the model

## Installation

1. Clone or download this repository

2. Create a virtual environment (recommended):
   ```bash
   python -m venv .venv
   .venv\Scripts\activate
   ```

3. Install dependencies:
   ```bash
   pip install -r requirements.txt
   ```

   Note: The first install may take a while as it downloads PyTorch and NeMo.

4. Run FreeFlow:
   ```bash
   python main.py
   ```

## First Run

On the first run, FreeFlow will download the Parakeet model from NVIDIA (~600MB). This only happens once - the model is cached locally afterward.

## Usage

1. Run `python main.py`
2. Wait for the model to load (status shows "Ready")
3. Open any text editor or input field
4. Press and hold the hotkey (default: `Ctrl+Shift+Space`) to record
5. Speak into your microphone
6. Release the hotkey - your speech will be transcribed and typed

### Floating Window

- **Drag** to reposition
- **Right-click** to access Settings or Quit

### Settings

Right-click the floating window and select "Settings" to:

- Change the hotkey
- Switch between Push-to-Talk and Toggle mode
- Select audio input device

## Configuration

Settings are saved to:
- Windows: `%APPDATA%\freeflow\config.json`

Default configuration:
```json
{
  "hotkey": ["ctrl", "shift", "space"],
  "activation_mode": "push_to_talk",
  "window_position": [100, 100],
  "audio_device": null
}
```

## Modes

- **Push-to-Talk**: Hold the hotkey to record, release to transcribe
- **Toggle**: Press hotkey once to start recording, press again to stop and transcribe

## Troubleshooting

### "Model not ready yet"
Wait for the model to finish loading. First run downloads ~600MB.

### No audio recorded
Check your microphone is connected and selected in Settings.

### Text not appearing
Make sure the target application is focused before releasing the hotkey.

### Slow transcription
GPU is recommended. On CPU, transcription may take longer.

## File Structure

```
freeflow/
├── main.py              # Entry point
├── audio_capture.py     # Microphone recording
├── transcriber.py       # NeMo model wrapper
├── keyboard_output.py   # Text pasting
├── hotkey_manager.py    # Global hotkey handling
├── gui.py               # Floating indicator window
├── settings_dialog.py   # Settings UI
├── config.py            # Configuration management
└── requirements.txt     # Dependencies
```

## License

MIT
