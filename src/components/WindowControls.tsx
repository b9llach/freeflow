import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

const appWindow = getCurrentWindow();

export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      setMaximized(await appWindow.isMaximized());
      unlisten = await appWindow.onResized(async () => {
        setMaximized(await appWindow.isMaximized());
      });
    })();
    return () => {
      unlisten?.();
    };
  }, []);

  return (
    <div className="window-controls">
      <button
        className="window-control"
        onClick={() => appWindow.minimize()}
        aria-label="Minimize"
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
          <line
            x1="2"
            y1="6"
            x2="10"
            y2="6"
            stroke="currentColor"
            strokeWidth="1.2"
            strokeLinecap="round"
          />
        </svg>
      </button>
      <button
        className="window-control"
        onClick={() => appWindow.toggleMaximize()}
        aria-label={maximized ? "Restore" : "Maximize"}
      >
        {maximized ? (
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
            <rect
              x="2.25"
              y="3.75"
              width="6"
              height="6"
              rx="0.5"
              stroke="currentColor"
              strokeWidth="1.1"
            />
            <path
              d="M4 3.5V2.75a.5.5 0 0 1 .5-.5H9.5a.5.5 0 0 1 .5.5V7.5a.5.5 0 0 1-.5.5H8.75"
              stroke="currentColor"
              strokeWidth="1.1"
              fill="none"
              strokeLinecap="round"
            />
          </svg>
        ) : (
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
            <rect
              x="2.25"
              y="2.25"
              width="7.5"
              height="7.5"
              rx="0.5"
              stroke="currentColor"
              strokeWidth="1.1"
            />
          </svg>
        )}
      </button>
      <button
        className="window-control close"
        onClick={() => appWindow.close()}
        aria-label="Close"
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
          <line
            x1="2.5"
            y1="2.5"
            x2="9.5"
            y2="9.5"
            stroke="currentColor"
            strokeWidth="1.2"
            strokeLinecap="round"
          />
          <line
            x1="9.5"
            y1="2.5"
            x2="2.5"
            y2="9.5"
            stroke="currentColor"
            strokeWidth="1.2"
            strokeLinecap="round"
          />
        </svg>
      </button>
    </div>
  );
}
