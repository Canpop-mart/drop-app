/**
 * Which server notifications count as game request decisions, and which of
 * them get a toast.
 *
 * Runs on the Node test runner with no extra dependency:
 *   node --test main/tests/request-notifications.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  isRequestDecisionsReadMessage,
  LEGACY_TOASTED_STORAGE_KEY,
  mergeLegacyToasted,
  messagePollDelay,
  nextPollDelay,
  pickNewToasts,
  pruneToastedIds,
  REQUEST_DECISIONS_READ_MESSAGE,
  requestDecisionKind,
  toastedStorageKey,
  unreadRequestDecisions,
  type ServerNotification,
} from "../composables/request-notifications-logic.ts";

function n(
  id: string,
  nonce: string | null,
  created: string,
  read = false,
): ServerNotification {
  return { id, nonce, title: id, description: "", read, created };
}

test("recognises the three request decision nonces and nothing else", () => {
  assert.equal(requestDecisionKind(n("a", "request-approved-r1", "")), "approved");
  assert.equal(requestDecisionKind(n("a", "request-denied-r1", "")), "denied");
  assert.equal(
    requestDecisionKind(n("a", "request-fulfilled-r1", "")),
    "fulfilled",
  );
  assert.equal(requestDecisionKind(n("a", "request-vote-r1", "")), null);
  assert.equal(requestDecisionKind(n("a", "task-finished-x", "")), null);
  assert.equal(requestDecisionKind(n("a", null, "")), null);
});

test("unread decisions skip read ones and other notifications, oldest first", () => {
  const list = [
    n("new", "request-denied-2", "2026-10-01T12:00:00Z"),
    n("old", "request-approved-1", "2026-10-01T10:00:00Z"),
    n("read", "request-approved-3", "2026-10-01T11:00:00Z", true),
    n("other", "update-available", "2026-10-01T09:00:00Z"),
  ];
  assert.deepEqual(
    unreadRequestDecisions(list).map((x) => x.id),
    ["old", "new"],
  );
});

test("toasts only what was not toasted before, capped to the newest few", () => {
  const list = [1, 2, 3, 4, 5].map((i) =>
    n(`n${i}`, `request-approved-${i}`, `2026-10-01T0${i}:00:00Z`),
  );
  assert.deepEqual(
    pickNewToasts(list, new Set(["n5"]), 3).map((x) => x.id),
    ["n2", "n3", "n4"],
  );
  assert.deepEqual(
    pickNewToasts(list, new Set(list.map((x) => x.id))),
    [],
    "nothing new, nothing toasted",
  );
});

test("remembered toast ids forget notifications the server no longer has", () => {
  assert.deepEqual(
    pruneToastedIds(["a", "b", "c"], new Set(["b", "c"])),
    ["b", "c"],
  );
  assert.deepEqual(
    pruneToastedIds(["a", "b", "c"], new Set(["a", "b", "c"]), 2),
    ["b", "c"],
  );
});

test("poll delay backs off on failure and is capped, never giving up", () => {
  assert.equal(nextPollDelay(0, 60, 1000), 60);
  assert.equal(nextPollDelay(1, 60, 1000), 120);
  assert.equal(nextPollDelay(3, 60, 1000), 480);
  assert.equal(nextPollDelay(4, 60, 1000), 960);
  assert.equal(nextPollDelay(5, 60, 1000), 1000);
  assert.equal(nextPollDelay(10_000, 60, 1000), 1000);
  assert.ok(Number.isFinite(nextPollDelay(10_000)));
});

test("toasted ids are kept per user", () => {
  assert.notEqual(toastedStorageKey("alice"), toastedStorageKey("bob"));
  assert.equal(toastedStorageKey("alice"), toastedStorageKey("alice"));
  assert.notEqual(toastedStorageKey("alice"), LEGACY_TOASTED_STORAGE_KEY);
});

test("legacy toasted ids merge into the user's list without duplicates", () => {
  assert.deepEqual(mergeLegacyToasted(["a", "b"], ["b", "c"]), ["a", "b", "c"]);
  assert.deepEqual(mergeLegacyToasted([], ["x"]), ["x"]);
  assert.deepEqual(mergeLegacyToasted(["x"], []), ["x"]);
});

test("message-triggered polls are spaced out", () => {
  assert.equal(messagePollDelay(1000, null, 5000), 0);
  assert.equal(messagePollDelay(7000, 1000, 5000), 0);
  assert.equal(messagePollDelay(2000, 1000, 5000), 4000);
  assert.equal(messagePollDelay(6000, 1000, 5000), 0);
});

test("recognises only the request-decisions-read message", () => {
  assert.equal(
    isRequestDecisionsReadMessage({ type: REQUEST_DECISIONS_READ_MESSAGE }),
    true,
  );
  assert.equal(isRequestDecisionsReadMessage({ type: "bp-bridge-ready" }), false);
  assert.equal(isRequestDecisionsReadMessage(null), false);
  assert.equal(isRequestDecisionsReadMessage("drop:request-decisions-read"), false);
});
