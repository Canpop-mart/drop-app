/**
 * An addon's save panel on a game page: the server's reply is checked, a
 * failed load is never mistaken for "no league", and each league row offers
 * only what makes sense for who holds the save and what this device has.
 *
 *   node --test main/tests/addon-play.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  contextView,
  ctxKey,
  isMissingRoute,
  parseGameAddons,
  parseOutcome,
  toProblem,
  type LocalSave,
  type PlayContext,
} from "../composables/addon-play-logic.ts";

const slot = { root: "winLocalAppData", file: ["a"] };

function ctx(over: Partial<PlayContext> = {}): PlayContext {
  return {
    id: "1",
    title: "Big Ten",
    state: "free",
    holderName: null,
    holderIsMe: false,
    lockExpiresAt: null,
    lockBase: 4,
    version: 4,
    week: 3,
    season: 2026,
    canCheckOut: true,
    canCheckIn: false,
    slot,
    ...over,
  };
}

const mineHere: LocalSave = {
  baseVersion: 4,
  handedIn: false,
  saveExists: true,
  unsent: false,
  problem: null,
};

const idle = { confirming: null, busy: null, running: false };

test("a reply that isn't { addons: [] } is an error, not 'no league'", () => {
  assert.throws(() => parseGameAddons(undefined));
  assert.throws(() => parseGameAddons([]));
  assert.throws(() => parseGameAddons({ addons: "x" }));
  assert.deepEqual(parseGameAddons({ addons: [] }), []);
});

test("malformed addons and leagues are dropped, the rest kept", () => {
  const out = parseGameAddons({
    addons: [
      null,
      { name: "Bad Name" },
      {
        name: "ncaa14",
        title: "NCAA 14",
        site: "javascript:alert(1)",
        available: true,
        contexts: [
          { id: 1, state: "free", slot, title: "Big Ten" },
          { id: "0", state: "free", slot },
          { id: "2", state: "weird", slot },
          { id: "3", state: "free" },
          { id: "../4", state: "free", slot },
        ],
      },
      {
        name: "down",
        available: false,
        contexts: [{ id: "1", state: "free", slot }],
      },
    ],
  });
  assert.equal(out.length, 2);
  assert.equal(out[0].site, null);
  assert.deepEqual(
    out[0].contexts.map((c) => c.id),
    ["1"],
  );
  assert.equal(out[1].available, false);
  assert.deepEqual(out[1].contexts, []);
});

test("only a 404 counts as a server without addon panels", () => {
  assert.ok(
    isMissingRoute(
      new Error("API api/v1/games/x/addons failed: 404 Not Found"),
    ),
  );
  assert.ok(
    !isMissingRoute(new Error("API x failed: 500 Internal Server Error")),
  );
  assert.ok(!isMissingRoute(new TypeError("Failed to fetch")));
});

test("server outcomes and Rust errors are read safely", () => {
  assert.deepEqual(
    parseOutcome({ ok: true, version: 5, sha1: "a".repeat(40) }),
    {
      ok: true,
      version: 5,
      sha1: "a".repeat(40),
      unchanged: false,
    },
  );
  assert.equal(
    (parseOutcome({ ok: true, version: 5, sha1: "nope" }) as any).sha1,
    null,
  );
  const refused = parseOutcome({
    ok: false,
    code: "held",
    message: "Someone has it.",
  });
  assert.deepEqual(refused, {
    ok: false,
    code: "held",
    message: "Someone has it.",
  });
  assert.equal(parseOutcome(null).ok, false);
  assert.deepEqual(toProblem({ code: "play_first", message: "Play once." }), {
    code: "play_first",
    message: "Play once.",
  });
  assert.equal(toProblem("boom").code, "failed");
});

test("free: check out", () => {
  const v = contextView("ncaa14", ctx(), null, idle);
  assert.equal(v.primary, "check_out");
  assert.equal(v.secondary, null);
});

test("held by someone else or no save: nothing to press", () => {
  const held = contextView(
    "ncaa14",
    ctx({ state: "held", holderName: "Sam" }),
    null,
    idle,
  );
  assert.equal(held.primary, null);
  assert.match(held.line, /Sam/);
  assert.equal(
    contextView("ncaa14", ctx({ state: "no_save" }), null, idle).primary,
    null,
  );
});

test("mine but not on this device: bring it here", () => {
  const mine = ctx({ state: "mine", holderIsMe: true });
  assert.equal(contextView("ncaa14", mine, null, idle).primary, "check_out");
  assert.equal(
    parseGameAddons({
      addons: [
        {
          name: "n",
          available: true,
          contexts: [{ id: "1", state: "mine", slot, lockBase: 4 }],
        },
      ],
    })[0].contexts[0].lockBase,
    4,
  );
  assert.equal(
    contextView("ncaa14", mine, { ...mineHere, baseVersion: null }, idle)
      .primary,
    "check_out",
  );
  assert.equal(
    contextView("ncaa14", mine, { ...mineHere, handedIn: true }, idle).primary,
    "check_out",
  );
});

test("mine on this device, not played: nothing to send", () => {
  const mine = ctx({ state: "mine", holderIsMe: true });
  const clean = contextView("ncaa14", mine, mineHere, idle);
  assert.equal(clean.primary, null);
  assert.equal(clean.secondary, null);
});

test("mine on this device with play: check in, urgent once the game is closed", () => {
  const mine = ctx({ state: "mine", holderIsMe: true });
  const played = contextView(
    "ncaa14",
    mine,
    { ...mineHere, unsent: true },
    idle,
  );
  assert.equal(played.primary, "check_in");
  assert.equal(played.secondary, "replace");
  assert.equal(played.urgent, true);
  const stillPlaying = contextView(
    "ncaa14",
    mine,
    { ...mineHere, unsent: true },
    {
      ...idle,
      running: true,
    },
  );
  assert.equal(stillPlaying.urgent, false);
});

test("checked out again elsewhere since: this device must not check in", () => {
  // Device A took v4; the player then checked out again on B after a new
  // version, so the lock's base is 6. A's play (even if any) is stale.
  const moved = ctx({ state: "mine", holderIsMe: true, lockBase: 6 });
  const v = contextView("ncaa14", moved, { ...mineHere, unsent: true }, idle);
  assert.equal(v.primary, "check_out");
  assert.equal(
    contextView(
      "ncaa14",
      ctx({ state: "mine", lockBase: null }),
      mineHere,
      idle,
    ).primary,
    "check_out",
  );
});

test("busy and confirming follow the row's key, across addons", () => {
  const key = ctxKey("ncaa14", "1");
  const v = contextView("ncaa14", ctx(), null, {
    confirming: key,
    busy: key,
    running: false,
  });
  assert.ok(v.busy && v.confirming && v.locked);
  const other = contextView("other", ctx(), null, {
    confirming: key,
    busy: key,
    running: false,
  });
  assert.ok(!other.busy && !other.confirming && other.locked);
});
