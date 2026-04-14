import { listen } from "@tauri-apps/api/event";

type Status = "idle" | "recording" | "thinking" | "pasted";

const LABELS: Record<Status, string> = {
  idle: "",
  recording: "Recording",
  thinking: "Thinking",
  pasted: "Pasted",
};

const dot = document.querySelector<HTMLDivElement>(".dot")!;
const label = document.querySelector<HTMLDivElement>(".label")!;

function apply(status: Status) {
  dot.className = `dot ${status}`;
  label.textContent = LABELS[status];
}

apply("recording");

listen<Status>("freeflow://status", (e) => {
  if (e.payload === "idle") return;
  apply(e.payload);
});
