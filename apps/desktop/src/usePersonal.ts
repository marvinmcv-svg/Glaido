import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { errorText, previewBackend } from "./personal";

type Call = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

// One in-memory backend per page load, so preview edits survive page switches.
const preview = previewBackend();
const call: Call = (cmd, args) => (isTauri() ? invoke(cmd, args) : preview(cmd, args));

/**
 * A personalization list (`dictionary_list` or `snippets_list`) that reloads on
 * `personal-changed`, plus a `run` that reports failures as a short sentence.
 */
export function usePersonal<T>(listCommand: string) {
  const [items, setItems] = useState<T[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      setItems(await call<T[]>(listCommand));
      setError(null);
    } catch (e) {
      setItems((i) => i ?? []);
      setError(errorText(e));
    }
  }, [listCommand]);

  useEffect(() => {
    void reload();
    if (!isTauri()) return;
    const off = listen("personal-changed", () => void reload());
    return () => void off.then((f) => f());
  }, [reload]);

  /** Run a command; resolves to an error sentence, or `null` on success. */
  const run = useCallback(
    async (cmd: string, args?: Record<string, unknown>): Promise<string | null> => {
      try {
        await call(cmd, args);
        // The event reloads in the app; the preview has no events.
        if (!isTauri()) await reload();
        return null;
      } catch (e) {
        return errorText(e);
      }
    },
    [reload],
  );

  return { items, error, setError, run, preview: !isTauri() };
}
