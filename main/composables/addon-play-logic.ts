/*
Pure logic for an addon's save panel on a game's page (the league's dynasty
check-out / check-in), shared by the desktop game page
(components/game-detail/AddonActions.vue) and Big Picture
(components/bigpicture/game-detail/BpmAddonActions.vue).
composables/addon-play.ts wires it to drop-server and to the Rust commands
in src-tauri/src/addons.rs. Kept free of Vue and Nuxt so node --test can run
it: main/tests/addon-play.test.ts.

Wording here is placeholder; the owner rewrites player-facing text.
*/

/** Where the save goes. Passed through to Rust untouched; Rust checks it. */
export type SaveSlot = Record<string, unknown>;

export type PlayContext = {
  id: string;
  title: string;
  state: "free" | "mine" | "held" | "no_save";
  holderName: string | null;
  holderIsMe: boolean;
  lockExpiresAt: string | null;
  /** The version the current lock was taken from (see contextView). */
  lockBase: number | null;
  version: number | null;
  week: number | null;
  season: number | null;
  canCheckOut: boolean;
  canCheckIn: boolean;
  slot: SaveSlot;
};

export type GameAddon = {
  name: string;
  title: string;
  site: string | null;
  available: boolean;
  contexts: PlayContext[];
};

export type AddonProblem = { code: string; message: string };

/** What this device knows about its copy (Rust `addon_local_state`). */
export type LocalSave = {
  baseVersion: number | null;
  handedIn: boolean;
  saveExists: boolean;
  unsent: boolean;
  problem: AddonProblem | null;
};

export type PanelStatus = "loading" | "error" | "ready";

const NAME = /^[a-z0-9][a-z0-9-]{0,31}$/;
const CONTEXT = /^[1-9][0-9]{0,8}$/;
const STATES = new Set(["free", "mine", "held", "no_save"]);

function str(v: unknown): string {
  return typeof v === "string" ? v : "";
}
function strOrNull(v: unknown): string | null {
  return typeof v === "string" && v ? v : null;
}
function intOrNull(v: unknown): number | null {
  return typeof v === "number" && Number.isInteger(v) ? v : null;
}

function parseContext(raw: unknown): PlayContext | null {
  if (!raw || typeof raw !== "object") return null;
  const o = raw as Record<string, unknown>;
  const id = typeof o.id === "number" ? String(o.id) : str(o.id);
  if (!CONTEXT.test(id) || !STATES.has(str(o.state))) return null;
  if (!o.slot || typeof o.slot !== "object" || Array.isArray(o.slot))
    return null;
  return {
    id,
    title: str(o.title) || `League ${id}`,
    state: str(o.state) as PlayContext["state"],
    holderName: strOrNull(o.holderName),
    holderIsMe: o.holderIsMe === true,
    lockExpiresAt: strOrNull(o.lockExpiresAt),
    lockBase: intOrNull(o.lockBase),
    version: intOrNull(o.version),
    week: intOrNull(o.week),
    season: intOrNull(o.season),
    canCheckOut: o.canCheckOut === true,
    canCheckIn: o.canCheckIn === true,
    slot: o.slot as SaveSlot,
  };
}

/**
 * drop-server's `{ addons }`, checked. A malformed entry is dropped; a reply
 * that isn't the right shape at all throws, so the caller shows "couldn't
 * load", never "nothing here".
 */
export function parseGameAddons(raw: unknown): GameAddon[] {
  if (!raw || typeof raw !== "object" || !Array.isArray((raw as any).addons))
    throw new Error("addon panel reply is not { addons: [] }");
  const out: GameAddon[] = [];
  for (const entry of (raw as { addons: unknown[] }).addons) {
    if (!entry || typeof entry !== "object") continue;
    const o = entry as Record<string, unknown>;
    const name = str(o.name);
    if (!NAME.test(name)) continue;
    const site = str(o.site);
    const available = o.available === true;
    out.push({
      name,
      title: str(o.title) || name,
      site: /^https?:\/\//i.test(site) ? site : null,
      available,
      contexts:
        available && Array.isArray(o.contexts)
          ? o.contexts.map(parseContext).filter((c): c is PlayContext => !!c)
          : [],
    });
  }
  return out;
}

/** A server too old to have addon panels answers 404: that is "none". */
export function isMissingRoute(e: unknown): boolean {
  return /failed: 404\b/.test(String((e as Error)?.message ?? e));
}

