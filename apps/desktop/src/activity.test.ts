import { describe, expect, it } from "vitest";
import { canReport, recordChips, reportOutcome } from "./activity";
import type { HistoryRecord } from "./history";

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
  durationMs: 500,
  wpm: 120,
  audioPath: null,
  errorCode: null,
  createdAt: 0,
};
const labels = (r: HistoryRecord) => recordChips(r).map((c) => c.label);

describe("recordChips", () => {
  it("dictations have none", () => {
    expect(labels(base)).toEqual([]);
  });
  it("commands show Command, then Running or the tool count", () => {
    const cmd = { ...base, kind: "command" as const };
    expect(labels(cmd)).toEqual(["Command"]);
    expect(labels({ ...cmd, status: "running" })).toEqual(["Command", "Running"]);
    expect(labels({ ...cmd, toolCount: 1 })).toEqual(["Command", "1 tool"]);
    expect(labels({ ...cmd, toolCount: 3 })).toEqual(["Command", "3 tools"]);
    expect(labels({ ...cmd, status: "running", toolCount: 3 })).toEqual(["Command", "Running"]);
    expect(recordChips({ ...cmd, status: "running" })[1]?.tone).toBe("live");
  });
});

describe("report", () => {
  it("running records can't be reported", () => {
    expect(canReport(base)).toBe(true);
    expect(canReport({ ...base, status: "failed" })).toBe(true);
    expect(canReport({ ...base, status: "running" })).toBe(false);
  });
  it("says honestly that nothing was sent yet", () => {
    const payload = { text: "x", category: "wrong_word" as const, correctSpelling: "Vocalis" };
    expect(
      reportOutcome({ payload, sent: false, addedToDictionary: true, dictionaryNote: null }),
    ).toBe(
      "Thanks. Sending reports isn't available yet, so nothing left this device. The word is in your dictionary now.",
    );
    expect(
      reportOutcome({
        payload,
        sent: true,
        addedToDictionary: false,
        dictionaryNote: "this entry already exists",
      }),
    ).toBe("Thanks, your report was sent. Not added to your dictionary: this entry already exists");
  });
});
