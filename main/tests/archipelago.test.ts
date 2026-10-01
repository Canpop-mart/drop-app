/**
 * Archipelago leave-error parsing, session-gone detection and the localStorage
 * wrappers.
 *
 * Runs on the Node test runner with no extra dependency:
 *   node --test main/tests/archipelago.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  AP_LEAVE_NOT_RECORDED,
  addressNoteKey,
  isSessionGoneError,
  normaliseSessionCode,
  parseLeaveError,
  restoreRejoinPlan,
  safeGetItem,
  safeSetItem,
} from "../composables/archipelago-logic.ts";

test("a leave the server never recorded is told apart from a local failure", () => {
  assert.deepEqual(parseLeaveError(`${AP_LEAVE_NOT_RECORDED}HTTP 500`), {
    notRecorded: true,
    message: "HTTP 500",
  });
  // Tauri may wrap the string; the marker is found anywhere.
  assert.deepEqual(parseLeaveError(`Error: ${AP_LEAVE_NOT_RECORDED} offline`), {
    notRecorded: true,
    message: "offline",
  });
  const local = "ZeroTier isn't running or isn't responding";
  assert.deepEqual(parseLeaveError(local), { notRecorded: false, message: local });
});

test("the marker matches the Rust side", () => {
  assert.equal(AP_LEAVE_NOT_RECORDED, "ap_leave_not_recorded:");
});

test("only the stable marker means the session is gone", () => {
  assert.ok(isSessionGoneError("session_not_found"));
  assert.ok(!isSessionGoneError("Could not fetch session: HTTP 502"));
  assert.ok(!isSessionGoneError("You're not in this session."));
});

test("session codes are normalised however they're typed", () => {
  assert.equal(normaliseSessionCode("abc-123"), "ABC123");
  assert.equal(normaliseSessionCode(" ab c 1 2 3 "), "ABC123");
  assert.equal(normaliseSessionCode("--"), "");
});

test("storage helpers never throw", () => {
  const g = globalThis as { localStorage?: unknown };
  const saved = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
  try {
    // Missing storage (SSR, or a runtime without it).
    Object.defineProperty(globalThis, "localStorage", {
      value: undefined,
      configurable: true,
      writable: true,
    });
    assert.equal(safeGetItem("k"), null);
    assert.equal(safeSetItem("k", "1"), false);

    // Storage that throws (blocked or cleared site data).
    g.localStorage = {
      getItem() {
        throw new Error("SecurityError");
      },
      setItem() {
        throw new Error("QuotaExceededError");
      },
    };
    assert.equal(safeGetItem("k"), null);
    assert.equal(safeSetItem("k", "1"), false);

    // Working storage.
    const store = new Map<string, string>();
    g.localStorage = {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => void store.set(k, v),
    };
    assert.equal(safeSetItem("k", "1"), true);
    assert.equal(safeGetItem("k"), "1");
  } finally {
    if (saved) Object.defineProperty(globalThis, "localStorage", saved);
    else delete g.localStorage;
  }
});

test("restore re-joins on its own only when nothing will be asked", () => {
  // ZeroTier already answers: re-joining needs no prompt.
  assert.equal(restoreRejoinPlan({ running: true }), "auto");
  // Not running and starting it could prompt: the user presses Reconnect.
  assert.equal(
    restoreRejoinPlan({ running: false, startNeedsPrompt: true }),
    "ask",
  );
  // Linux after a reboot: Drop's daemon is down but has its capabilities, so
  // starting it asks for nothing.
  assert.equal(
    restoreRejoinPlan({ running: false, startNeedsPrompt: false }),
    "auto",
  );
  // An older build that doesn't report it: assume it could prompt.
  assert.equal(restoreRejoinPlan({ running: false }), "ask");
  // Status unreadable: never prompt on our own.
  assert.equal(restoreRejoinPlan(null), "ask");
  // Game Mode without the one-time setup: no button that can't work.
  assert.equal(
    restoreRejoinPlan({ running: false, needsDesktopSetup: true }),
    "desktop-setup",
  );
});

test("the address note is dismissed per session", () => {
  assert.notEqual(addressNoteKey("a"), addressNoteKey("b"));
  assert.equal(addressNoteKey("a"), addressNoteKey("a"));
});
