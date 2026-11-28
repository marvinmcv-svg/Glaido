/** Light/dark theme override (the design tokens follow the system until one is chosen). */

export type Theme = "light" | "dark";
const KEY = "vocalis.theme";

const isTheme = (t: unknown): t is Theme => t === "light" || t === "dark";

export function effectiveTheme(): Theme {
  const t = document.documentElement.dataset.theme;
  if (isTheme(t)) return t;
  return window.matchMedia?.("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

export function applyStoredTheme(): void {
  try {
    const t = localStorage.getItem(KEY);
    if (isTheme(t)) document.documentElement.dataset.theme = t;
  } catch {
    // Storage unavailable: follow the system.
  }
}

export function toggleTheme(): Theme {
  const next: Theme = effectiveTheme() === "dark" ? "light" : "dark";
  document.documentElement.dataset.theme = next;
  try {
    localStorage.setItem(KEY, next);
  } catch {
    // Not saved; still applied for this session.
  }
  return next;
}
