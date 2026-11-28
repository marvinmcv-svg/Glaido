import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button, Card } from "@vocalis/ui";
import { useCallback, useEffect, useId, useRef, useState } from "react";
import {
  canReport,
  recordChips,
  REPORT_CATEGORIES,
  type ReportCategory,
  type ReportResult,
  reportOutcome,
} from "./activityChips";
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
import "./home-insights.css";

const errorText = (e: unknown) => (typeof e === "string" ? e : "Something went wrong.");

/** A record to open, e.g. picked in the Command Palette; `nonce` re-opens the same one. */
export type ActivityFocus = { id: string; query: string; nonce: number };

/** The Activity panel: every dictation grouped by day, newest first (spec 4.6). */
export function Activity({ focus = null }: { focus?: ActivityFocus | null }) {
  const [groups, setGroups] = useState<DayGroup[]>([]);
  const [results, setResults] = useState<HistoryRecord[] | null>(null);
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState<string | null>(null);
  const [busy, setBusy] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [more, setMore] = useState(false);
  const [reporting, setReporting] = useState<HistoryRecord | null>(null);
  const queryRef = useRef(query);
  useEffect(() => {
    queryRef.current = query;
  }, [query]);

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

  // Open a record picked elsewhere: search for it, expand it, scroll to it.
  const [seenFocus, setSeenFocus] = useState<ActivityFocus | null>(null);
  if (focus !== seenFocus) {
    setSeenFocus(focus);
    if (focus) {
      setQuery(focus.query);
      setOpen(focus.id);
    }
  }
  useEffect(() => {
    if (open) document.getElementById(`record-${open}`)?.scrollIntoView?.({ block: "center" });
  }, [open, results, groups]);

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
      onReport={() => setReporting(r)}
    />
  );

  const empty = results ? results.length === 0 : groups.length === 0;
  return (
    <section aria-labelledby="activity-title" className="activity">
      <div className="activity-head">
        <h2 id="activity-title">Activity</h2>
        <PrivacyNote />
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
      {reporting ? <ReportDialog record={reporting} onClose={() => setReporting(null)} /> : null}
    </section>
  );
}

function ShieldIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      <path
        d="M8 1.5 2.5 3.5v4c0 3.3 2.3 6 5.5 7 3.2-1 5.5-3.7 5.5-7v-4L8 1.5Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinejoin="round"
      />
      <path
        d="m5.6 8 1.7 1.7 3.2-3.4"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/** Shield with a tooltip that also opens on keyboard focus. */
function PrivacyNote() {
  const id = useId();
  return (
    <span className="privacy">
      <button type="button" className="privacy-button muted" aria-describedby={id}>
        <ShieldIcon /> Private
      </button>
      <span role="tooltip" id={id} className="tooltip">
        Your transcripts and recordings are stored only on this device, encrypted. Nothing here is
        uploaded unless you report a record.
      </span>
    </span>
  );
}

