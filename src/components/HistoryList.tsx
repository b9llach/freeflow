import { Transcription } from "../lib/api";

interface Props {
  items: Transcription[];
  onCopy: (text: string) => void;
  onDelete: (id: string) => void;
}

function formatTime(iso: string) {
  try {
    const d = new Date(iso);
    const now = new Date();
    const sameDay =
      d.getFullYear() === now.getFullYear() &&
      d.getMonth() === now.getMonth() &&
      d.getDate() === now.getDate();
    if (sameDay) {
      return d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
    }
    return d.toLocaleDateString([], {
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

export function HistoryList({ items, onCopy, onDelete }: Props) {
  return (
    <>
      {items.map((t) => {
        const main = t.cleaned_text ?? t.raw_text;
        const hasBoth = !!t.cleaned_text && t.cleaned_text !== t.raw_text;
        return (
          <article className="card" key={t.id}>
            <div className="card-meta">
              <span>{formatTime(t.created_at)}</span>
              <span className="right">
                {t.llm_model && <span className="model">{t.llm_model}</span>}
                {t.duration_ms != null && <span>{Math.round(t.duration_ms)}ms</span>}
              </span>
            </div>
            <div className="card-text">{main}</div>
            {hasBoth && (
              <div className="card-raw">
                <span className="card-raw-label">Raw transcript</span>
                {t.raw_text}
              </div>
            )}
            <div className="card-actions">
              <button className="btn small primary" onClick={() => onCopy(main)}>
                Copy
              </button>
              {hasBoth && (
                <button className="btn small ghost" onClick={() => onCopy(t.raw_text)}>
                  Copy raw
                </button>
              )}
              <button className="btn small danger" onClick={() => onDelete(t.id)}>
                Delete
              </button>
            </div>
          </article>
        );
      })}
    </>
  );
}
