# Freeflow

A Tauri 2 desktop dictation tool. Hold a single hotkey, speak, release — Whisper transcribes locally, Ollama cleans it up, and the result is pasted at your cursor. Think Whisperflow, but with a real LLM pass in the middle so proper nouns, numbers, and punctuation come out right.

![status](https://img.shields.io/badge/platform-Windows-blue) ![stack](https://img.shields.io/badge/stack-Tauri%202%20%7C%20React%20%7C%20Rust-8b5cf6)

## Features

- **Local Whisper STT** via `whisper-rs` / `whisper.cpp` — no cloud, no API keys
- **LLM cleanup through Ollama** using the OpenAI-compatible `/v1/models` and `/v1/chat/completions` endpoints — point it at localhost or any remote box
- **User vocabulary / facts** that get injected into the cleanup prompt so proper nouns and product names always come out spelled right
- **Single-key push-to-talk or toggle mode** with left/right modifier distinction — bind right Ctrl only and left Ctrl won't fire it
- **Floating status indicator** anchored top-right of the monitor that swaps between `Recording` (red), `Thinking` (violet), and `Pasted` (green) as the pipeline moves through its states
- **Auto-paste at cursor via Ctrl+Shift+V** with explicit key sequencing — works reliably in terminals (Windows Terminal, Claude Code), Electron apps (VS Code, Slack, Discord), and browsers
- **SQLite history** of every raw + cleaned transcription, system prompt, and the full LLM message chain
- **Custom window chrome** — OS titlebar is stripped and replaced with in-app controls plus a draggable header
- **System tray icon** — closing the window hides to tray so the hotkey keeps working; left-click the tray icon to reopen
- **Dark + light theme** with a real dual-palette color system, not an inverted hack
- **Eager model preload + silent-sample warmup** on startup so the first hotkey press is fast, not painful
- **In-app Whisper model downloader** — pick any of the official ggml models and it fetches them from Hugging Face with a progress bar, then auto-loads
- **Pluggable pipeline** (`SttEngine`, `LlmProvider`, `OutputSink` traits) so voice assistant / TTS modes slot in without touching capture or the hotkey

## Screenshots

> Add screenshots here (`docs/screenshot-dark.png`, `docs/screenshot-light.png`) once you have them.

## Requirements

- Windows 10 / 11, or macOS 10.15+
- [Rust](https://rustup.rs/) 1.77+
- Node 18+
- An [Ollama](https://ollama.com) instance reachable over HTTP
- A Whisper `ggml-*.bin` model — Freeflow can download one for you on first run

## Quick start

```bash
git clone https://github.com/b9llach/freeflow.git
cd freeflow
npm install
npm run tauri dev
```

On first launch:

1. Open the settings rail on the right
2. **Ollama** → set the Base URL (default `http://localhost:11434`) and click Refresh to list models via `/v1/models`
3. **Whisper** → choose a model from the dropdown and click Download — the app streams it from the official `ggerganov/whisper.cpp` Hugging Face repo with a live progress bar and auto-loads it once done
4. **Hotkey** → click the trigger key field and press the key you want (e.g. right Ctrl). Pick push-to-talk (hold) or toggle mode
5. **Vocabulary** (optional) → add any proper nouns, product names, or facts that the cleanup model should know about. These get injected into the system prompt
6. Hold your hotkey anywhere on the desktop, speak, release. The text lands in whatever window has focus

## Hotkey

The hotkey capture uses a low-level `rdev` keyboard hook on Windows, so `ControlLeft` vs `ControlRight`, `ShiftLeft` vs `ShiftRight`, `Alt` vs `AltGr`, and `MetaLeft` vs `MetaRight` are all distinct. Most global-hotkey APIs (including Win32 `RegisterHotKey`) can't tell them apart — this one can.

- **Push-to-talk:** press and hold to record, release to stop and transcribe
- **Toggle:** first press starts, second press stops

## Whisper models

Models come from https://huggingface.co/ggerganov/whisper.cpp — the same files whisper.cpp uses. English-only variants (`.en`) are faster and more accurate for English than the multilingual equivalents at the same size.

| Model       | Size    | Notes                                      |
| ----------- | ------- | ------------------------------------------ |
| `tiny.en`   | ~75 MB  | Fastest, rough accuracy                    |
| `base.en`   | ~142 MB | Good balance — default recommendation      |
| `small.en`  | ~466 MB | Noticeably more accurate                   |
| `medium.en` | ~1.5 GB | High quality, slower                       |
| `large-v3`  | ~2.9 GB | Best quality, multilingual, slowest        |

Downloaded models land in `%APPDATA%/com.freeflow.app/models/ggml-*.bin`. You can also point at an existing `ggml-*.bin` from another whisper.cpp install via the Browse button.

On startup, Freeflow loads the model on a background thread and then runs a short silent transcription to force the weights into RAM and warm the internal caches — so the first real hotkey press doesn't pay the mmap page-in cost.

## Paste reliability

Freeflow sends **Ctrl+Shift+V** with raw `VK_V` scancodes (not `Key::Unicode`), with explicit gapped key press/release timing. Ctrl+Shift+V is the "paste as plain text" shortcut in Windows Terminal, VS Code, Chrome, Slack, Discord, and most Electron apps, and it lands far more reliably than plain Ctrl+V — especially in terminals. Since you're pasting raw text anyway, stripping any inherited formatting is exactly what you want.

If you have Copy-to-clipboard disabled, the original clipboard is restored on a detached thread after 450ms so slow async paste readers aren't raced.

## Status states

The pipeline moves through four states and both the header pill and the floating indicator reflect them:

| State      | Color  | When                                                       |
| ---------- | ------ | ---------------------------------------------------------- |
| `Ready`    | green  | Idle                                                       |
| `Recording`| red    | Hotkey held / toggle on. Red dot pulses.                   |
| `Thinking` | violet | STT + LLM post-processing running.                         |
| `Pasted`   | green  | Paste sink reported success. Fades back to Ready after 1.5s. |

## System tray

Closing the main window does not quit the app — it hides to the system tray (notification area) so the global hotkey keeps working. Left-click the tray icon to reopen the window. Right-click for `Open Freeflow` / `Pause hotkey` / `Quit`.

## Build

```bash
npm run tauri build
```

On Windows this produces:

- `src-tauri/target/release/freeflow2.exe` — standalone release exe
- `src-tauri/target/release/bundle/msi/Freeflow_<version>_x64_en-US.msi` — MSI installer
- `src-tauri/target/release/bundle/nsis/Freeflow_<version>_x64-setup.exe` — NSIS installer

On macOS this produces:

- `src-tauri/target/release/bundle/macos/Freeflow.app` — app bundle
- `src-tauri/target/release/bundle/dmg/Freeflow_<version>_x64.dmg` — installer disk image (Intel)
  or `_aarch64.dmg` (Apple Silicon)

### macOS first-launch setup

After installing, launch Freeflow once and then grant two permissions before the hotkey and transcription will work:

1. **Microphone** — macOS will prompt automatically the first time you press the hotkey. Approve it.
2. **Accessibility** — open `System Settings → Privacy & Security → Accessibility`, click the `+` button, and add `Freeflow.app`. This is what lets Freeflow install the global keyboard hook for push-to-talk and send the Cmd+V keystroke to paste into the focused window. Without it, holding your hotkey does nothing.

You may need to quit and relaunch Freeflow after granting Accessibility.

On macOS the paste shortcut is `Cmd+V` (Freeflow detects the platform at runtime and sends the right combo). The native traffic-light buttons stay in the top-left; the custom Windows min/max/close buttons are automatically hidden.

## Icon generation

The app icon is a single SVG (`src-tauri/icons/icon.svg`) — a white circle with four violet soundwave bars. Rasters for every Windows DPI scaling factor (16 / 20 / 24 / 32 / 40 / 48 / 56 / 64 / 96 / 128 / 256) plus the multi-frame `icon.ico` are generated by a zero-dependency PowerShell + GDI+ script:

```powershell
.\scripts\generate-icon.ps1
```

Re-run it any time you want to tweak the design.

## SQL schema

The app uses a local SQLite database at `%APPDATA%/com.freeflow.app/freeflow.sqlite`.

```sql
CREATE TABLE IF NOT EXISTS transcriptions (
    id           TEXT PRIMARY KEY,
    created_at   TEXT NOT NULL,
    raw_text     TEXT NOT NULL,
    cleaned_text TEXT,
    audio_path   TEXT,
    stt_model    TEXT,
    llm_model    TEXT,
    duration_ms  INTEGER,
    meta         TEXT
);

CREATE INDEX IF NOT EXISTS idx_transcriptions_created_at
    ON transcriptions(created_at DESC);

CREATE TABLE IF NOT EXISTS llm_messages (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    transcription_id TEXT NOT NULL,
    role             TEXT NOT NULL,
    content          TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    FOREIGN KEY(transcription_id) REFERENCES transcriptions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_llm_messages_tx
    ON llm_messages(transcription_id);
```

Neon / Postgres equivalent:

```sql
CREATE TABLE IF NOT EXISTS transcriptions (
    id           TEXT PRIMARY KEY,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    raw_text     TEXT NOT NULL,
    cleaned_text TEXT,
    audio_path   TEXT,
    stt_model    TEXT,
    llm_model    TEXT,
    duration_ms  BIGINT,
    meta         JSONB
);

CREATE INDEX IF NOT EXISTS idx_transcriptions_created_at
    ON transcriptions(created_at DESC);

CREATE TABLE IF NOT EXISTS llm_messages (
    id               BIGSERIAL PRIMARY KEY,
    transcription_id TEXT NOT NULL REFERENCES transcriptions(id) ON DELETE CASCADE,
    role             TEXT NOT NULL,
    content          TEXT NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_llm_messages_tx
    ON llm_messages(transcription_id);
```

## Architecture

```
src-tauri/src/
  audio.rs       cpal capture on a dedicated thread -> f32 mono 16kHz
  stt/
    mod.rs       SttEngine trait
    whisper.rs   whisper-rs impl with background load + silent warmup
  llm/
    mod.rs       LlmProvider trait, ChatMessage, ModelInfo
    ollama.rs    Ollama client over OpenAI-compatible endpoints
  output/
    mod.rs       OutputSink trait
    paste.rs     Ctrl+Shift+V paste via enigo + raw VK_V scancode
    clipboard.rs Clipboard write via tauri-plugin-clipboard-manager
  hotkey.rs      rdev low-level keyboard hook, L/R modifier aware
  indicator.rs   Show/hide the floating indicator window
  pipeline.rs    Orchestrates record -> stt -> llm -> outputs -> db, emits status
  db.rs          SQLite layer (rusqlite, bundled)
  settings.rs    Persisted user config (JSON at app_config_dir)
  commands.rs    Tauri command surface
  lib.rs         App bootstrap, tray, window events, DI
src/
  App.tsx                Main window, status state, history + settings layout
  components/
    BrandMark.tsx        SVG soundwave brand mark
    WindowControls.tsx   Custom min / max / close buttons
    HistoryList.tsx      Flat transcription cards
    SettingsPanel.tsx    Hotkey, Ollama, Vocabulary, Whisper, Output, Appearance
    VocabularyEditor.tsx Free-form fact list
    HotkeyCapture.tsx    Click-to-capture that disambiguates L/R modifiers
  indicator.ts           Tiny script that swaps the indicator dot + label
  styles.css             Dark + light themes via CSS variables
```

### Extending

To add a voice-assistant mode:
- Add a new `LlmProvider` variant or an `AssistantMode` enum in `settings.rs`
- Add a TTS output sink implementing `OutputSink` (e.g. `output/tts.rs` using Piper or ElevenLabs)
- Branch in `Pipeline::stop_and_process` based on mode: dictation (current) or assistant (feed the cleaned text as a query to the LLM and pipe the reply into the TTS sink)

The trait boundaries and DI in `lib.rs` mean these slot in without touching audio capture, the hotkey, the indicator, or the history layer.

## License

MIT
