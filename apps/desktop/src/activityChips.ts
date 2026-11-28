/** Activity row chips and report-a-record options (spec 4.6). Pure. */

import type { HistoryRecord } from "./history";

export type Chip = { label: string; tone: "default" | "live" };

/** Command / Running / tool-count chips for a record row. */
export function recordChips(r: HistoryRecord): Chip[] {
  const chips: Chip[] = [];
  if (r.kind === "command") chips.push({ label: "Command", tone: "default" });
  if (r.status === "running") chips.push({ label: "Running", tone: "live" });
  else if (r.kind === "command" && typeof r.toolCount === "number" && r.toolCount > 0)
    chips.push({
      label: r.toolCount === 1 ? "1 tool" : `${r.toolCount} tools`,
      tone: "default",
    });
  return chips;
}

export type ReportCategory =
  "wrong_word" | "missing_text" | "wrong_formatting" | "wrong_app" | "other";

export const REPORT_CATEGORIES: { value: ReportCategory; label: string }[] = [
  { value: "wrong_word", label: "A word came out wrong" },
  { value: "missing_text", label: "Some of what I said is missing" },
  { value: "wrong_formatting", label: "The formatting or punctuation is off" },
  { value: "wrong_app", label: "It went to the wrong place" },
  { value: "other", label: "Something else" },
];

/** A report can be sent for records that have finished. */
export function canReport(r: HistoryRecord): boolean {
  return r.status !== "running";
}

export type ReportResult = {
  payload: { text: string; category: ReportCategory; correctSpelling: string | null };
  sent: boolean;
  addedToDictionary: boolean;
  dictionaryNote: string | null;
};

/** What the dialog says after "Send report". */
export function reportOutcome(r: ReportResult): string {
  const sent = r.sent
    ? "Thanks, your report was sent."
    : "Thanks. Sending reports isn't available yet, so nothing left this device.";
  if (r.addedToDictionary) return `${sent} The word is in your dictionary now.`;
  if (r.dictionaryNote) return `${sent} Not added to your dictionary: ${r.dictionaryNote}`;
  return sent;
}
