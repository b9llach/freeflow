import { invoke } from "@tauri-apps/api/core";

export type HotkeyMode = "pushtotalk" | "toggle";
export type Theme = "light" | "dark";
export type PipelineStatus = "idle" | "recording" | "thinking" | "pasted";
export type SttBackend = "whisper" | "parakeet";

export interface Settings {
  ollama_base_url: string;
  ollama_model: string;
  system_prompt: string;
  hotkey: string;
  hotkey_mode: HotkeyMode;
  auto_paste: boolean;
  copy_clipboard: boolean;
  whisper_model_path: string | null;
  whisper_language: string;
  show_indicator: boolean;
  llm_enabled: boolean;
  vocabulary: string[];
  theme: Theme;
  stt_backend: SttBackend;
  parakeet_model_dir: string | null;
}

export interface Transcription {
  id: string;
  created_at: string;
  raw_text: string;
  cleaned_text: string | null;
  audio_path: string | null;
  stt_model: string | null;
  llm_model: string | null;
  duration_ms: number | null;
  meta: string | null;
}

export interface ModelInfo {
  id: string;
  object?: string;
}

export type WhisperModelKind =
  | "tiny-en"
  | "base-en"
  | "small-en"
  | "medium-en"
  | "tiny"
  | "base"
  | "small"
  | "medium"
  | "large-v3";

export interface DownloadProgress {
  name: string;
  downloaded: number;
  total: number | null;
}

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (s: Settings) => invoke<void>("save_settings", { newSettings: s }),
  listOllamaModels: () => invoke<ModelInfo[]>("list_ollama_models"),
  startRecording: () => invoke<void>("start_recording"),
  stopRecording: () => invoke<Transcription | null>("stop_recording"),
  isRecording: () => invoke<boolean>("is_recording"),
  listHistory: (limit = 200) => invoke<Transcription[]>("list_history", { limit }),
  deleteTranscription: (id: string) => invoke<void>("delete_transcription", { id }),
  clearHistory: () => invoke<void>("clear_history"),
  setHotkeyEnabled: (enabled: boolean) => invoke<void>("set_hotkey_enabled", { enabled }),
  pickWhisperModel: (path: string) => invoke<void>("pick_whisper_model", { path }),
  downloadWhisperModel: (model: WhisperModelKind) =>
    invoke<string>("download_whisper_model", { model }),
  downloadParakeetModel: () => invoke<string>("download_parakeet_model"),
  setSttBackend: (backend: SttBackend) =>
    invoke<void>("set_stt_backend", { backend }),
  getPlatform: () => invoke<string>("get_platform"),
  listDownloadedWhisperModels: () =>
    invoke<string[]>("list_downloaded_whisper_models"),
};

export const WHISPER_MODEL_OPTIONS: {
  value: WhisperModelKind;
  label: string;
  size: string;
  filename: string;
}[] = [
  { value: "tiny-en",   label: "tiny.en (English only, fastest)",           size: "~75 MB",  filename: "ggml-tiny.en.bin" },
  { value: "base-en",   label: "base.en (English only, recommended)",       size: "~142 MB", filename: "ggml-base.en.bin" },
  { value: "small-en",  label: "small.en (English only, more accurate)",    size: "~466 MB", filename: "ggml-small.en.bin" },
  { value: "medium-en", label: "medium.en (English only, high accuracy)",   size: "~1.5 GB", filename: "ggml-medium.en.bin" },
  { value: "tiny",      label: "tiny (multilingual)",                       size: "~75 MB",  filename: "ggml-tiny.bin" },
  { value: "base",      label: "base (multilingual)",                       size: "~142 MB", filename: "ggml-base.bin" },
  { value: "small",     label: "small (multilingual)",                      size: "~466 MB", filename: "ggml-small.bin" },
  { value: "medium",    label: "medium (multilingual)",                     size: "~1.5 GB", filename: "ggml-medium.bin" },
  { value: "large-v3",  label: "large-v3 (best quality)",                   size: "~2.9 GB", filename: "ggml-large-v3.bin" },
];

/** Filename portion of a full path on either Windows or POSIX. */
export function basename(path: string | null | undefined): string {
  if (!path) return "";
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || "";
}
