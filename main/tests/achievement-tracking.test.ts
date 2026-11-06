import { test } from "node:test";
import assert from "node:assert/strict";
import {
  latestTrackingRows,
  overlayLocalUnlocks,
  progressPercent,
  type TrackingRecord,
} from "../composables/achievements/tracking.ts";

function row(
  account: string,
  current: number,
  observedAt: string,
): TrackingRecord {
  return {
    achievementId: null,
    title: "First",
    disposition: "pending",
    reason: null,
    event: {
      externalId: "first",
      provider: "Steam",
      providerGameId: "10",
      providerAccount: account,
      source: "steam-native",
      mode: "standard",
      kind: "progress",
      current,
      target: 10,
      observedAt,
    },
  };
}
test("progress follows source ordering, allows decreases and isolates accounts", () => {
  const rows = latestTrackingRows([
    row("alice", 8, "2026-11-05T12:00:00Z"),
    row("alice", 2, "2026-11-05T12:01:00Z"),
    row("bob", 9, "2026-11-05T12:00:00Z"),
  ]);
  assert.equal(rows.length, 2);
  assert.equal(rows[0].event.current, 2);
  assert.equal(rows[1].event.current, 9);
});
test("unknown and invalid progress stay unknown; display values are bounded", () => {
  assert.equal(progressPercent(null, 10), null);
  assert.equal(progressPercent(2, 0), null);
  assert.equal(progressPercent(Infinity, 10), null);
  assert.equal(progressPercent(20, 10), 100);
});
test("durable local unlocks update existing UI before server sync without promoting RA evidence", () => {
  const local = row("alice", 0, "2026-11-05T12:00:00Z");
  local.achievementId = "first";
  local.event.kind = "unlock";
  const server = [{ id: "first", unlocked: false }];
  assert.equal(overlayLocalUnlocks(server, [local])[0].unlocked, true);
  assert.equal(server[0].unlocked, false);
  local.event.provider = "RetroAchievements";
  assert.equal(overlayLocalUnlocks(server, [local])[0].unlocked, false);
  local.event.provider = "Steam";
  local.disposition = "before_reset";
  assert.equal(overlayLocalUnlocks(server, [local])[0].unlocked, false);
});
