import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Card, StatCard } from "@vocalis/ui";
import { useEffect, useState } from "react";
import { utcOffsetMinutes } from "./history";
import "./home-insights.css";
import {
  type Bucket,
  bucketDays,
  compactNumber,
  type DayStats,
  type Insights as InsightsData,
  type InsightsRange,
  niceTicks,
  RANGES,
  shortDate,
} from "./stats";

/** Insights page (spec 4.6): words dictated, active days and command usage over time. */
export function Insights() {
  const [range, setRange] = useState<InsightsRange>("month");
  const [data, setData] = useState<InsightsData | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    let live = true;
    const load = () =>
      void invoke<InsightsData>("stats_insights", { range, utcOffsetMinutes: utcOffsetMinutes() })
        .then((next) => {
          if (!live) return;
          setData(next);
          setError(null);
        })
        .catch((e: unknown) => {
          if (live) setError(typeof e === "string" ? e : "Couldn't load your insights.");
        });
    load();
    const off = listen("history-changed", load);
    return () => {
      live = false;
      void off.then((f) => f());
    };
  }, [range]);

  const span = RANGES.find((r) => r.value === range)!;
  const days = data?.days ?? [];
  const buckets = bucketDays(days);
  const weekly = buckets.length !== days.length;
  return (
    <div className="insights">
      <div className="range" role="group" aria-label="Time range">
        {RANGES.map((r) => (
          <button
            key={r.value}
            className="range-button"
            aria-pressed={r.value === range}
            onClick={() => setRange(r.value)}
          >
            {r.label}
          </button>
        ))}
      </div>
      {error ? (
        <p role="alert" className="error-text">
          {error}
        </p>
      ) : null}
      <div className="stats">
        <StatCard
          label="Words dictated"
          value={compactNumber(data?.totalWords ?? 0)}
          hint={`In the last ${span.label}`}
        />
        <StatCard
          label="Active days"
          value={`${data?.activeDays ?? 0}`}
          hint={`Of ${days.length || span.days} days`}
        />
        <StatCard
          label="Commands"
          value={compactNumber(data?.commands ?? 0)}
          hint={data?.commands ? `In the last ${span.label}` : "None in this period yet"}
        />
      </div>
      <ChartCard
        title={weekly ? "Words per week" : "Words per day"}
        buckets={buckets}
        value={(b) => b.words}
        unit="words"
        empty="Dictate something and it shows up here."
      />
      <Card>
        <h2 className="chart-title">Active days</h2>
        <ActiveDays days={days} />
      </Card>
      <ChartCard
        title={weekly ? "Commands per week" : "Commands per day"}
        buckets={buckets}
        value={(b) => b.commands}
        unit="commands"
        empty="No commands in this period."
      />
    </div>
  );
}

const W = 640;
const H = 180;
const PAD = { top: 8, right: 8, bottom: 22, left: 44 };

