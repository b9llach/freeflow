import { useEffect, useState } from "react";
import {
  api,
  DownloadProgress,
  ModelInfo,
  Settings,
  WHISPER_MODEL_OPTIONS,
  WhisperModelKind,
} from "../lib/api";
import { HotkeyCapture } from "./HotkeyCapture";
import { VocabularyEditor } from "./VocabularyEditor";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";

interface Props {
  settings: Settings;
  onChange: (s: Settings) => void;
  onClearHistory: () => void;
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function SettingsPanel({ settings, onChange, onClearHistory }: Props) {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [modelErr, setModelErr] = useState<string | null>(null);
  const [loadingModels, setLoadingModels] = useState(false);
  const [downloadKind, setDownloadKind] = useState<WhisperModelKind>("base-en");
  const [downloading, setDownloading] = useState(false);
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [downloadErr, setDownloadErr] = useState<string | null>(null);

  const refreshModels = async () => {
    setLoadingModels(true);
    setModelErr(null);
    try {
      const m = await api.listOllamaModels();
      setModels(m);
    } catch (e) {
      setModelErr(String(e));
      setModels([]);
    } finally {
      setLoadingModels(false);
    }
  };

  useEffect(() => {
    refreshModels();
  }, [settings.ollama_base_url]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      unlisten = await listen<DownloadProgress>("freeflow://download-progress", (e) => {
        setProgress(e.payload);
      });
    })();
    return () => {
      unlisten?.();
    };
  }, []);

  const update = <K extends keyof Settings>(k: K, v: Settings[K]) =>
    onChange({ ...settings, [k]: v });

  const pickWhisperModel = async () => {
    const picked = await open({
      multiple: false,
      filters: [{ name: "Whisper model", extensions: ["bin", "gguf"] }],
    });
    if (typeof picked === "string") {
      update("whisper_model_path", picked);
    }
  };

  const handleDownload = async () => {
    setDownloading(true);
    setDownloadErr(null);
    setProgress(null);
    try {
      const path = await api.downloadWhisperModel(downloadKind);
      update("whisper_model_path", path);
    } catch (e) {
      setDownloadErr(String(e));
    } finally {
      setDownloading(false);
      setTimeout(() => setProgress(null), 1800);
    }
  };

  return (
    <aside className="rail">
      <div className="rail-section">
        <h2>Appearance</h2>
        <div className="field">
          <label>Theme</label>
          <div className="seg">
            <button
              className={settings.theme === "light" ? "active" : ""}
              onClick={() => update("theme", "light")}
            >
              Light
            </button>
            <button
              className={settings.theme === "dark" ? "active" : ""}
              onClick={() => update("theme", "dark")}
            >
              Dark
            </button>
          </div>
        </div>
      </div>

      <div className="rail-section">
        <h2>Hotkey</h2>
        <div className="field">
          <label>Trigger key</label>
          <HotkeyCapture
            value={settings.hotkey}
            onChange={(k) => update("hotkey", k)}
          />
        </div>
        <div className="field">
          <label>Mode</label>
          <select
            value={settings.hotkey_mode}
            onChange={(e) =>
              update("hotkey_mode", e.target.value as Settings["hotkey_mode"])
            }
          >
            <option value="pushtotalk">Hold to talk</option>
            <option value="toggle">Toggle on / off</option>
          </select>
        </div>
      </div>

      <div className="rail-section">
        <h2>Ollama</h2>
        <div className="field">
          <label>Base URL</label>
          <input
            type="text"
            value={settings.ollama_base_url}
            onChange={(e) => update("ollama_base_url", e.target.value)}
            placeholder="http://localhost:11434"
          />
        </div>
        <div className="field">
          <label>Model</label>
          <div className="row">
            <select
              value={settings.ollama_model}
              onChange={(e) => update("ollama_model", e.target.value)}
            >
              {models.length === 0 && (
                <option value={settings.ollama_model}>{settings.ollama_model}</option>
              )}
              {models.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.id}
                </option>
              ))}
            </select>
            <button
              className="btn small fit"
              onClick={refreshModels}
              disabled={loadingModels}
            >
              {loadingModels ? "..." : "Refresh"}
            </button>
          </div>
          {modelErr && <div className="field-error">{modelErr}</div>}
        </div>
        <div className="switch">
          <div className="label-stack">
            <span>LLM cleanup</span>
            <span className="sub">Rewrite raw transcripts through the selected model</span>
          </div>
          <input
            type="checkbox"
            checked={settings.llm_enabled}
            onChange={(e) => update("llm_enabled", e.target.checked)}
          />
        </div>
        <div className="field" style={{ marginTop: 14 }}>
          <label>System prompt</label>
          <textarea
            value={settings.system_prompt}
            onChange={(e) => update("system_prompt", e.target.value)}
          />
        </div>
      </div>

      <div className="rail-section">
        <h2>Vocabulary</h2>
        <VocabularyEditor
          items={settings.vocabulary}
          onChange={(next) => update("vocabulary", next)}
        />
      </div>

      <div className="rail-section">
        <h2>Whisper</h2>
        <div className="field">
          <label>Model file</label>
          <div className="row">
            <input
              type="text"
              value={settings.whisper_model_path ?? ""}
              onChange={(e) => update("whisper_model_path", e.target.value || null)}
              placeholder="ggml-base.en.bin"
            />
            <button className="btn small fit" onClick={pickWhisperModel}>
              Browse
            </button>
          </div>
        </div>
        <div className="field">
          <label>Download from Hugging Face</label>
          <div className="row">
            <select
              value={downloadKind}
              onChange={(e) => setDownloadKind(e.target.value as WhisperModelKind)}
              disabled={downloading}
            >
              {WHISPER_MODEL_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label} — {o.size}
                </option>
              ))}
            </select>
            <button
              className="btn small primary fit"
              onClick={handleDownload}
              disabled={downloading}
            >
              {downloading ? "..." : "Download"}
            </button>
          </div>
          {progress && (
            <>
              <div className="progress">
                <div
                  className="progress-bar"
                  style={{
                    width: progress.total
                      ? `${Math.min(100, (progress.downloaded / progress.total) * 100)}%`
                      : "5%",
                  }}
                />
              </div>
              <div className="progress-meta">
                <span>{formatBytes(progress.downloaded)}</span>
                {progress.total && <span>{formatBytes(progress.total)}</span>}
              </div>
            </>
          )}
          {downloadErr && <div className="field-error">{downloadErr}</div>}
        </div>
        <div className="field">
          <label>Language</label>
          <input
            type="text"
            value={settings.whisper_language}
            onChange={(e) => update("whisper_language", e.target.value)}
            placeholder="en"
          />
        </div>
      </div>

      <div className="rail-section">
        <h2>Output</h2>
        <div className="switch">
          <div className="label-stack">
            <span>Auto-paste at cursor</span>
            <span className="sub">Types the cleaned text into the focused window</span>
          </div>
          <input
            type="checkbox"
            checked={settings.auto_paste}
            onChange={(e) => update("auto_paste", e.target.checked)}
          />
        </div>
        <div className="switch">
          <div className="label-stack">
            <span>Copy to clipboard</span>
            <span className="sub">Also leave the result on the clipboard</span>
          </div>
          <input
            type="checkbox"
            checked={settings.copy_clipboard}
            onChange={(e) => update("copy_clipboard", e.target.checked)}
          />
        </div>
        <div className="switch">
          <div className="label-stack">
            <span>Floating indicator</span>
            <span className="sub">Show the pulsing dot while recording</span>
          </div>
          <input
            type="checkbox"
            checked={settings.show_indicator}
            onChange={(e) => update("show_indicator", e.target.checked)}
          />
        </div>
      </div>

      <div className="rail-section">
        <h2>History</h2>
        <button className="btn danger" onClick={onClearHistory}>
          Clear all history
        </button>
      </div>
    </aside>
  );
}
