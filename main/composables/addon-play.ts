/*
State for an addon's save panel on a game's page, shared by the desktop game
page and Big Picture (each mounts its own instance). The pure parts live in
addon-play-logic.ts; the file work happens in Rust (src-tauri/src/addons.rs),
which also refuses to touch the save while the game runs and holds launches
off while it works.

Check-out is three steps. Rust first checks everything local that would stop
it (game open, no save folder yet, several profiles, play never checked in)
BEFORE the league's lock is taken, so nothing local leaves a player holding
the save. Then drop-server takes the lock and answers with the version and
its SHA-1, and Rust downloads exactly that version into the game's save
folder. If that last step fails, the player still has the lock, so the row
offers "Check out" again (taking it again is allowed). There is no release
button by design.

One request at a time per panel. Every result is said on its row.
*/
import { invoke } from "@tauri-apps/api/core";
import {
  ctxKey,
  isMissingRoute,
  parseGameAddons,
  parseOutcome,
  toProblem,
  type GameAddon,
  type LocalSave,
  type PanelStatus,
  type PlayContext,
} from "./addon-play-logic";
import { useServerApi } from "./use-server-api";

export type RowNotice = { key: string; kind: "ok" | "error"; message: string };

export function useAddonPlay(gameId: () => string, running: () => boolean) {
  const api = useServerApi();

  const status = ref<PanelStatus>("loading");
  const addons = ref<GameAddon[]>([]);
  const local = ref<Record<string, LocalSave | null>>({});
  const busy = ref<string | null>(null);
  const confirming = ref<string | null>(null);
  const notice = ref<RowNotice | null>(null);

  function find(key: string): { addon: GameAddon; ctx: PlayContext } | null {
    for (const addon of addons.value) {
      for (const ctx of addon.contexts) {
        if (ctxKey(addon.name, ctx.id) === key) return { addon, ctx };
      }
    }
    return null;
  }

  async function readLocal() {
    const next: Record<string, LocalSave | null> = {};
    await Promise.all(
      addons.value.flatMap((addon) =>
        addon.contexts.map(async (ctx) => {
          try {
            next[ctxKey(addon.name, ctx.id)] = await invoke<LocalSave>(
              "addon_local_state",
              {
                gameId: gameId(),
                addon: addon.name,
                context: ctx.id,
                slot: ctx.slot,
              },
            );
          } catch (e) {
            console.warn("[addons] local state:", e);
            next[ctxKey(addon.name, ctx.id)] = null;
          }
        }),
      ),
    );
    local.value = next;
  }

  /** Reload from the server. `quiet` keeps the rows up while it runs. */
  async function load(quiet = false) {
    if (!quiet) status.value = "loading";
    try {
      addons.value = parseGameAddons(await api.addons.forGame(gameId()));
      status.value = "ready";
    } catch (e) {
      if (isMissingRoute(e)) {
        addons.value = [];
        status.value = "ready";
      } else {
        console.warn("[addons] panel load failed:", e);
        if (!quiet) status.value = "error";
        return;
      }
    }
    await readLocal();
  }

  async function install(
    key: string,
    replaceUnsent: boolean,
    version: number,
    sha1: string,
  ) {
    const found = find(key);
    if (!found) return;
    try {
      await invoke("addon_install_save", {
        gameId: gameId(),
        addon: found.addon.name,
        context: found.ctx.id,
        version,
        sha1,
        slot: found.ctx.slot,
        replaceUnsent,
      });
      confirming.value = null;
      notice.value = {
        key,
        kind: "ok",
        message: "Checked out. The save is ready to play.",
      };
    } catch (e) {
      const p = toProblem(e);
      if (p.code === "unsent_changes") confirming.value = key;
      notice.value = { key, kind: "error", message: p.message };
    }
  }

  /**
   * Check out (take the lock, then put the save on this device). With
   * `replaceUnsent` the local save is replaced even if it holds play never
   * checked in; a copy is kept in Drop's backups either way.
   */
  async function checkOut(key: string, replaceUnsent = false) {
    if (busy.value || !find(key)) return;
    busy.value = key;
    notice.value = null;
    try {
      const { addon, ctx } = find(key)!;
      try {
        const pre = await invoke<{ unsent: boolean }>("addon_preflight", {
          gameId: gameId(),
          addon: addon.name,
          context: ctx.id,
          slot: ctx.slot,
        });
        if (pre.unsent && !replaceUnsent) {
          confirming.value = key;
          notice.value = {
            key,
            kind: "error",
            message:
              "The save on this device has play that was never checked in.",
          };
          return;
        }
      } catch (e) {
        notice.value = { key, kind: "error", message: toProblem(e).message };
        return;
      }
      let outcome;
      try {
        outcome = parseOutcome(await api.addons.checkout(addon.name, ctx.id));
      } catch (e) {
        console.warn("[addons] checkout failed:", e);
        notice.value = {
          key,
          kind: "error",
          message: "Couldn't reach your Drop server. Try again.",
        };
        return;
      }
      if (!outcome.ok) {
        notice.value = { key, kind: "error", message: outcome.message };
        return;
      }
      if (outcome.version === null || outcome.sha1 === null) {
        notice.value = {
          key,
          kind: "error",
          message: "The server's answer was incomplete. Try again.",
        };
        return;
      }
      await install(key, replaceUnsent, outcome.version, outcome.sha1);
    } finally {
      busy.value = null;
      await load(true);
    }
  }

  async function checkIn(key: string) {
    const found = find(key);
    if (busy.value || !found) return;
    busy.value = key;
    notice.value = null;
    confirming.value = null;
    try {
      const raw = await invoke("addon_check_in", {
        gameId: gameId(),
        addon: found.addon.name,
        context: found.ctx.id,
        slot: found.ctx.slot,
      });
      const outcome = parseOutcome(raw);
      notice.value = outcome.ok
        ? {
            key,
            kind: "ok",
            message: outcome.unchanged
              ? "Checked in. Nothing had changed since you checked out."
              : "Checked in. Thanks for passing it on.",
          }
        : { key, kind: "error", message: outcome.message };
    } catch (e) {
      notice.value = { key, kind: "error", message: toProblem(e).message };
    } finally {
      busy.value = null;
      await load(true);
    }
  }

  function cancelReplace() {
    confirming.value = null;
    notice.value = null;
  }

  // When the game closes, look at the save again so a row with play waiting
  // to go back to the league says so.
  watch(running, (now, before) => {
    if (before && !now && status.value === "ready") readLocal();
  });

  return {
    status,
    addons,
    local,
    busy,
    confirming,
    notice,
    load,
    checkOut,
    checkIn,
    cancelReplace,
  };
}
