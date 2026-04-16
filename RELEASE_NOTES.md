# v0.1.1 — Reliable push-to-talk

Bugfix release focused on making push-to-talk rock-solid on a cold start.

## Fixed

- **Push-to-talk no longer stalls on the first press.** Whisper's weights are mmap'd at load time, so on the initial real transcription the OS had to page the entire model into RAM — a 1-3 second stall that happened right when you needed the app to feel snappy. The model now runs a silent 500ms warmup transcription during startup that forces every weight into memory and fills CPU caches before the hotkey ever fires.
- **Pipeline errors no longer leave the UI stuck.** Every early-return path in `stop_and_process` now routes through `finish_idle()` and emits a user-facing toast with the actual error message. Previously, a failed transcription (or any `?` bail) skipped the cleanup and left the status indicator stranded on "Thinking" with no feedback.
- **Detects an unloaded model upfront** so you get a clear "Whisper model is still loading, try again in a moment" toast instead of silence.

## Install

- **`Freeflow_0.1.1_x64-setup.exe`** — Windows installer (recommended)
- **`Freeflow_0.1.1_x64_en-US.msi`** — MSI for managed deployments

If upgrading from 0.1.0, your settings (hotkey, theme, vocabulary, Ollama URL, Whisper model path) all persist automatically.

## Full changelog

https://github.com/b9llach/freeflow/compare/v0.1.0...v0.1.1
