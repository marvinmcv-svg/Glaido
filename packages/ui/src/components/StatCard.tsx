import { Card } from "./Card";

type StatCardProps = { label: string; value: string; hint?: string };

export function StatCard({ label, value, hint }: StatCardProps) {
  return (
    <Card className="vc-stat">
      <div className="vc-stat__label">{label}</div>
      <div className="vc-stat__value">{value}</div>
      {hint ? <div className="vc-stat__hint">{hint}</div> : null}
    </Card>
  );
}
