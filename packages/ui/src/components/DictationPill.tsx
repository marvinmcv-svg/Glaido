export type PillState = "recording" | "processing" | "slow" | "limit" | "error";

const STATUS: Record<PillState, string> = {
  recording: "Listening",
  processing: "Writing it up",
  slow: "Slow connection, catching up",
  limit: "Under 30 seconds left",
  error: "Something went wrong. Your audio is saved.",
};

type PillProps = {
  state: PillState;
  /** Recent input levels, 0–1, oldest first. */
  levels?: number[];
};

/** The dictation bar. It is a status display only and never takes focus. */
export function DictationPill({ state, levels = [] }: PillProps) {
  const bars = levels.slice(-24);
  return (
    <div className="vc-pill" data-state={state} role="status" aria-live="polite">
      <span className="vc-pill__dot" aria-hidden="true" />
      <span className="vc-pill__wave" aria-hidden="true">
        {bars.map((l, i) => (
          <span key={i} className="vc-pill__bar" style={{ height: `${Math.round(4 + Math.min(1, Math.max(0, l)) * 20)}px` }} />
        ))}
      </span>
      <span className="vc-pill__label">{STATUS[state]}</span>
    </div>
  );
}
