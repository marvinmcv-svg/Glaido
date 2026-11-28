import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Platform } from "@vocalis/ui";
import { useCallback, useEffect, useState } from "react";

export type Hotkeys = { dictation: string; dictationHandsfree: string };
export type SettingsView = {
  languages: string[];
  microphone: string | null;
  hotkeys: Hotkeys;
  setupCompleted: boolean;
  pasteIntoStartingApp: boolean;
  platform: Platform;
};

/** `"left_ctrl+space"` → `["left_ctrl", "space"]` for the Keycap component. */
export const chordKeys = (setting: string): string[] => setting.split("+").filter(Boolean);

/** Settings from the engine; `save` applies a command and keeps the result. */
export function useSettings() {
  const [settings, setSettings] = useState<SettingsView | null>(null);
  useEffect(() => {
    if (isTauri()) void invoke<SettingsView>("settings_get").then(setSettings);
  }, []);
  const save = useCallback(async (command: string, args: Record<string, unknown>) => {
    const next = await invoke<SettingsView>(command, args);
    setSettings(next);
    return next;
  }, []);
  return { settings, save };
}
