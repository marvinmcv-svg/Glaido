import { describe, expect, it } from "vitest";
import {
  bucketDays,
  compactNumber,
  type DayStats,
  formatDuration,
  niceTicks,
  speedComparison,
  streakLabel,
  streakValue,
  timeSavedComparison,
} from "./stats";

const MIN = 60_000;
const HOUR = 60 * MIN;

describe("formatDuration", () => {
  it("formats minutes and hours", () => {
    expect(formatDuration(0)).toBe("0m");
    expect(formatDuration(29_000)).toBe("0m");
    expect(formatDuration(45 * MIN)).toBe("45m");
    expect(formatDuration(3 * HOUR + 12 * MIN)).toBe("3h 12m");
    expect(formatDuration(123 * HOUR + 46 * MIN)).toBe("123h 46m");
    expect(formatDuration(-5)).toBe("0m");
  });
});

describe("timeSavedComparison", () => {
  it.each([
    [0, "Your first dictation starts the clock"],
    [5 * MIN, "about a coffee break"],
    [40 * MIN, "about a lunch break"],
    [2 * HOUR, "about a feature film"],
    [4 * HOUR, "about an afternoon"],
    [8 * HOUR, "about a full work day"],
    [24 * HOUR, "about 3 work days"],
    [40 * HOUR, "about a full work week"],
    [123 * HOUR, "about 3 work weeks"],
    [400 * HOUR, "about 2 months of work days"],
  ])("%d ms → %s", (ms, text) => {
    expect(timeSavedComparison(ms)).toBe(text);
  });
});

describe("speedComparison", () => {
  it("compares with a 40 wpm typist", () => {
    expect(speedComparison(0)).toBe("Measured after your first dictation");
    expect(speedComparison(42)).toBe("about as fast as typing");
    expect(speedComparison(150)).toBe("×3.8 faster than typing");
    expect(speedComparison(160)).toBe("×4 faster than typing");
    expect(speedComparison(420)).toBe("×11 faster than typing");
  });
});

describe("streak", () => {
  it("labels", () => {
    expect(streakValue(1)).toBe("1 day");
    expect(streakValue(0)).toBe("0 days");
    expect(streakLabel(0, false)).toBe("Dictate today to start one");
    expect(streakLabel(5, false)).toBe("Dictate today to keep it going");
    expect(streakLabel(1, true)).toBe("Day one. See you tomorrow");
    expect(streakLabel(3, true)).toBe("Building a habit");
    expect(streakLabel(7, true)).toBe("A whole week and counting");
    expect(streakLabel(200, true)).toBe("A true regular");
  });
});

describe("compactNumber", () => {
  it("is full under 10,000 and compact above", () => {
    expect(compactNumber(1284)).toBe("1,284");
    expect(compactNumber(12_900)).toBe("12.9K");
    expect(compactNumber(4_200_000)).toBe("4.2M");
  });
});

describe("niceTicks", () => {
  it("rounds to clean steps", () => {
    expect(niceTicks(0)).toEqual([0, 1]);
    expect(niceTicks(7)).toEqual([0, 2, 4, 6, 8]);
    expect(niceTicks(1830)).toEqual([0, 500, 1000, 1500, 2000]);
    expect(niceTicks(100)).toEqual([0, 25, 50, 75, 100]);
  });
});

describe("bucketDays", () => {
  const days = (n: number): DayStats[] =>
    Array.from({ length: n }, (_, i) => ({
      date: `d${i}`,
      words: i,
      dictations: i % 2,
      commands: 0,
    }));

  it("keeps daily columns for short ranges", () => {
    const b = bucketDays(days(30));
    expect(b).toHaveLength(30);
    expect(b[3]).toMatchObject({ date: "d3", words: 3, days: 1, activeDays: 1 });
  });

  it("groups a year into weeks that end today", () => {
    const d = days(365);
    const b = bucketDays(d);
    expect(b).toHaveLength(53); // 1 leftover day + 52 full weeks
    expect(b[0]).toMatchObject({ days: 1, words: 0 });
    expect(b.at(-1)).toMatchObject({ days: 7, label: "d358 – d364" });
    expect(b.reduce((n, x) => n + x.words, 0)).toBe(d.reduce((n, x) => n + x.words, 0));
  });
});
