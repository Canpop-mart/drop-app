/**
 * One place that turns a rejected `launch_game` / `launch_moonlight` invoke
 * into something worth showing a player, shared by the desktop game-detail
 * modal and the Big Picture dialog.
 *
 * It exists because the two surfaces had drifted into two different (and
 * differently incomplete) lists of `errMsg.includes(...)` checks, and because
 * several launch paths had no user-facing message at all — a `console.error`
 * and an app that looks like it ignored the button.
 *
 * The reason strings come from `ProcessError`'s `Display` impl
 * (src-tauri/process/src/error.rs), which is already written to be read by a
 * player. This layer only adds the things the backend cannot know: which
 * screen to send someone to, and an honest fallback for the cases where the
 * cause genuinely isn't known.
 */

/**
 * Where to send someone whose launch failed for an unknown reason. Names the
 * bug-report flow because that is the only log-reaching action that actually
 * exists on BOTH surfaces (the header's bug icon on desktop, Settings on Big
 * Picture), and both attach the client log automatically.
 */
const LOG_HINT =
  "Submitting a bug report from Drop attaches the launcher log automatically.";

export type LaunchFailure = {
  title: string;
  /** Full sentence(s). Safe to render as plain text on either surface. */
  message: string;
  /**
   * True when Drop knows the cause. False means the message says so rather
   * than dressing an unknown failure up as a diagnosis.
   */
  identified: boolean;
};

function messageOf(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  // Tauri serialises ProcessError as a bare string, but a non-command
  // rejection can be anything.
  return String(err);
}

/**
 * True for the one failure that should never reach a dialog: a second Play
 * press landing while the first launch is still starting the game.
 */
export function isBenignLaunchError(err: unknown): boolean {
  const msg = messageOf(err);
  return msg.includes("AlreadyRunning") || msg.includes("already running");
}

export function describeLaunchFailure(
  err: unknown,
  gameName?: string,
): LaunchFailure {
  const msg = messageOf(err);
  const named = gameName ? `"${gameName}"` : "This game";
  const identified = (title: string, message: string): LaunchFailure => ({
    title,
    message,
    identified: true,
  });

  // The emulator/dependency cases. The backend already names the emulator and
  // says what to do, so pass its sentence through rather than paraphrasing it.
  if (msg.includes("isn't installed yet")) {
    return identified("Emulator not installed", msg);
  }
  if (msg.includes("setup hasn't been finished")) {
    return identified("Emulator setup not finished", msg);
  }
  if (msg.includes("hasn't synced")) {
    return identified("Missing a required program", msg);
  }

  // Missing files.
  if (msg.includes("game file is missing")) {
    return identified("Game file missing", msg);
  }
  if (msg.includes("Launch file is missing")) {
    return identified("Launch file missing", msg);
  }
  if (msg.includes("Game not installed")) {
    return identified(
      "Not installed",
      `${named} isn't installed on this device. Install it from your library first.`,
    );
  }

  // Compatibility layer.
  if (msg.includes("exec format error") || msg.includes("os error 8")) {
    return identified(
      "Needs a compatibility layer",
      `${named} is a Windows program and can't run natively on Linux. Set a Proton version in Settings, and check the game's platform is set to Windows.`,
    );
  }
  if (msg.includes("NoCompat") || msg.includes("compatibility layer")) {
    return identified(
      "No Proton found",
      "Drop couldn't find a Proton build to run this with. Set a default Proton path in Settings, or add an override for this game.",
    );
  }
  if (msg.includes("InvalidPlatform") || msg.includes("cannot be played on the current platform")) {
    return identified(
      "Wrong platform",
      `${named} can't run on this device as configured. Check the game's platform setting, and that a compatibility layer is available if it needs one.`,
    );
  }

  // Configuration.
  if (msg.includes("Could not format template")) {
    return identified(
      "Launch command is invalid",
      `${named} has a custom launch template Drop couldn't expand. Clear or fix it in the game's Configure screen, under Launch.`,
    );
  }
  if (msg.includes("Invalid arguments in command")) {
    return identified(
      "Launch command is invalid",
      `${named} has a launch command Drop couldn't parse: ${msg}`,
    );
  }
  if (msg.includes("Invalid game version")) {
    return identified(
      "Version data missing",
      `Drop has no version details for ${named}. Reconnect to your Drop server so the library can sync, then try again.`,
    );
  }

  // Everything else. Say that, and say where to look — do not guess a cause.
  return {
    title: "Launch failed",
    message: `${named} didn't start, and Drop couldn't tell why. The launcher reported: ${msg}. ${LOG_HINT}`,
    identified: false,
  };
}
