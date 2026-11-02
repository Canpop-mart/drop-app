import { test } from "node:test";
import assert from "node:assert/strict";
import { moveOptionGrid } from "../composables/bigpicture/option-grid-navigation.ts";
const columns = [["account", "executable"], ["updates"], ["shelf", "folder", "remove", "uninstall"]];
test("Left/Right enter adjacent columns at the closest available row", () => {
  assert.equal(moveOptionGrid(columns, "account", "right"), "updates");
  assert.equal(moveOptionGrid(columns, "updates", "right"), "shelf");
  assert.equal(moveOptionGrid(columns, "executable", "right"), "updates");
  assert.equal(moveOptionGrid(columns, "folder", "left"), "updates");
  assert.equal(moveOptionGrid(columns, "updates", "left"), "account");
});
test("Up/Down stay in the current column and stop at edges", () => {
  assert.equal(moveOptionGrid(columns, "account", "down"), "executable");
  assert.equal(moveOptionGrid(columns, "executable", "down"), "executable");
  assert.equal(moveOptionGrid(columns, "updates", "down"), "updates");
  assert.equal(moveOptionGrid(columns, "folder", "up"), "shelf");
  assert.equal(moveOptionGrid(columns, "account", "left"), "account");
  assert.equal(moveOptionGrid(columns, "uninstall", "right"), "uninstall");
});
test("Async inserts preserve identity; empty columns and removed items recover", () => {
  assert.equal(moveOptionGrid([["account", "new", "executable"], [], ["updates"]], "executable", "up"), "new");
  assert.equal(moveOptionGrid([["account"], [], ["updates"]], "account", "right"), "updates");
  assert.equal(moveOptionGrid(columns, "removed", "down"), "account");
  assert.equal(moveOptionGrid([], null, "right"), null);
});
