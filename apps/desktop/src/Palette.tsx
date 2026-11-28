import { invoke, isTauri } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState } from "react";
import type { HistoryRecord } from "./history";
import "./home-insights.css";
import {
  buildRows,
  canSearchTranscripts,
  filterRows,
  groupRows,
  moveSelection,
  type PaletteAction,
  type PaletteRow,
  transcriptRows,
} from "./palette";
import { effectiveTheme } from "./theme";

type Props = {
  pages: readonly string[];
  mac: boolean;
  onAction: (action: PaletteAction) => void;
  onClose: () => void;
};

/** Command Palette (⌘K / Ctrl+K): jump to pages, quick-add, settings, theme and transcripts. */
export function Palette({ pages, mac, onAction, onClose }: Props) {
  const [query, setQuery] = useState("");
  const [transcripts, setTranscripts] = useState<PaletteRow[]>([]);
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  }, [onClose]);

  const staticRows = useMemo(
    () => buildRows({ pages, settingsShortcut: mac ? "⌘," : "Ctrl+,", theme: effectiveTheme() }),
    [pages, mac],
  );
  const searchable = canSearchTranscripts(query);
  const rows = useMemo(
    () => [...filterRows(staticRows, query), ...(searchable ? transcripts : [])],
    [staticRows, query, searchable, transcripts],
  );

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    input.current?.focus();
    // A click outside the panel closes it.
    const onDown = (e: MouseEvent) => {
      if (!panel.current?.contains(e.target as Node)) close.current();
    };
    document.addEventListener("mousedown", onDown);
    return () => {
      document.removeEventListener("mousedown", onDown);
      previous?.focus?.();
    };
  }, []);

  useEffect(() => {
    if (!searchable || !isTauri()) return;
    let stale = false;
    const t = setTimeout(() => {
      invoke<HistoryRecord[]>("history_search", { query: query.trim() })
        .then((r) => {
          if (!stale) setTranscripts(transcriptRows(r, query));
        })
        .catch(() => {
          if (!stale) setTranscripts([]);
        });
    }, 120);
    return () => {
      stale = true;
      clearTimeout(t);
    };
  }, [query, searchable]);

  useEffect(() => {
    listRef.current
      ?.querySelector(`[data-index="${active}"]`)
      ?.scrollIntoView?.({ block: "nearest" });
  }, [active]);

  const choose = (row: PaletteRow | undefined) => {
    if (!row) return;
    onClose();
    onAction(row.action);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      setActive((i) => moveSelection(i, e.key === "ArrowDown" ? 1 : -1, rows.length));
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(rows[active]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (e.key === "Tab") {
      // Focus stays in the palette while it is open.
      e.preventDefault();
    }
  };

  const optionId = (i: number) => `palette-option-${i}`;
  let index = 0;
  return (
    <div className="palette-backdrop">
      <div
        ref={panel}
        className="palette"
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
      >
        <input
          ref={input}
          className="palette-input"
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-list"
          aria-autocomplete="list"
          aria-activedescendant={rows.length ? optionId(active) : undefined}
          aria-label="Search pages, settings and transcripts"
          placeholder="Search pages, settings and transcripts"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
          onKeyDown={onKeyDown}
        />
        <ul
          id="palette-list"
          ref={listRef}
          role="listbox"
          className="palette-list"
          aria-label="Results"
        >
          {rows.length === 0 ? (
            <li className="palette-empty muted" role="presentation">
              {searchable
                ? "Nothing matches."
                : "Nothing matches. Type 2 letters to search transcripts."}
            </li>
          ) : (
            groupRows(rows).map((g) => (
              <li key={g.group} role="presentation">
                <div className="palette-group" id={`palette-group-${g.group}`} role="presentation">
                  {g.group}
                </div>
                <ul role="group" aria-labelledby={`palette-group-${g.group}`}>
                  {g.rows.map((row) => {
                    const i = index++;
                    return (
                      // Keyboard handling lives on the combobox (aria-activedescendant).
                      // eslint-disable-next-line jsx-a11y/click-events-have-key-events
                      <li
                        key={row.id}
                        id={optionId(i)}
                        data-index={i}
                        role="option"
                        aria-selected={i === active}
                        className="palette-option"
                        onMouseMove={() => setActive(i)}
                        onClick={() => choose(row)}
                      >
                        <span className="palette-label">{row.label}</span>
                        {row.hint ? <span className="palette-hint muted">{row.hint}</span> : null}
                      </li>
                    );
                  })}
                </ul>
              </li>
            ))
          )}
        </ul>
        <div className="palette-foot muted" aria-hidden="true">
          ↑↓ to move · Enter to open · Esc to close
        </div>
      </div>
    </div>
  );
}
