import { useEffect, useRef, useState } from "react";

interface Props {
  value: string;
  onChange: (key: string) => void;
}

function eventToKeyName(e: KeyboardEvent): string | null {
  if (e.code.startsWith("Control")) {
    return e.location === 2 ? "ControlRight" : "ControlLeft";
  }
  if (e.code.startsWith("Shift")) {
    return e.location === 2 ? "ShiftRight" : "ShiftLeft";
  }
  if (e.code === "AltLeft") return "Alt";
  if (e.code === "AltRight") return "AltGr";
  if (e.code === "MetaLeft" || e.code === "OSLeft") return "MetaLeft";
  if (e.code === "MetaRight" || e.code === "OSRight") return "MetaRight";
  if (e.code === "Space") return "Space";
  if (e.code === "Tab") return "Tab";
  if (e.code === "Escape") return "Escape";
  if (e.code === "CapsLock") return "CapsLock";
  if (/^F\d{1,2}$/.test(e.code)) return e.code;
  if (/^Key[A-Z]$/.test(e.code)) return e.code.slice(3);
  if (/^Digit\d$/.test(e.code)) return e.code.slice(5);
  return null;
}

function pretty(name: string): string {
  return name
    .replace("ControlLeft", "Ctrl L")
    .replace("ControlRight", "Ctrl R")
    .replace("ShiftLeft", "Shift L")
    .replace("ShiftRight", "Shift R")
    .replace("MetaLeft", "Meta L")
    .replace("MetaRight", "Meta R");
}

export function HotkeyCapture({ value, onChange }: Props) {
  const [capturing, setCapturing] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!capturing) return;
    const handler = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const name = eventToKeyName(e);
      if (name) {
        onChange(name);
        setCapturing(false);
      }
    };
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [capturing, onChange]);

  return (
    <div
      ref={ref}
      className={`hotkey-capture ${capturing ? "capturing" : ""}`}
      onClick={() => setCapturing(true)}
      tabIndex={0}
    >
      {capturing ? "Press a key..." : pretty(value)}
    </div>
  );
}