function ReportDialog({ record: r, onClose }: { record: HistoryRecord; onClose: () => void }) {
  const [category, setCategory] = useState<ReportCategory>("wrong_word");
  const [wrongWord, setWrongWord] = useState("");
  const [spelling, setSpelling] = useState("");
  const [addToDictionary, setAddToDictionary] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<string | null>(null);
  const first = useRef<HTMLInputElement>(null);
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  }, [onClose]);
  useEffect(() => {
    first.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close.current();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  const wordCase = category === "wrong_word";

  const send = async () => {
    if (!isTauri()) return;
    setBusy(true);
    try {
      const res = await invoke<ReportResult>("history_report", {
        id: r.id,
        category,
        correctSpelling: wordCase && spelling.trim() ? spelling : null,
        wrongWord: wordCase && wrongWord.trim() ? wrongWord : null,
        addToDictionary: wordCase && addToDictionary && spelling.trim().length > 0,
      });
      setOutcome(reportOutcome(res));
      setError(null);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="wizard">
      <div
        className="wizard-panel dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="report-title"
      >
        <h1 id="report-title">What went wrong?</h1>
        {outcome ? (
          <>
            <p role="status">{outcome}</p>
            <div className="wizard-nav">
              <span className="spacer" />
              <Button onClick={onClose}>Done</Button>
            </div>
          </>
        ) : (
          <>
            <fieldset className="field">
              <legend className="muted">Pick the closest match</legend>
              {REPORT_CATEGORIES.map((c, i) => (
                <label key={c.value} className="radio">
                  <input
                    ref={i === 0 ? first : undefined}
                    type="radio"
                    name="report-category"
                    checked={category === c.value}
                    onChange={() => setCategory(c.value)}
                  />
                  {c.label}
                </label>
              ))}
            </fieldset>
            {wordCase ? (
              <div className="field-pair">
                <label className="field">
                  What it wrote
                  <input
                    className="input"
                    value={wrongWord}
                    maxLength={50}
                    placeholder="e.g. vocal is"
                    onChange={(e) => setWrongWord(e.target.value)}
                  />
                </label>
                <label className="field">
                  The right spelling
                  <input
                    className="input"
                    value={spelling}
                    maxLength={50}
                    placeholder="e.g. Vocalis"
                    onChange={(e) => setSpelling(e.target.value)}
                  />
                </label>
                <label className="checkbox">
                  <input
                    type="checkbox"
                    checked={addToDictionary}
                    onChange={(e) => setAddToDictionary(e.target.checked)}
                  />
                  Add to my dictionary
                </label>
              </div>
            ) : null}
            <p className="muted">
              Only this record&apos;s text is included, never its audio or any other record.
            </p>
            <blockquote className="report-preview">
              {snippet(r.text || failureMessage(r.errorCode), 400)}
            </blockquote>
            {error ? (
              <p role="alert" className="error-text">
                {error}
              </p>
            ) : null}
            <div className="wizard-nav">
              <span className="spacer" />
              <Button variant="ghost" onClick={onClose}>
                Cancel
              </Button>
              <Button disabled={busy} onClick={() => void send()}>
                {busy ? "Sending…" : "Send report"}
              </Button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function HistoryRow({
  record: r,
  open,
  busy,
  onToggle,
  onAction,
  onReport,
}: {
  record: HistoryRecord;
  open: boolean;
  busy: boolean;
  onToggle: () => void;
  onAction: (command: string) => void;
  onReport: () => void;
}) {
  const failed = r.status === "failed";
  return (
    <li className={`record${failed ? " failed" : ""}`} id={`record-${r.id}`}>
      <button className="record-main" aria-expanded={open} onClick={onToggle}>
        <span className="record-time muted">{timeLabel(r.createdAt)}</span>
        <span className="record-text">
          {failed ? failureMessage(r.errorCode) : snippet(r.text)}
        </span>
        {r.appName ? <span className="record-app muted">{r.appName}</span> : null}
        {recordChips(r).map((c) => (
          <span key={c.label} className={`chip${c.tone === "live" ? " chip-live" : ""}`}>
            {c.label}
          </span>
        ))}
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
        {canReport(r) ? (
          <Button
            variant="ghost"
            aria-label="Report a problem with this record"
            title="Report a problem"
            onClick={onReport}
          >
            <ThumbsDown />
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
          {/* eslint-disable-next-line jsx-a11y/media-has-caption -- the transcript shown above is this recording's text; there is no timed caption track to offer */}
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

function ThumbsDown() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      <path
        d="M5.5 9.5V2.5h6.2c.6 0 1.1.4 1.2 1l.8 4.6c.1.8-.5 1.4-1.2 1.4H9l.5 2.6c.1.7-.4 1.4-1.2 1.4L5.5 9.5Zm0 0H2.5v-7h3"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
        strokeLinejoin="round"
      />
    </svg>
  );
}
