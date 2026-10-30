/**
 * The one-press Install rule, shared by the Big Picture home screen (hero
 * button and context menu) and the Big Picture game page's Install.
 *
 * `fetch_game_version_options` returns the server's versions newest first,
 * already filtered to platforms this device can run, so the first option is
 * the pick. It is by definition the newest for its platform, so it follows
 * updates (the `isLatestForPlatform` rule in update-review.ts).
 * No Vue, no Tauri: `node --test main/tests/quick-install.test.ts`.
 */
export interface QuickInstallArgs {
  versionId: string;
  targetPlatform: string;
  installDir: number;
  enableUpdates: boolean;
}

export function quickInstallArgs(
  options: readonly { versionId: string; platform: string }[],
): QuickInstallArgs | null {
  const vo = options[0];
  if (!vo) return null;
  return {
    versionId: vo.versionId,
    targetPlatform: vo.platform,
    // The first install directory, as the game page has always used.
    installDir: 0,
    enableUpdates: true,
  };
}
