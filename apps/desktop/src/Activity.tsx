import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button, Card } from "@vocalis/ui";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  canRetry,
  type DayGroup,
  dayLabel,
  failureMessage,
  type HistoryRecord,
  MIN_QUERY,
  snippet,
  timeLabel,
  utcOffsetMinutes,
} from "./history";

const errorText = (e: unknown) => (typeof e === "string" ? e : "Something went wrong.");

/** The Activity panel: every dictation grouped by day, newest first (spec 4.6). */
export function Activity() {
  const [groups, setGroups] = useState<DayGroup[]>([]);
  const [results, setResults] = useState<HistoryRecord[] | null>(null);
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState<string | null>(null);
  const [busy, setBusy] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [more, setMore] = useState(false);
  const queryRef = useRef(query);
  queryRef.current = query;

  const load = useCallback(async () => {
    if (!isTauri()) return;
    try {
      const q = queryRef.current.trim();
      if (q.length >= MIN_QUERY) {
        setResults(await invoke<HistoryRecord[]>("history_search", { query: q }));
      } else {
        setResults(null);
        const g = await invoke<DayGroup[]>("history_list", {
          before: null,
          utcOffsetMinutes: utcOffsetMinutes(),
        });
        setGroups(g);
        setMore(g.reduce((n, d) => n + d.records.length, 0) >= 100);
      }
      setError(null);
    } catch (e) {
      setError(errorText(e));
    }
  }, []);

  const loadOlder = async () => {
    const last = groups.at(-1)?.records.at(-1);
    if (!last) return;
    const older = await invoke<DayGroup[]>("history_list", {
      before: last.createdAt,
      utcOffsetMinutes: utcOffsetMinutes(),
    });
    setMore(older.reduce((n, d) => n + d.records.length, 0) >= 100);
    setGroups((g) => {
      const merged = [...g];
      for (const day of older) {
        const tail = merged.at(-1);
        if (tail && tail.date === day.date)
          merged[merged.length - 1] = { ...tail, records: [...tail.records, ...day.records] };
        else merged.push(day);
      }
      return merged;
    });
  };

  useEffect(() => {
    void load();
    if (!isTauri()) return;
    const off = listen("history-changed", () => void load());
    return () => void off.then((f) => f());
  }, [load]);

  useEffect(() => {
    const t = setTimeout(() => void load(), 150);
    return () => clearTimeout(t);
  }, [query, load]);

  const act = async (id: string, command: string) => {
    setBusy((b) => new Set(b).add(id));
    try {
      await invoke(command, { id });
      setError(null);
    } catch (e) {
      setError(errorText(e));
    } finally {
      // Retry finishes later (a history-changed event); keep its spinner briefly.
      setTimeout(
        () =>
          setBusy((b) => {
            const n = new Set(b);
            n.delete(id);
            return n;
          }),
        command === "history_retry" ? 1500 : 0,
      );
    }
  };

  const row = (r: HistoryRecord) => (
    <HistoryRow
      key={r.id}
      record={r}
      open={open === r.id}
      busy={busy.has(r.id)}
      onToggle={() => setOpen((o) => (o === r.id ? null : r.id))}
      onAction={(cmd) => void act(r.id, cmd)}
    />
  );

  const empty = results ? results.length === 0 : groups.length === 0;
  return (
    <section aria-labelledby="activity-title" className="activity">
      <div className="activity-head">
        <h2 id="activity-title">Activity</h2>
        <span
          className="muted privacy"
          title="Transcripts and recordings are stored only on this device, encrypted."
        >
          🔒 On this device, encrypted
        </span>
        <input
          type="search"
          aria-label="Search your dictations"
          placeholder="Search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>
      {error ? (
        <p role="alert" className="error-text">
          {error}
        </p>
      ) : null}
      <Card>
        {empty ? (
          <p className="muted">
            {results
              ? "No dictations match. Failed recordings aren't searchable; find them by date."
              : "Nothing here yet. Your dictations will show up here."}
          </p>
        ) : results ? (
          <ul className="records">{results.map(row)}</ul>
        ) : (
          groups.map((g) => (
            <div key={g.date} className="day">
              <h3>{dayLabel(g.date)}</h3>
              <ul className="records">{g.records.map(row)}</ul>
            </div>
          ))
        )}
        {more && !results ? (
          <Button variant="ghost" onClick={() => void loadOlder()}>
            Show older
          </Button>
        ) : null}
      </Card>
    </section>
  );
}

function HistoryRow({
  record: r,
  open,
  busy,
  onToggle,
  onAction,
}: {
  record: HistoryRecord;
  open: boolean;
  busy: boolean;
  onToggle: () => void;
  onAction: (command: string) => void;
}) {
  const failed = r.status === "failed";
  return (
    <li className={`record${failed ? " failed" : ""}`}>
      <button className="record-main" aria-expanded={open} onClick={onToggle}>
        <span className="record-time muted">{timeLabel(r.createdAt)}</span>
        <span className="record-text">
          {failed ? failureMessage(r.errorCode) : snippet(r.text)}
        </span>
        {r.appName ? <span className="record-app muted">{r.appName}</span> : null}
        {r.kind === "command" ? <span className="chip">Command</span> : null}
      </button>
      <span className="record-actions">
        {failed && canRetry(r) ? (
          <Button variant="ghost" disabled={busy} onClick={() => onAction("history_retry")}>
            {busy ? "Retrying…" : "Retry"}
          </Button>
        ) : null}
        {!failed && r.text ? (
          <Button variant="ghost" onClick={() => onAction("history_copy")}>
            Copy
          </Button>
        ) : null}
      </span>
      {open ? <RecordDetail record={r} onDelete={() => onAction("history_delete")} /> : null}
    </li>
  );
}

function RecordDetail({ record: r, onDelete }: { record: HistoryRecord; onDelete: () => void }) {
  const [src, setSrc] = useState<string | null>(null);
  const [audioError, setAudioError] = useState<string | null>(null);

  useEffect(() => {
    if (!r.audioPath || !isTauri()) return;
    let url: string | null = null;
    invoke<ArrayBuffer>("history_audio", { id: r.id })
      .then((buf) => {
        url = URL.createObjectURL(new Blob([buf], { type: "audio/wav" }));
        setSrc(url);
      })
      .catch((e) => setAudioError(errorText(e)));
    return () => {
      if (url) URL.revokeObjectURL(url);
    };
  }, [r.id, r.audioPath]);

  return (
    <div className="record-detail">
      {r.text ? <p className="record-full">{r.text}</p> : null}
      {src ? (
        <div className="row">
          <audio controls src={src} aria-label="Recording" />
          <a className="muted" href={src} download={`vocalis-${r.id}.wav`}>
            Download audio
          </a>
        </div>
      ) : r.audioPath ? (
        <p className="muted">{audioError ?? "Loading audio…"}</p>
      ) : (
        <p className="muted">Audio is kept for your 100 most recent recordings.</p>
      )}
      <div className="row">
        <span className="muted">
          {r.words} words · {Math.round(r.durationMs / 1000)} s{r.wpm ? ` · ${r.wpm} wpm` : ""}
        </span>
        <Button
          variant="ghost"
          onClick={() => {
            if (window.confirm("Delete this record and its audio? This can't be undone."))
              onDelete();
          }}
        >
          Delete
        </Button>
      </div>
    </div>
  );
}
