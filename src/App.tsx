import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { api, PipelineStatus, Settings, Transcription } from "./lib/api";
import { HistoryList } from "./components/HistoryList";
import { SettingsPanel } from "./components/SettingsPanel";
import { BrandMark } from "./components/BrandMark";
import { WindowControls } from "./components/WindowControls";

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [history, setHistory] = useState<Transcription[]>([]);
  const [status, setStatus] = useState<PipelineStatus>("idle");
  const [toast, setToast] = useState<string | null>(null);
  const [platform, setPlatform] = useState<string>("windows");
  const toastTimer = useRef<number | null>(null);

  const showToast = useCallback((msg: string) => {
    setToast(msg);
    if (toastTimer.current) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 2600);
  }, []);

  useEffect(() => {
    (async () => {
      try {
        const s = await api.getSettings();
        setSettings(s);
        const h = await api.listHistory();
        setHistory(h);
        const r = await api.isRecording();
        setStatus(r ? "recording" : "idle");
      } catch (e) {
        showToast(String(e));
      }
    })();
  }, [showToast]);

  useEffect(() => {
    if (settings?.theme) {
      document.documentElement.dataset.theme = settings.theme;
    }
  }, [settings?.theme]);

  useEffect(() => {
    (async () => {
      try {
        const p = await api.getPlatform();
        setPlatform(p);
        document.documentElement.dataset.os = p;
      } catch {
        // fall back to windows layout
        document.documentElement.dataset.os = "windows";
      }
    })();
  }, []);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    (async () => {
      const a = await listen<PipelineStatus>("freeflow://status", (e) => {
        setStatus(e.payload);
      });
      const b = await listen<Transcription>("freeflow://transcription", async () => {
        const h = await api.listHistory();
        setHistory(h);
      });
      const c = await listen<string>("freeflow://toast", (e) => showToast(e.payload));
      unlisteners.push(a, b, c);
    })();
    return () => {
      unlisteners.forEach((u) => u());
    };
  }, [showToast]);

  const updateSettings = useCallback(
    async (s: Settings) => {
      setSettings(s);
      try {
        await api.saveSettings(s);
      } catch (e) {
        showToast(String(e));
      }
    },
    [showToast]
  );

  const handleCopy = useCallback(
    async (text: string) => {
      try {
        await writeText(text);
        showToast("Copied to clipboard");
      } catch (e) {
        showToast(String(e));
      }
    },
    [showToast]
  );

  const handleDelete = useCallback(async (id: string) => {
    await api.deleteTranscription(id);
    setHistory((h) => h.filter((t) => t.id !== id));
  }, []);

  const handleClearHistory = useCallback(async () => {
    if (!confirm("Delete all transcription history?")) return;
    await api.clearHistory();
    setHistory([]);
  }, []);

  return (
    <div className="app">
      <header className="header" data-tauri-drag-region>
        <div className="brand" data-tauri-drag-region>
          <BrandMark />
          <h1>Freeflow</h1>
        </div>
        <div className="header-right" data-tauri-drag-region>
          {settings && (
            <div className="status" data-tauri-drag-region>
              <span className="status-meta" data-tauri-drag-region>
                {settings.hotkey_mode === "pushtotalk" ? "Hold" : "Press"}
                <kbd>{prettyKey(settings.hotkey)}</kbd>
              </span>
              <span className={`pill ${statusClass(status)}`}>
                <span className="pill-dot" />
                {statusLabel(status)}
              </span>
            </div>
          )}
          {platform !== "macos" && <WindowControls />}
        </div>
      </header>

      <main className="main">
        <div className="section-label">
          <h2>History</h2>
          <span className="count">
            {history.length.toString().padStart(2, "0")}
          </span>
        </div>
        {history.length === 0 ? (
          <div className="empty">
            <h3>Speak to begin.</h3>
            <p>
              Press and hold your push-to-talk key, say what you need, and Freeflow
              will transcribe it locally and clean it up through your Ollama model.
            </p>
            {settings && (
              <div className="hint">
                Hotkey <kbd>{prettyKey(settings.hotkey)}</kbd>
              </div>
            )}
          </div>
        ) : (
          <div className="list">
            <HistoryList items={history} onCopy={handleCopy} onDelete={handleDelete} />
          </div>
        )}
      </main>

      {settings && (
        <SettingsPanel
          settings={settings}
          onChange={updateSettings}
          onClearHistory={handleClearHistory}
        />
      )}

      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}

function statusLabel(s: PipelineStatus): string {
  switch (s) {
    case "recording":
      return "Recording";
    case "thinking":
      return "Thinking";
    case "pasted":
      return "Pasted";
    default:
      return "Ready";
  }
}

function statusClass(s: PipelineStatus): string {
  switch (s) {
    case "recording":
      return "on";
    case "thinking":
      return "thinking";
    case "pasted":
      return "pasted";
    default:
      return "ok";
  }
}

function prettyKey(k: string): string {
  return k
    .replace(/Left$/, " L")
    .replace(/Right$/, " R")
    .replace("ControlL", "Ctrl L")
    .replace("ControlR", "Ctrl R")
    .replace("ShiftL", "Shift L")
    .replace("ShiftR", "Shift R")
    .replace("MetaL", "Meta L")
    .replace("MetaR", "Meta R");
}
