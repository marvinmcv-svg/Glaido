import type { Platform } from "@vocalis/ui";

export function currentPlatform(): Platform {
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("mac")) return "macos";
  if (ua.includes("win")) return "windows";
  return "linux";
}

/** Default hotkeys (docs/MASTER_PROMPT.md Appendix A, our documented defaults). */
export const DEFAULT_HOTKEYS: Record<
  Platform,
  { dictation: string[]; handsFree: string[]; commands: string[] }
> = {
  macos: { dictation: ["fn"], handsFree: ["fn", "space"], commands: ["right-option"] },
  windows: {
    dictation: ["ctrl", "cmd"],
    handsFree: ["ctrl", "cmd", "shift"],
    commands: ["left-ctrl", "left-alt"],
  },
  linux: {
    dictation: ["left-ctrl", "space"],
    handsFree: ["left-ctrl", "shift", "space"],
    commands: ["ctrl", "alt"],
  },
};