function ChartCard({
  title,
  buckets,
  value,
  unit,
  empty,
}: {
  title: string;
  buckets: Bucket[];
  value: (b: Bucket) => number;
  unit: string;
  empty: string;
}) {
  const [hover, setHover] = useState<number | null>(null);
  const values = buckets.map(value);
  const max = Math.max(0, ...values);
  const ticks = niceTicks(max);
  const top = ticks.at(-1) ?? 1;
  const plotW = W - PAD.left - PAD.right;
  const plotH = H - PAD.top - PAD.bottom;
  const slot = buckets.length ? plotW / buckets.length : plotW;
  const barW = Math.max(1, Math.min(24, slot - 2));
  const y = (v: number) => PAD.top + plotH - (v / top) * plotH;
  const labelAt = new Set([0, Math.floor((buckets.length - 1) / 2), buckets.length - 1]);
  const h = hover !== null ? buckets[hover] : undefined;

  return (
    <Card>
      <h2 className="chart-title">{title}</h2>
      {max === 0 ? (
        <p className="muted">{empty}</p>
      ) : (
        <div className="chart" onMouseLeave={() => setHover(null)}>
          <svg
            viewBox={`0 0 ${W} ${H}`}
            role="img"
            aria-label={`${title}, up to ${compactNumber(max)} ${unit}`}
          >
            {ticks.map((t) => (
              <g key={t}>
                <line className="chart-grid" x1={PAD.left} x2={W - PAD.right} y1={y(t)} y2={y(t)} />
                <text
                  className="chart-axis"
                  x={PAD.left - 6}
                  y={y(t)}
                  textAnchor="end"
                  dominantBaseline="middle"
                >
                  {compactNumber(t)}
                </text>
              </g>
            ))}
            {buckets.map((b, i) => {
              const v = values[i] ?? 0;
              const x = PAD.left + i * slot + (slot - barW) / 2;
              const barH = Math.max(0, y(0) - y(v));
              const r = Math.min(4, barW / 2, barH);
              return (
                <g key={b.date} onMouseEnter={() => setHover(i)}>
                  {/* Hit target: the whole slot, taller than the bar. */}
                  <rect
                    x={PAD.left + i * slot}
                    y={PAD.top}
                    width={slot}
                    height={plotH}
                    fill="transparent"
                  />
                  {v > 0 ? (
                    <path
                      className={`chart-bar${hover === i ? " is-hover" : ""}`}
                      d={roundedTop(x, y(v), barW, barH, r)}
                    />
                  ) : null}
                  {labelAt.has(i) ? (
                    <text
                      className="chart-axis"
                      x={PAD.left + i * slot + slot / 2}
                      y={H - 6}
                      textAnchor={i === 0 ? "start" : i === buckets.length - 1 ? "end" : "middle"}
                    >
                      {shortDate(b.date)}
                    </text>
                  ) : null}
                </g>
              );
            })}
          </svg>
          {h ? (
            <div
              className="chart-tooltip"
              role="status"
              style={{ left: `${((PAD.left + ((hover ?? 0) + 0.5) * slot) / W) * 100}%` }}
            >
              <div className="muted">
                {h.days > 1 ? `Week of ${shortDate(h.date)}` : shortDate(h.date)}
              </div>
              <strong>
                {value(h).toLocaleString()} {unit}
              </strong>
            </div>
          ) : null}
        </div>
      )}
      <details className="chart-table">
        <summary className="muted">Show as table</summary>
        <table>
          <thead>
            <tr>
              <th scope="col">{buckets.some((b) => b.days > 1) ? "Week starting" : "Day"}</th>
              <th scope="col">{unit[0]!.toUpperCase() + unit.slice(1)}</th>
            </tr>
          </thead>
          <tbody>
            {buckets.map((b) => (
              <tr key={b.date}>
                <td>{shortDate(b.date)}</td>
                <td>{value(b).toLocaleString()}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </Card>
  );
}

/** A column path with 4px rounded top corners and a square baseline. */
function roundedTop(x: number, y: number, w: number, h: number, r: number): string {
  if (h <= 0) return "";
  return `M${x},${y + h}V${y + r}Q${x},${y} ${x + r},${y}H${x + w - r}Q${x + w},${y} ${x + w},${y + r}V${y + h}Z`;
}

/** One cell per day, weeks as columns (oldest left); filled when anything succeeded that day. */
function ActiveDays({ days }: { days: DayStats[] }) {
  const [hover, setHover] = useState<DayStats | null>(null);
  if (days.length === 0) return <p className="muted">No days to show yet.</p>;
  const rows = days.length <= 7 ? days.length : 7;
  const cols = Math.ceil(days.length / rows);
  const size = days.length <= 7 ? 28 : days.length <= 90 ? 16 : 10;
  const gap = 2;
  const w = cols * (size + gap);
  const h = rows * (size + gap);
  const active = (d: DayStats) => d.dictations + d.commands > 0;
  return (
    <div className="chart">
      <div className="legend muted">
        <span className="legend-key is-active" /> Active
        <span className="legend-key" /> No activity
        <span className="legend-note">
          {hover
            ? `${shortDate(hover.date)}: ${active(hover) ? `${hover.words.toLocaleString()} words, ${hover.commands} commands` : "no activity"}`
            : ""}
        </span>
      </div>
      <svg
        viewBox={`0 0 ${w} ${h}`}
        className="active-days"
        style={{ maxWidth: `${w}px` }}
        role="img"
        aria-label={`Active on ${days.filter(active).length} of ${days.length} days`}
        onMouseLeave={() => setHover(null)}
      >
        {days.map((d, i) => (
          <rect
            key={d.date}
            x={Math.floor(i / rows) * (size + gap)}
            y={(i % rows) * (size + gap)}
            width={size}
            height={size}
            rx={2}
            className={active(d) ? "day-cell is-active" : "day-cell"}
            onMouseEnter={() => setHover(d)}
          />
        ))}
      </svg>
      <details className="chart-table">
        <summary className="muted">Show as table</summary>
        <table>
          <thead>
            <tr>
              <th scope="col">Day</th>
              <th scope="col">Words</th>
              <th scope="col">Dictations</th>
              <th scope="col">Commands</th>
            </tr>
          </thead>
          <tbody>
            {days.map((d) => (
              <tr key={d.date}>
                <td>{shortDate(d.date)}</td>
                <td>{d.words.toLocaleString()}</td>
                <td>{d.dictations}</td>
                <td>{d.commands}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </div>
  );
}
