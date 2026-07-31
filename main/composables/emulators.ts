import { invoke } from "@tauri-apps/api/core";

/**
 * One installed emulator host (the RetroArch install that backs the console
 * rows, or a standalone emulator like Ryujinx). Returned by the client-side
 * `list_installed_emulators` Tauri command — see `src-tauri/src/emulators.rs`.
 */
export interface EmulatorHost {
  /** Host game id — the same id the uninstall / open-folder commands take. */
  id: string;
  name: string;
  installDir: string;
  /** Icon object id for `useObject()`; empty string when there's no icon. */
  iconObjectId: string;
  /** RetroArch install — gates the cores UI (cores are a RetroArch concept). */
  retroarch: boolean;
  /** Core library filenames in `<install>/cores/` (`*_libretro.dll` / `.so`). */
  cores: string[];
}

/**
 * List the installed emulator hosts. Client-only: emulator installs live on
 * the desktop client, so this reads local install state rather than the server
 * library (which is why a host can appear here even when it isn't in the grid).
 */
export function listInstalledEmulators(): Promise<EmulatorHost[]> {
  return invoke<EmulatorHost[]>("list_installed_emulators");
}
