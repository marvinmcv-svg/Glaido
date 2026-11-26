// Build CSS variables and a typed JS export from tokens.json, and check
// WCAG 2.2 AA contrast for the main text/background pairs in both themes.
// `--check` validates without writing.
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const tokens = JSON.parse(readFileSync(join(root, "tokens.json"), "utf8"));
const checkOnly = process.argv.includes("--check");

const resolve = (value) =>
  value.replace(/^\{([a-z]+)\.([A-Za-z0-9]+)\}$/, (_, group, key) => {
    const v = tokens.color[group]?.[key];
    if (!v) throw new Error(`Unknown token reference ${value}`);
    return v;
  });

const themes = Object.fromEntries(
  Object.entries(tokens.themes).map(([name, t]) => [
    name,
    Object.fromEntries(Object.entries(t).map(([k, v]) => [k, resolve(v)])),
  ]),
);

// --- contrast checks -------------------------------------------------------
const lum = (hex) => {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((c) =>
    c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4,
  );
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
export const contrast = (a, b) => {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
};
const pairs = [
  ["text-default", "bg-primary", 4.5],
  ["text-default", "bg-card", 4.5],
  ["text-muted", "bg-primary", 4.5],
  ["text-subtle", "bg-card", 4.5],
  ["text-accent", "bg-primary", 4.5],
  ["button-primary-text", "button-primary", 4.5],
  ["button-attention-text", "button-attention", 4.5],
  ["focus-ring", "bg-primary", 3],
  ["border-accent", "bg-card", 3],
];
const failures = [];
for (const [theme, t] of Object.entries(themes)) {
  for (const [fg, bg, min] of pairs) {
    const c = contrast(t[fg].slice(0, 7), t[bg].slice(0, 7));
    if (c < min) failures.push(`${theme}: ${fg} on ${bg} = ${c.toFixed(2)} (< ${min})`);
  }
}
if (failures.length) {
  console.error("Contrast check failed:\n" + failures.join("\n"));
  process.exit(1);
}

// --- outputs ---------------------------------------------------------------
const vars = (obj, prefix, indent = "  ") =>
  Object.entries(obj).map(([k, v]) => `${indent}--${prefix}${k}: ${v};`).join("\n");
const shared = [
  vars(tokens.color.neutral, "neutral-"),
  vars(tokens.color.accent, "accent-"),
  vars(tokens.color.status, "status-"),
  vars(tokens.radius, "radius-"),
  vars(tokens.space, "space-"),
  vars(tokens.font, "font-"),
  vars(tokens.motion, "motion-"),
].join("\n");

const css = `/* Generated from tokens.json by scripts/build.mjs. Do not edit. */
:root {
${shared}
}
:root, [data-theme="dark"] {
  color-scheme: dark;
${vars(themes.dark, "")}
}
[data-theme="light"] {
  color-scheme: light;
${vars(themes.light, "")}
}
@media (prefers-color-scheme: light) {
  :root:not([data-theme]) {
    color-scheme: light;
${vars(themes.light, "", "    ")}
  }
}
@media (prefers-reduced-motion: reduce) {
  :root { --motion-fast: 0ms; --motion-base: 0ms; --motion-slow: 0ms; }
}
`;

const data = { color: tokens.color, themes, radius: tokens.radius, space: tokens.space, font: tokens.font, motion: tokens.motion };
const js = `// Generated from tokens.json. Do not edit.\nexport const tokens = ${JSON.stringify(data, null, 2)};\nexport default tokens;\n`;
const dts = `// Generated from tokens.json. Do not edit.
export type ThemeName = ${Object.keys(themes).map((t) => JSON.stringify(t)).join(" | ")};
export type SemanticToken = ${Object.keys(themes.dark).map((t) => JSON.stringify(t)).join(" | ")};
export declare const tokens: {
  color: { accent: Record<string, string>; neutral: Record<string, string>; status: Record<string, string> };
  themes: Record<ThemeName, Record<SemanticToken, string>>;
  radius: Record<"sm" | "md" | "lg" | "full", string>;
  space: Record<string, string>;
  font: { sans: string; mono: string };
  motion: Record<string, string>;
};
export default tokens;
`;

if (checkOnly) {
  console.log("Tokens valid; contrast checks passed.");
} else {
  mkdirSync(join(root, "dist"), { recursive: true });
  writeFileSync(join(root, "dist/tokens.css"), css);
  writeFileSync(join(root, "dist/index.js"), js);
  writeFileSync(join(root, "dist/index.d.ts"), dts);
  console.log("Wrote dist/tokens.css, dist/index.js, dist/index.d.ts");
}
