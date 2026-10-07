import { useEffect, useMemo, useState } from "react";
import {
  api,
  basename,
  DownloadProgress,
  InputDeviceInfo,
  ModelInfo,
  PhononProgress,
  PhononStatus,
  Settings,
  SttBackend,
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

  const [switchingBackend, setSwitchingBackend] = useState(false);
  const [backendErr, setBackendErr] = useState<string | null>(null);
  const [testingServer, setTestingServer] = useState(false);
  const [serverTest, setServerTest] = useState<{ ok: boolean; msg: string } | null>(
    null
  );

  const [phonon, setPhonon] = useState<PhononStatus | null>(null);
  const [phononBusy, setPhononBusy] = useState(false);
  const [phononStep, setPhononStep] = useState<PhononProgress | null>(null);
  const [phononErr, setPhononErr] = useState<string | null>(null);

  const refreshPhonon = async () => {
    try {
      setPhonon(await api.phononStatus());
    } catch (e) {
      setPhononErr(String(e));
    }
  };

  useEffect(() => {
    refreshPhonon();
    let un: (() => void) | undefined;
    listen<PhononProgress>("freeflow://phonon-setup", (ev) =>
      setPhononStep(ev.payload)
    ).then((f) => (un = f));
    return () => un?.();
  }, []);

  const handlePhononInstall = async () => {
    setPhononErr(null);
    setPhononBusy(true);
    setPhononStep(null);
    try {
      await api.phononInstall();
      onChange({ ...settings, stt_backend: "phonon" });
    } catch (e) {
      setPhononErr(String(e));
    } finally {
      setPhononBusy(false);
      refreshPhonon();
    }
  };

  const handlePhononUninstall = async () => {
    if (!window.confirm("Remove Phonon-2 and free its disk space (about 1.6 GB)?")) return;
    setPhononErr(null);
    setPhononBusy(true);
    try {
      await api.phononUninstall();
      if (settings.stt_backend === "phonon") {
        onChange({ ...settings, stt_backend: "whisper" });
      }
    } catch (e) {
      setPhononErr(String(e));
    } finally {
      setPhononBusy(false);
      setPhononStep(null);
      refreshPhonon();
    }
  };

  // Filenames of Whisper models already sitting in the app models dir.
  const [downloadedWhisper, setDownloadedWhisper] = useState<string[]>([]);

  // Available cpal input devices, with the system default flagged.
  const [inputDevices, setInputDevices] = useState<InputDeviceInfo[]>([]);
  const [inputDevicesErr, setInputDevicesErr] = useState<string | null>(null);
  const [loadingInputs, setLoadingInputs] = useState(false);

  const refreshInputDevices = async () => {
    setLoadingInputs(true);
    setInputDevicesErr(null);
    try {
      const list = await api.listInputDevices();
      setInputDevices(list);
    } catch (e) {
      setInputDevicesErr(String(e));
      setInputDevices([]);
    } finally {
      setLoadingInputs(false);
    }
  };

  useEffect(() => {
    refreshInputDevices();
  }, []);

  const refreshDownloadedWhisper = async () => {
    try {
      const list = await api.listDownloadedWhisperModels();
      setDownloadedWhisper(list);
    } catch {
      setDownloadedWhisper([]);
    }
  };

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

  // Refresh the downloaded list on mount and whenever the current whisper
  // model path changes (e.g. after a fresh download or a manual pick).
  useEffect(() => {
    refreshDownloadedWhisper();
  }, [settings.whisper_model_path]);

  // Auto-select the download dropdown to match whatever whisper model is
  // currently loaded, so the button reflects "Active" for it instead of
  // silently sitting on the default "base-en" option.
  useEffect(() => {
    const current = basename(settings.whisper_model_path);
    if (!current) return;
    const match = WHISPER_MODEL_OPTIONS.find((o) => o.filename === current);
    if (match && match.value !== downloadKind) {
      setDownloadKind(match.value);
    }
    // We deliberately don't depend on downloadKind here — otherwise the user
    // couldn't manually switch to a different option in the dropdown.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.whisper_model_path]);

  const selectedOption = useMemo(
    () => WHISPER_MODEL_OPTIONS.find((o) => o.value === downloadKind),
    [downloadKind]
  );
  const currentWhisperFilename = basename(settings.whisper_model_path);
  const isSelectedActive =
    !!selectedOption && selectedOption.filename === currentWhisperFilename;
  const isSelectedDownloaded =
    !!selectedOption && downloadedWhisper.includes(selectedOption.filename);

  const downloadButtonLabel = downloading
    ? "..."
    : isSelectedActive
      ? "Active"
      : isSelectedDownloaded
        ? "Use downloaded"
        : "Download";

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
      // If the file already existed the backend skips the fetch, still loads
      // it, and returns the path — so this codepath handles both "download"
      // and "use already-downloaded" in one shot.
      update("whisper_model_path", path);
      await refreshDownloadedWhisper();
    } catch (e) {
      setDownloadErr(String(e));
    } finally {
      setDownloading(false);
      setTimeout(() => setProgress(null), 1800);
    }
  };

  const handleBackendChange = async (backend: SttBackend) => {
    if (backend === settings.stt_backend) return;
    setBackendErr(null);
    setSwitchingBackend(true);
    try {
      // The backend persists the choice and swaps the live engine. Local
      // state only follows once that succeeded, so the UI never claims an
      // engine that failed to load.
      await api.setSttBackend(backend);
      onChange({ ...settings, stt_backend: backend });
    } catch (e) {
      setBackendErr(String(e));
    } finally {
      setSwitchingBackend(false);
    }
  };

  const handleTestServer = async () => {
    setTestingServer(true);
    setServerTest(null);
    try {
      const id = await api.testSttServer(settings.stt_http_url);
      setServerTest({ ok: true, msg: `Connected. Serving ${id}` });
    } catch (e) {
      setServerTest({ ok: false, msg: String(e) });
    } finally {
      setTestingServer(false);
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
        <h2>Speech to text</h2>
        <div className="field">
          <label>Microphone</label>
          <div className="row">
            <select
              value={settings.input_device ?? ""}
              onChange={(e) =>
                update("input_device", e.target.value || null)
              }
            >
              <option value="">
                {(() => {
                  const def = inputDevices.find((d) => d.is_default);
                  return def
                    ? `System default (${def.name})`
                    : "System default";
                })()}
              </option>
              {inputDevices.map((d) => (
                <option key={d.name} value={d.name}>
                  {d.name}
                  {d.is_default ? " · default" : ""}
                </option>
              ))}
            </select>
            <button
              className="btn small fit"
              onClick={refreshInputDevices}
              disabled={loadingInputs}
            >
              {loadingInputs ? "..." : "Refresh"}
            </button>
          </div>
          {inputDevicesErr && (
            <div className="field-error">{inputDevicesErr}</div>
          )}
          {inputDevices.length === 0 && !loadingInputs && !inputDevicesErr && (
            <div className="field-hint">
              No input devices detected. On macOS, grant Freeflow microphone
              access in System Settings and click Refresh.
            </div>
          )}
        </div>
        <div className="field">
          <label>Engine</label>
          <div className="seg">
            <button
              className={settings.stt_backend === "whisper" ? "active" : ""}
              onClick={() => handleBackendChange("whisper")}
              disabled={switchingBackend || phononBusy}
            >
              Whisper
            </button>
            <button
              className={settings.stt_backend === "phonon" ? "active" : ""}
              onClick={() => handleBackendChange("phonon")}
              disabled={switchingBackend || phononBusy}
            >
              Phonon-2
            </button>
            <button
              className={settings.stt_backend === "http" ? "active" : ""}
              onClick={() => handleBackendChange("http")}
              disabled={switchingBackend || phononBusy}
            >
              Remote
            </button>
          </div>
          {backendErr && <div className="field-error">{backendErr}</div>}
          <div className="field-hint">
            Whisper runs inside the app. Phonon-2 is fast and light on the
            CPU; Freeflow sets it up and runs it for you. Remote connects to a
            speech server you run yourself.
          </div>
        </div>

        {settings.stt_backend === "phonon" && (
          <div className="field">
            <label>Phonon-2</label>
            {phonon && !phonon.supported && (
              <div className="field-error">
                Automatic setup is not available on this system.
              </div>
            )}
            {phonon?.supported && phonon.installed && !phononBusy && (
              <div className="field-hint">
                {phonon.running
                  ? "Installed and running."
                  : "Installed. The engine starts on first use."}
              </div>
            )}
            {phonon?.supported && !phonon.installed && !phononBusy && (
              <div className="field-hint">
                Phonon-2 is not set up yet. Setup downloads about 700 MB
                (a private Python and the speech engine) plus a 160 MB model,
                and needs no other software. It only has to run once.
              </div>
            )}
            {phononBusy && (
              <>
                <div className="field-hint">
                  {phononStep?.message ?? "Starting setup"}
                </div>
                <div className="progress">
                  <div
                    className="progress-bar"
                    style={{
                      width:
                        phononStep?.percent != null
                          ? `${Math.min(100, phononStep.percent)}%`
                          : "30%",
                    }}
                  />
                </div>
                {phononStep?.detail && (
                  <div className="field-hint">{phononStep.detail}</div>
                )}
              </>
            )}
            {phononErr && (
              <div className="field-error" style={{ whiteSpace: "pre-wrap" }}>
                {phononErr}
              </div>
            )}
            <div className="row" style={{ marginTop: 8 }}>
              {phonon?.supported && !phonon.installed && (
                <button
                  className="btn small primary"
                  onClick={handlePhononInstall}
                  disabled={phononBusy}
                >
                  {phononBusy ? "Setting up..." : phononErr ? "Retry setup" : "Set up Phonon-2"}
                </button>
              )}
              {phonon?.installed && (
                <button
                  className="btn small"
                  onClick={handlePhononUninstall}
                  disabled={phononBusy}
                >
                  Remove
                </button>
              )}
            </div>
          </div>
        )}

        {settings.stt_backend === "whisper" && (
          <>
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
              {WHISPER_MODEL_OPTIONS.map((o) => {
                const dl = downloadedWhisper.includes(o.filename);
                const active = o.filename === currentWhisperFilename;
                const tag = active ? " · active" : dl ? " · downloaded" : "";
                return (
                  <option key={o.value} value={o.value}>
                    {o.label} — {o.size}
                    {tag}
                  </option>
                );
              })}
            </select>
            <button
              className="btn small primary fit"
              onClick={handleDownload}
              disabled={downloading || isSelectedActive}
              title={
                isSelectedActive
                  ? "Currently loaded model"
                  : isSelectedDownloaded
                    ? "Model already downloaded — load it"
                    : "Download from Hugging Face"
              }
            >
              {downloadButtonLabel}
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
          </>
        )}

        {settings.stt_backend === "http" && (
          <>
            <div className="field">
              <label>Server URL</label>
              <div className="row">
                <input
                  type="text"
                  value={settings.stt_http_url}
                  onChange={(e) => update("stt_http_url", e.target.value)}
                  placeholder="http://127.0.0.1:8000/v1"
                />
                <button
                  className="btn small fit"
                  onClick={handleTestServer}
                  disabled={testingServer}
                >
                  {testingServer ? "..." : "Test"}
                </button>
              </div>
              {serverTest && (
                <div className={serverTest.ok ? "field-hint" : "field-error"}>
                  {serverTest.msg}
                </div>
              )}
            </div>
            <div className="field">
              <label>Model</label>
              <input
                type="text"
                value={settings.stt_http_model}
                onChange={(e) => update("stt_http_model", e.target.value)}
                placeholder="phonon-2"
              />
              <div className="field-hint">
                Start the server with <code>fermion serve phonon-2</code>. Any
                OpenAI-compatible <code>/audio/transcriptions</code> server
                works, local or on another machine.
              </div>
            </div>
          </>
        )}
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
