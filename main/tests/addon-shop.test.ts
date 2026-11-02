/**
 * The Store's Addons tab: the server's list is checked, a row's state follows
 * the server, and a failed load is never mistaken for "no addons".
 *
 *   node --test main/tests/addon-shop.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  parseShopList,
  rowView,
  withAdded,
  type ShopAddon,
} from "../composables/addon-shop-logic.ts";

const league: ShopAddon = {
  name: "ncaa14",
  title: "NCAA 14 League",
  summary: "Pass the dynasty save",
  description: "",
  site: "https://ncaa14.example/",
  added: false,
};

test("a list that is not an array is an error, not an empty shop", () => {
  assert.throws(() => parseShopList(undefined));
  assert.throws(() => parseShopList({ addons: [] }));
  assert.throws(() => parseShopList("[]"));
});

test("an empty array is a genuinely empty shop", () => {
  assert.deepEqual(parseShopList([]), []);
});

test("malformed and duplicate entries are dropped, the rest kept", () => {
  const list = parseShopList([
    null,
    "ncaa14",
    { name: "Bad Name", title: "x" },
    { name: "ncaa14", title: "NCAA 14 League", added: true },
    { name: "ncaa14", title: "Duplicate" },
    { name: "other", added: "yes" },
  ]);
  assert.deepEqual(
    list.map((a) => [a.name, a.title, a.added]),
    [
      ["ncaa14", "NCAA 14 League", true],
      // missing title falls back to the name; only a real `true` is added
      ["other", "other", false],
    ],
  );
});

test("only http(s) links are kept as the website", () => {
  const [a, b, c] = parseShopList([
    { name: "a", site: "https://a.example/" },
    { name: "b", site: "javascript:alert(1)" },
    { name: "c", site: 42 },
  ]);
  assert.equal(a.site, "https://a.example/");
  assert.equal(b.site, null);
  assert.equal(c.site, null);
});

test("withAdded changes one addon and leaves the original list alone", () => {
  const list = [league, { ...league, name: "other" }];
  const next = withAdded(list, "ncaa14", true);
  assert.equal(next[0].added, true);
  assert.equal(next[1].added, false);
  assert.equal(list[0].added, false);
});

test("a row offers Add until added, then Remove", () => {
  assert.equal(rowView(league, null, null).primary, "add");
  assert.equal(rowView({ ...league, added: true }, null, null).primary, "remove");
});

test("the remove confirm shows only on the added row it was opened for", () => {
  const added = { ...league, added: true };
  assert.equal(rowView(added, "ncaa14", null).confirming, true);
  assert.equal(rowView(added, "other", null).confirming, false);
  // An addon that is not added has nothing to confirm.
  assert.equal(rowView(league, "ncaa14", null).confirming, false);
});

test("while any request runs, every row waits", () => {
  const v = rowView(league, null, "other");
  assert.equal(v.busy, false);
  assert.equal(v.locked, true);
  assert.equal(rowView(league, null, "ncaa14").busy, true);
});
