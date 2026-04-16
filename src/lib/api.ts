import { invoke } from "@tauri-apps/api/core";

export type HotkeyMode = "pushtotalk" | "toggle";
export type Theme = "light" | "dark";
export type PipelineStatus = "idle" | "recording" | "thinking" | "pasted";

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
  getPlatform: () => invoke<string>("get_platform"),
};

export const WHISPER_MODEL_OPTIONS: {
  value: WhisperModelKind;
  label: string;
  size: string;
}[] = [
  { value: "tiny-en", label: "tiny.en (English only, fastest)", size: "~75 MB" },
  { value: "base-en", label: "base.en (English only, recommended)", size: "~142 MB" },
  { value: "small-en", label: "small.en (English only, more accurate)", size: "~466 MB" },
  { value: "medium-en", label: "medium.en (English only, high accuracy)", size: "~1.5 GB" },
  { value: "tiny", label: "tiny (multilingual)", size: "~75 MB" },
  { value: "base", label: "base (multilingual)", size: "~142 MB" },
  { value: "small", label: "small (multilingual)", size: "~466 MB" },
  { value: "medium", label: "medium (multilingual)", size: "~1.5 GB" },
  { value: "large-v3", label: "large-v3 (best quality)", size: "~2.9 GB" },
];
