export type Platform = "macos" | "windows" | "linux";

const MAC: Record<string, string> = {
  cmd: "⌘", command: "⌘", meta: "⌘", option: "⌥", alt: "⌥", ctrl: "⌃", control: "⌃",
  shift: "⇧", fn: "fn", enter: "↩", return: "↩", esc: "esc", escape: "esc", space: "Space",
  backspace: "⌫", tab: "⇥", up: "↑", down: "↓", left: "←", right: "→",
};
const PC: Record<string, string> = {
  cmd: "Win", command: "Win", meta: "Win", option: "Alt", alt: "Alt", ctrl: "Ctrl", control: "Ctrl",
  shift: "Shift", fn: "Fn", enter: "Enter", return: "Enter", esc: "Esc", escape: "Esc", space: "Space",
  backspace: "Backspace", tab: "Tab", up: "↑", down: "↓", left: "←", right: "→",
};
const SPOKEN: Record<string, string> = {
  "⌘": "Command", "⌥": "Option", "⌃": "Control", "⇧": "Shift", "↩": "Return", "⌫": "Delete", "⇥": "Tab",
};

/** Display label for one key on a platform. Prefixes `left`/`right` are kept ("Right ⌥"). */
export function keyLabel(key: string, platform: Platform): string {
  const m = /^(left|right)[ _-]?(.+)$/i.exec(key.trim());
  const side = m ? (m[1]!.toLowerCase() === "left" ? "Left " : "Right ") : "";
  const base = (m ? m[2]! : key).trim();
  const map = platform === "macos" ? MAC : PC;
  const label = map[base.toLowerCase()] ?? (base.length === 1 ? base.toUpperCase() : base);
  return side + (platform === "linux" && label === "Win" ? "Super" : label);
}

type KeycapProps = { keys: string[]; platform: Platform };

/** A shortcut rendered as keycaps, with a readable label for assistive tech. */
export function Keycap({ keys, platform }: KeycapProps) {
  const labels = keys.map((k) => keyLabel(k, platform));
  const spoken = labels.map((l) => l.split(" ").map((p) => SPOKEN[p] ?? p).join(" ")).join(" plus ");
  return (
    <span className="vc-keys" aria-label={spoken} role="img">
      {labels.map((l, i) => (
        <kbd key={i} className="vc-keycap" aria-hidden="true">
          {l}
        </kbd>
      ))}
    </span>
  );
}