/** A Rust `AddonError` (or anything thrown) as a problem to show. */
export function toProblem(e: unknown): AddonProblem {
  if (e && typeof e === "object" && typeof (e as any).code === "string") {
    return {
      code: (e as any).code,
      message: str((e as any).message) || "Something went wrong.",
    };
  }
  return { code: "failed", message: String((e as Error)?.message ?? e) };
}

/** drop-server's `{ ok, ... }` answer to a check-out or check-in. */
export function parseOutcome(raw: unknown):
  | {
      ok: true;
      version: number | null;
      sha1: string | null;
      unchanged: boolean;
    }
  | { ok: false; code: string; message: string } {
  const o = (raw && typeof raw === "object" ? raw : {}) as Record<
    string,
    unknown
  >;
  if (o.ok === true) {
    const sha1 = str(o.sha1);
    return {
      ok: true,
      version: intOrNull(o.version),
      sha1: /^[0-9a-f]{40}$/.test(sha1) ? sha1 : null,
      unchanged: o.unchanged === true,
    };
  }
  return {
    ok: false,
    code: str(o.code) || "failed",
    message: str(o.message) || "Something went wrong. Try again.",
  };
}

/** One league row's key across every addon on the page. */
export function ctxKey(addon: string, ctx: string): string {
  return `${addon}/${ctx}`;
}

export type Action = "check_out" | "check_in" | "replace";

export type ContextView = {
  /** The main button, if any. */
  primary: Action | null;
  /** A second, quieter button. */
  secondary: Action | null;
  /** One line about who has the save. */
  line: string;
  /** Draw attention to the main button (play waiting to be checked in). */
  urgent: boolean;
  /** "Replace the save on this device?" rows are open. */
  confirming: boolean;
  busy: boolean;
  /** Something is in flight anywhere in the panel. */
  locked: boolean;
};

/**
 * What a league row offers. `local` is null while it's still being read (or
 * couldn't be): the row then offers what the server allows and the Rust side
 * refuses anything unsafe.
 */
export function contextView(
  addon: string,
  ctx: PlayContext,
  local: LocalSave | null,
  opts: { confirming: string | null; busy: string | null; running: boolean },
): ContextView {
  const key = ctxKey(addon, ctx.id);
  const confirming = opts.confirming === key;
  const busy = opts.busy === key;
  const base = {
    urgent: false,
    confirming,
    busy,
    locked: opts.busy !== null,
  };
  const when = ctx.week !== null ? ` (week ${ctx.week})` : "";
  switch (ctx.state) {
    case "no_save":
      return {
        ...base,
        primary: null,
        secondary: null,
        line: "The league has no save yet.",
      };
    case "held":
      return {
        ...base,
        primary: null,
        secondary: null,
        line: `${ctx.holderName ?? "Someone else"} has the save${when}.`,
      };
    case "free":
      return {
        ...base,
        primary: "check_out",
        secondary: null,
        line: `The save is free${when}.`,
      };
    case "mine": {
      // Checked out to me, but not on this device: another PC, a check-out
      // again elsewhere since (the lock's base moved on), or a device from
      // before receipts. Bring it here; never check in from here, which
      // could end a turn being played on the other device.
      if (
        !local ||
        local.baseVersion === null ||
        local.handedIn ||
        ctx.lockBase === null ||
        local.baseVersion !== ctx.lockBase
      ) {
        return {
          ...base,
          primary: "check_out",
          secondary: null,
          line: `You have the save${when}, but not on this device.`,
        };
      }
      // On this device and not played yet: nothing to send. (There is no
      // "give it back" button by design.)
      if (!local.unsent) {
        return {
          ...base,
          primary: null,
          secondary: null,
          line: `You have the save${when}. Play, then check it in here.`,
        };
      }
      return {
        ...base,
        primary: "check_in",
        // Throw this session's play away for the league's copy.
        secondary: "replace",
        urgent: !opts.running,
        line: `You have the save${when}. Check it in when you're done.`,
      };
    }
  }
}

export function actionLabel(action: Action, busy: boolean): string {
  switch (action) {
    case "check_out":
      return busy ? "Checking out..." : "Check out";
    case "check_in":
      return busy ? "Checking in..." : "Check in";
    case "replace":
      return busy ? "Downloading..." : "Download league copy";
  }
}
