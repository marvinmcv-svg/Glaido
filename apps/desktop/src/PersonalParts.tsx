import { Button } from "@vocalis/ui";
import { type RefObject, useState } from "react";
import { insertAt, TAGS } from "./personal";

type Field = HTMLInputElement | HTMLTextAreaElement;

/** Buttons that insert a dynamic tag at the caret of `target`. */
export function TagPicker({
  target,
  value,
  onChange,
}: {
  target: RefObject<Field | null>;
  value: string;
  onChange: (value: string) => void;
}) {
  const insert = (tag: string) => {
    const el = target.current;
    const r = insertAt(value, tag, el?.selectionStart ?? null, el?.selectionEnd ?? null);
    onChange(r.value);
    requestAnimationFrame(() => {
      el?.focus();
      el?.setSelectionRange(r.caret, r.caret);
    });
  };
  return (
    <div className="tag-picker" role="group" aria-label="Insert a tag">
      <span className="muted">Insert</span>
      {TAGS.map((t) => (
        <button
          key={t.tag}
          type="button"
          className="chip chip-button"
          title={t.hint}
          onClick={() => insert(t.tag)}
        >
          {t.label}
        </button>
      ))}
    </div>
  );
}

/** "12/50", flagged once over the limit. */
export function Counter({ count, max }: { count: number; max: number }) {
  return (
    <span className={`counter${count > max ? " over" : ""}`} aria-live="polite">
      {count}/{max}
    </span>
  );
}

/**
 * The hover delete button. A click asks first; Shift-click deletes at once
 * (when `allowShift`).
 */
export function DeleteControl({
  label,
  allowShift,
  onDelete,
}: {
  label: string;
  allowShift: boolean;
  onDelete: () => void;
}) {
  const [asking, setAsking] = useState(false);
  if (asking)
    return (
      <span className="item-confirm" role="group" aria-label={`Confirm: ${label}`}>
        <span className="muted">Delete?</span>
        <Button size="sm" variant="attention" onClick={onDelete}>
          Delete
        </Button>
        <Button size="sm" variant="ghost" onClick={() => setAsking(false)}>
          Keep
        </Button>
      </span>
    );
  return (
    <button
      type="button"
      className="item-delete"
      aria-label={label}
      title={allowShift ? "Delete (Shift-click skips the question)" : "Delete"}
      onClick={(e) => (allowShift && e.shiftKey ? onDelete() : setAsking(true))}
    >
      ✕
    </button>
  );
}
