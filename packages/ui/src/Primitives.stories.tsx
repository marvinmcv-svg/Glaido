import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Button, Card, DictationPill, Keycap, StatCard, Switch } from "./index";

const meta: Meta = { title: "Primitives/Overview" };
export default meta;

export const Buttons: StoryObj = {
  render: () => (
    <div style={{ display: "flex", gap: 8 }}>
      <Button>Start free</Button>
      <Button variant="attention">Upgrade</Button>
      <Button variant="ghost">Cancel</Button>
      <Button disabled>Saving</Button>
    </div>
  ),
};

export const Keycaps: StoryObj = {
  render: () => (
    <div style={{ display: "grid", gap: 8 }}>
      <Keycap keys={["fn"]} platform="macos" />
      <Keycap keys={["right-option", "space"]} platform="macos" />
      <Keycap keys={["ctrl", "cmd"]} platform="windows" />
      <Keycap keys={["left-ctrl", "space"]} platform="linux" />
    </div>
  ),
};

export const Toggle: StoryObj = {
  render: function Render() {
    const [on, setOn] = useState(true);
    return <Switch checked={on} onChange={setOn} label="Interaction sounds" />;
  },
};

export const Stats: StoryObj = {
  render: () => (
    <div style={{ display: "flex", gap: 12 }}>
      <StatCard label="Time saved" value="12h 30m" hint="About a day and a half" />
      <StatCard label="Speed" value="148 wpm" hint="3.7× your typing" />
      <Card>Plain card</Card>
    </div>
  ),
};

const wave = Array.from({ length: 24 }, (_, i) => Math.abs(Math.sin(i / 2.5)));
export const Pill: StoryObj = {
  render: () => (
    <div style={{ display: "grid", gap: 12, justifyItems: "start" }}>
      {(["recording", "processing", "slow", "limit", "error"] as const).map((s) => (
        <DictationPill key={s} state={s} levels={wave} />
      ))}
    </div>
  ),
};
