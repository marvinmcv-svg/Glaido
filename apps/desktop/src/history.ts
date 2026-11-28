/** History types and pure helpers (spec 4.6). Mirrors `vocalis_store::Record`. */

export type RecordStatus = "ok" | "failed" | "running";
export type RecordType = "dictation" | "command";

export type HistoryRecord = {
  id: string;
  kind: RecordType;
  status: RecordStatus;
  text: string;
  appName: string | null;
  appId: string | null;
  url: string | null;
  fieldKind: string | null;
  words: number;
  durationMs: number;
  wpm: number;
  audioPath: string | null;
  errorCode: string | null;
  createdAt: number;
};

export type DayGroup = { date: string; records: HistoryRecord[] };

/** Search needs at least this many characters (spec 4.6). */
export const MIN_QUERY = 2;

/** Minutes east of UTC, as the store expects (JS reports the opposite sign). */
export function utcOffsetMinutes(at: Date = new Date()): number {
  return -at.getTimezoneOffset();
}

/** `YYYY-MM-DD` in local time. */
export function localDate(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/** "Today", "Yesterday", or a readable date for a `YYYY-MM-DD` group key. */
export function dayLabel(date: string, now: Date = new Date(), locale?: string): string {
  if (date === localDate(now)) return "Today";
  const y = new Date(now);
  y.setDate(y.getDate() - 1);
  if (date === localDate(y)) return "Yesterday";
  const [yy = 0, mm = 1, dd = 1] = date.split("-").map(Number);
  const d = new Date(yy, mm - 1, dd);
  return d.toLocaleDateString(locale, {
    weekday: "long",
    month: "long",
    day: "numeric",
    ...(yy !== now.getFullYear() ? { year: "numeric" } : {}),
  });
}

export function timeLabel(ms: number, locale?: string): string {
  return new Date(ms).toLocaleTimeString(locale, { hour: "numeric", minute: "2-digit" });
}

/** Original copy for a failed record, keyed by its error code (Appendix C). */
export function failureMessage(code: string | null): string {
  switch (code) {
    case "CANCELLED":
      return "You cancelled this recording. The audio is saved if you want the text after all.";
    case "INTERRUPTED":
      return "Vocalis closed before this recording finished. The audio is saved.";
    case "NETWORK":
      return "Couldn't reach the server. The recording is saved on this device.";
    case "QUOTA_EXCEEDED":
      return "This week's free words are used up. The recording is saved for later.";
    case "RECORDING_TOO_LONG":
      return "This recording is longer than the server accepts. The audio is saved.";
    case "SCREEN_LOCKED":
      return "Your screen locked while you were recording. What you said so far is saved.";
    case "MIC_DISCONNECTED":
      return "The microphone disconnected while you were speaking. What you said so far is saved.";
    case "AUDIO_UNREADABLE":
      return "The saved audio for this recording could not be read.";
    default:
      return "Couldn't turn this recording into text. The audio is saved.";
  }
}

/** Retry makes sense only for failed records that still have audio. */
export function canRetry(r: HistoryRecord): boolean {
  return r.status === "failed" && r.audioPath !== null && r.errorCode !== "AUDIO_UNREADABLE";
}

export function snippet(text: string, max = 140): string {
  const t = text.replace(/\s+/g, " ").trim();
  return t.length > max ? `${t.slice(0, max - 1).trimEnd()}…` : t;
}

/** Engine notices (`notice` events) as user-facing toasts. */
export type Notice =
  | { kind: "transcription_failed"; recordId: string; code: string }
  | { kind: "nowhere_to_paste"; recordId: string }
  | { kind: "paste_failed"; recordId: string }
  | { kind: "recording_limit_near" }
  | { kind: "mic_silent" }
  | { kind: "retry_succeeded"; recordId: string }
  | { kind: "mic_disconnected"; recordId: string | null }
  | { kind: "mic_unavailable"; reason: string };

export type Toast = {
  title: string;
  body: string;
  recordId?: string;
  action?: "retry" | "copy";
};

export function toastFor(n: Notice): Toast {
  switch (n.kind) {
    case "transcription_failed":
      return {
        title: "Dictation didn't go through",
        body: failureMessage(n.code),
        recordId: n.recordId,
        action: n.code === "QUOTA_EXCEEDED" ? undefined : "retry",
      };
    case "nowhere_to_paste":
      return {
        title: "No text field to paste into",
        body: "Your words are in History, ready to copy.",
        recordId: n.recordId,
        action: "copy",
      };
    case "paste_failed":
      return {
        title: "Couldn't paste",
        body: "The app didn't accept the text. It's in History, ready to copy.",
        recordId: n.recordId,
        action: "copy",
      };
    case "recording_limit_near":
      return {
        title: "30 seconds left",
        body: "Recordings stop at 10 minutes. Everything you said is still transcribed.",
      };
    case "mic_silent":
      return {
        title: "We didn't hear anything",
        body: "Check that the right microphone is selected and not muted.",
      };
    case "mic_disconnected":
      return n.recordId
        ? {
            title: "Microphone disconnected",
            body: "What you said so far is saved. Reconnect the mic, then press Retry.",
            recordId: n.recordId,
            action: "retry",
          }
        : {
            title: "Microphone disconnected",
            body: "Nothing was recorded. Check the connection or pick another mic in Settings.",
          };
    case "mic_unavailable":
      return {
        title: "Couldn't open the microphone",
        body: "Check that a mic is connected and that Vocalis may use it, then try again.",
      };
    case "retry_succeeded":
      return { title: "Transcribed", body: "The text is now in History.", recordId: n.recordId };
  }
}
