/**
 * Shared bits for the "which file does this game run" picker.
 *
 * Both surfaces (the desktop Configure modal and the Big Picture options menu)
 * read from here rather than each calling `scan_game_executables` with its own
 * idea of the shape. The Proton cycler drifted between the two surfaces once
 * already by being declared twice; this is that lesson applied up front.
 */

import { invoke } from "@tauri-apps/api/core";

/** One executable the backend found inside the install directory. */
export interface ExecutableCandidate {
  /** Path relative to the install dir, forward slashes. The stored value. */
  relativePath: string;
  fileName: string;
  size: number;
  /** The entry the game launches right now. */
  isCurrent: boolean;
  /** Uninstaller, redistributable or crash handler. Shown, but ranked last. */
  likelyNoise: boolean;
}

export type ExecutableUnsupportedReason =
  | "notInstalled"
  | "emulated"
  | "noVersionData";

export interface ExecutableScanResult {
  supported: boolean;
  unsupportedReason: ExecutableUnsupportedReason | null;
  /** What the server's launch config runs, relative to the install dir. */
  automatic: string | null;
  /** The override saved on this device, if any. */
  selected: string | null;
  candidates: ExecutableCandidate[];
}

/** Why the picker is not offered for this game. */
export const EXECUTABLE_UNSUPPORTED_TEXT: Record<
  ExecutableUnsupportedReason,
  string
> = {
  notInstalled: "Install the game first to see the files it can run.",
  emulated:
    "This game runs through an emulator, so the file it starts belongs to the emulator, not to the game.",
  noVersionData:
    "Drop has no launch details stored for this game yet, so there is nothing to compare against.",
};

/**
 * Whether a launch string template still passes the chosen executable through.
 *
 * The launcher resolves the picked executable first and then runs the whole
 * command through this template (STEP 5 of `process_manager/launch.rs`). The
 * formatter only substitutes `{}` / `{0}` (the full launch string), `{exe}`,
 * `{abs_exe}`, `{dir}` and `{rom}`, so a template holding none of the first
 * four throws the launch string away and runs its own text instead. The pick
 * then has no effect at all, silently.
 *
 * `{dir}` and `{rom}` deliberately do not count: neither one carries the
 * executable. A blank template does not count either, which matches the
 * warning already on the template field itself.
 */
export function launchTemplateUsesExecutable(
  template: string | null | undefined,
): boolean {
  const value = template ?? "";
  return (
    value.includes("{}") ||
    value.includes("{0}") ||
    value.includes("{exe}") ||
    value.includes("{abs_exe}")
  );
}

export async function scanGameExecutables(
  gameId: string,
): Promise<ExecutableScanResult> {
  return await invoke<ExecutableScanResult>("scan_game_executables", {
    gameId,
  });
}

/** Sizes are the main hint for picking the right binary, so keep them short. */
export function formatExecutableSize(bytes: number): string {
  if (bytes <= 0) return "";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value < 10 && unit > 0 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}
