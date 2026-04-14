import { KeyboardEvent, useState } from "react";

interface Props {
  items: string[];
  onChange: (next: string[]) => void;
}

export function VocabularyEditor({ items, onChange }: Props) {
  const [draft, setDraft] = useState("");

  const add = () => {
    const v = draft.trim();
    if (!v) return;
    if (items.includes(v)) {
      setDraft("");
      return;
    }
    onChange([...items, v]);
    setDraft("");
  };

  const remove = (idx: number) => {
    onChange(items.filter((_, i) => i !== idx));
  };

  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      add();
    }
  };

  return (
    <>
      <div className="field">
        <label>Add a term or fact</label>
        <div className="row">
          <input
            type="text"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={onKey}
            placeholder='e.g. "Belsad" is a company name'
          />
          <button
            className="btn small fit primary"
            onClick={add}
            disabled={!draft.trim()}
          >
            Add
          </button>
        </div>
        <div className="field-hint">
          These get sent to the cleanup model so proper nouns and product names
          come out right.
        </div>
      </div>
      {items.length > 0 && (
        <ul className="vocab-list">
          {items.map((v, i) => (
            <li className="vocab-row" key={`${i}-${v}`}>
              <span className="vocab-text">{v}</span>
              <button
                className="vocab-remove"
                onClick={() => remove(i)}
                aria-label="Remove"
              >
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}
