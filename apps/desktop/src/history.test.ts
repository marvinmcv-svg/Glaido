import { describe, expect, it } from "vitest";
import {
  canRetry,
  dayLabel,
  failureMessage,
  type HistoryRecord,
  localDate,
  snippet,
  toastFor,
  utcOffsetMinutes,
} from "./history";

const base: HistoryRecord = {
  id: "a",
  kind: "dictation",
  status: "ok",
  text: "hello",
  appName: null,
  appId: null,
  url: null,
  fieldKind: null,
  words: 1,
  durationMs: 1000,
  wpm: 60,
  audioPath: "/x.wav.enc",
  errorCode: null,
  createdAt: 0,
};

describe("history helpers", () => {
  it("labels today, yesterday and older days", () => {
    const now = new Date(2026, 10, 27, 9, 0);
    expect(dayLabel("2026-11-27", now, "en-US")).toBe("Today");
    expect(dayLabel("2026-11-26", now, "en-US")).toBe("Yesterday");
    expect(dayLabel("2026-11-20", now, "en-US")).toBe("Friday, November 20");
    expect(dayLabel("2025-01-02", now, "en-US")).toContain("2025");
  });

  it("yesterday crosses month boundaries", () => {
    expect(dayLabel("2026-11-30", new Date(2026, 11, 1), "en-US")).toBe("Yesterday");
  });

  it("formats local dates and offsets", () => {
    expect(localDate(new Date(2026, 0, 5))).toBe("2026-01-05");
    expect(typeof utcOffsetMinutes()).toBe("number");
  });

  it("allows retry only for failed records with readable audio", () => {
    expect(canRetry(base)).toBe(false);
    expect(canRetry({ ...base, status: "failed", errorCode: "NETWORK" })).toBe(true);
    expect(canRetry({ ...base, status: "failed", audioPath: null })).toBe(false);
    expect(canRetry({ ...base, status: "failed", errorCode: "AUDIO_UNREADABLE" })).toBe(false);
  });

  it("has a message for every known failure code and a fallback", () => {
    for (const code of [
      "CANCELLED",
      "INTERRUPTED",
      "NETWORK",
      "QUOTA_EXCEEDED",
      "MIC_DISCONNECTED",
      null,
      "???",
    ]) {
      expect(failureMessage(code)).toMatch(/\S/);
    }
    expect(failureMessage("INTERRUPTED")).not.toBe(failureMessage(null));
  });

  it("shortens long text on a word-ish boundary", () => {
    expect(snippet("a  b\n c")).toBe("a b c");
    const s = snippet("x".repeat(300), 20);
    expect(s).toHaveLength(20);
    expect(s.endsWith("…")).toBe(true);
  });

  it("maps notices to toasts with the right action", () => {
    expect(toastFor({ kind: "transcription_failed", recordId: "r", code: "NETWORK" }).action).toBe(
      "retry",
    );
    expect(
      toastFor({ kind: "transcription_failed", recordId: "r", code: "QUOTA_EXCEEDED" }).action,
    ).toBeUndefined();
    expect(toastFor({ kind: "nowhere_to_paste", recordId: "r" }).action).toBe("copy");
    expect(toastFor({ kind: "mic_silent" }).recordId).toBeUndefined();
    expect(toastFor({ kind: "mic_disconnected", recordId: "r" }).action).toBe("retry");
    expect(toastFor({ kind: "mic_disconnected", recordId: null }).action).toBeUndefined();
    expect(toastFor({ kind: "mic_unavailable", reason: "x" }).title).toMatch(/microphone/);
  });
});
