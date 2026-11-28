import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { StatCard } from "@vocalis/ui";
import { useEffect, useState } from "react";
import { utcOffsetMinutes } from "./history";
import {
  formatDuration,
  type HomeStats as Stats,
  speedComparison,
  streakLabel,
  streakValue,
  timeSavedComparison,
} from "./stats";

/** Home stat cards: time saved, dictation speed and day streak (spec 4.6). */
export function HomeStats() {
  const [stats, setStats] = useState<Stats | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    const load = () =>
      void invoke<Stats>("stats_home", { utcOffsetMinutes: utcOffsetMinutes() })
        .then(setStats)
        .catch(() => setStats(null));
    load();
    const off = listen("history-changed", load);
    // The streak can change at midnight without a new record.
    const t = setInterval(load, 10 * 60_000);
    return () => {
      clearInterval(t);
      void off.then((f) => f());
    };
  }, []);

  const s = stats;
  return (
    <div className="stats">
      <StatCard
        label="Time saved"
        value={formatDuration(s?.timeSavedMs ?? 0)}
        hint={timeSavedComparison(s?.timeSavedMs ?? 0)}
      />
      <StatCard
        label="Speed"
        value={s?.averageWpm ? `${s.averageWpm} wpm` : "— wpm"}
        hint={speedComparison(s?.averageWpm ?? 0)}
      />
      <StatCard
        label="Streak"
        value={streakValue(s?.streakDays ?? 0)}
        hint={streakLabel(s?.streakDays ?? 0, s?.activeToday ?? false)}
      />
    </div>
  );
}
