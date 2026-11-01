/*
A console's emulator goes first in that console's game list, ahead of its ROMs
(desktop console page and Big Picture console filter). Kept free of Vue, Nuxt
and Tauri so node --test can run it: main/tests/console-emulators.test.ts.
*/

/**
 * `roms` with the console's emulators put first, in `emulatorIds` order.
 *
 * Only emulators present in `emulators` are added (one that failed to load is
 * left out rather than shown as a blank tile), and an emulator that is also in
 * `roms` appears once, at the front.
 */
export function withEmulatorsFirst<T extends { game: { id: string } }>(
  roms: T[],
  emulatorIds: readonly string[] | undefined,
  emulators: ReadonlyMap<string, T>,
): T[] {
  const first: T[] = [];
  const seen = new Set<string>();
  for (const id of emulatorIds ?? []) {
    const entry = emulators.get(id);
    if (!entry || seen.has(id)) continue;
    seen.add(id);
    first.push(entry);
  }
  return [...first, ...roms.filter((r) => !seen.has(r.game.id))];
}
