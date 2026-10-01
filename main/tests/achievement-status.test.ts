/**
 * Achievement status helpers: empty-list reasons, RA link state, server error
 * text, and the toast de-duplication window.
 *
 *   node --test main/tests/achievement-status.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  parseAchievementStatus,
  parseRaServerAccount,
  raLinkState,
  raSyncProblemText,
  serverErrorText,
  unavailableReasonText,
} from "../composables/achievements/status.ts";
import { acceptUnlock } from "../composables/achievements/toast.ts";

test("every known reason has text, no reason has none, unknown reasons still show", () => {
  for (const r of ["no_link", "steam_key_missing", "ra_credentials_missing", "not_scanned"]) {
    const text = unavailableReasonText(r);
    assert.ok(text && text.length > 0, r);
    assert.ok(!text!.includes("—"), `no em-dash in ${r}`);
  }
  assert.equal(unavailableReasonText(null), null);
  assert.equal(unavailableReasonText(undefined), null);
  assert.match(unavailableReasonText("new_reason")!, /new_reason/);
});

test("status parsing tolerates junk", () => {
  assert.deepEqual(parseAchievementStatus({ definitionCount: 0, reason: "no_link", raAccountMissing: true }), {
    definitionCount: 0,
    reason: "no_link",
    raAccountMissing: true,
  });
  assert.equal(parseAchievementStatus(null), null);
  assert.deepEqual(parseAchievementStatus({}), { definitionCount: 0, reason: null, raAccountMissing: false });
});

test("server error text prefers the server's own message", () => {
  assert.equal(
    serverErrorText(400, { statusCode: 400, statusMessage: "RetroAchievements sign in failed: bad password" }),
    "RetroAchievements sign in failed: bad password",
  );
  assert.equal(serverErrorText(403, { message: "Forbidden" }), "Forbidden");
  assert.match(serverErrorText(502, null), /502/);
  assert.match(serverErrorText(500, { statusMessage: "" }), /500/);
});

test("RA account parsing: linked, not linked, old server without the flag", () => {
  assert.deepEqual(
    parseRaServerAccount({
      accounts: [{ id: "1", provider: "RetroAchievements", externalId: "Player" }],
      raApiKeyRequired: false,
    }),
    { username: "Player", hasConnectToken: true, apiKeyRequired: false },
  );
  assert.deepEqual(parseRaServerAccount({ accounts: [] }), {
    username: null,
    hasConnectToken: true,
    apiKeyRequired: true,
  });
  assert.deepEqual(parseRaServerAccount(null), { username: null, hasConnectToken: true, apiKeyRequired: true });
  assert.equal(
    parseRaServerAccount({
      accounts: [{ provider: "RetroAchievements", externalId: "P", hasConnectToken: false }],
    }).hasConnectToken,
    false,
  );
});

test("link state only says linked when the server has the account", () => {
  assert.equal(raLinkState({ serverUsername: "P", localHasToken: false, localExpired: false }), "linked");
  // A local-only sign-in from an older build is NOT linked.
  assert.equal(raLinkState({ serverUsername: null, localHasToken: true, localExpired: false }), "device_only");
  assert.equal(raLinkState({ serverUsername: "P", localHasToken: true, localExpired: true }), "expired");
  assert.equal(raLinkState({ serverUsername: null, localHasToken: false, localExpired: false }), "none");
});

test("a device holding a different account than the server is a mismatch, not linked", () => {
  assert.equal(
    raLinkState({ serverUsername: "B", localUsername: "A", localHasToken: true, localExpired: false }),
    "mismatch",
  );
  // Case differences are the same RA account.
  assert.equal(
    raLinkState({ serverUsername: "Player", localUsername: "player", localHasToken: true, localExpired: false }),
    "linked",
  );
  // No local copy yet: the next launch fetches it, so this is linked.
  assert.equal(
    raLinkState({ serverUsername: "B", localUsername: "", localHasToken: false, localExpired: false }),
    "linked",
  );
});

test("toast de-dup drops a repeat inside the window and allows it after", () => {
  const seen = new Map<string, number>();
  assert.equal(acceptUnlock(seen, "a", 1000, 10_000), true);
  assert.equal(acceptUnlock(seen, "a", 5000, 10_000), false);
  assert.equal(acceptUnlock(seen, "b", 5000, 10_000), true);
  assert.equal(acceptUnlock(seen, "a", 12_000, 10_000), true);
});

test("linked without a server Connect token asks for one sign-in", () => {
  assert.equal(
    raLinkState({
      serverUsername: "P",
      serverHasConnectToken: false,
      localHasToken: false,
      localExpired: false,
    }),
    "needs_signin",
  );
  // A device token for a different account doesn't help either.
  assert.equal(
    raLinkState({
      serverUsername: "P",
      serverHasConnectToken: false,
      localUsername: "Q",
      localHasToken: true,
      localExpired: false,
    }),
    "needs_signin",
  );
});

test("a device that still holds a token for the same account is linked", () => {
  // sync_ra_credentials keeps that token, so RetroArch does sign in.
  assert.equal(
    raLinkState({
      serverUsername: "P",
      serverHasConnectToken: false,
      localUsername: "p",
      localHasToken: true,
      localExpired: false,
    }),
    "linked",
  );
});

test("a sync that couldn't reach the server says so", () => {
  assert.match(
    raSyncProblemText({ status: "unreachable", serverUsername: null, signsInAs: "P", error: "timed out" })!,
    /timed out/,
  );
  assert.ok(raSyncProblemText(null));
  assert.equal(
    raSyncProblemText({ status: "linked", serverUsername: "P", signsInAs: "P", error: null }),
    null,
  );
});
