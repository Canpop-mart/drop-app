/**
 * Profile themes: every stored value must render as itself on every surface,
 * and the server's copy of this file must match this one.
 *
 *   node --test main/tests/profile-themes.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  DEFAULT_PROFILE_THEME,
  PROFILE_THEME_PRESETS,
  normalizeProfileTheme,
  resolveAccentHex,
  resolveThemeGradient,
} from "../composables/profile-themes.ts";

const HEX = /^#[0-9a-f]{6}$/;

// Every value any surface has ever offered: the old desktop list, and the
// list Big Picture and the web pages shared.
const LEGACY_IDS = [
  "default",
  "ocean",
  "sunset",
  "forest",
  "purple",
  "rose",
  "ember",
  "arctic",
  "midnight",
];

test("every value any surface ever stored is still a preset", () => {
  const ids = PROFILE_THEME_PRESETS.map((p) => p.id);
  for (const id of LEGACY_IDS) assert.ok(ids.includes(id), id);
});

test("preset ids are unique and colours are lower-case #rrggbb", () => {
  const ids = PROFILE_THEME_PRESETS.map((p) => p.id);
  assert.equal(new Set(ids).size, ids.length);
  for (const p of PROFILE_THEME_PRESETS) {
    assert.match(p.accent, HEX, `${p.id} accent`);
    assert.match(p.from, HEX, `${p.id} from`);
    assert.match(p.to, HEX, `${p.id} to`);
    assert.ok(p.label.length > 0);
  }
  assert.ok(ids.includes(DEFAULT_PROFILE_THEME));
});

test("presets resolve to themselves, not the default", () => {
  const def = PROFILE_THEME_PRESETS.find((p) => p.id === "default")!;
  for (const p of PROFILE_THEME_PRESETS) {
    assert.equal(resolveAccentHex(p.id), p.accent);
    assert.deepEqual(resolveThemeGradient(p.id), { from: p.from, to: p.to });
    if (p.id !== "default") {
      assert.notDeepEqual(resolveThemeGradient(p.id), {
        from: def.from,
        to: def.to,
      });
    }
  }
});

test("a custom colour renders as that colour everywhere", () => {
  assert.equal(resolveAccentHex("#FF8800"), "#ff8800");
  const g = resolveThemeGradient("#ff8800");
  assert.match(g.from, HEX);
  assert.match(g.to, HEX);
  // Not the default preset's gradient.
  assert.notEqual(g.from, "#1e3a5f");
  // Same hue family: red dominates green dominates blue, as in #ff8800.
  const rgb = (h: string) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
  const [r, gr, b] = rgb(g.from);
  assert.ok(r! > gr! && gr! > b!, g.from);
});

test("grey and black custom colours produce valid gradients", () => {
  for (const c of ["#000000", "#ffffff", "#808080"]) {
    const g = resolveThemeGradient(c);
    assert.match(g.from, HEX, c);
    assert.match(g.to, HEX, c);
  }
});

test("unknown or empty values fall back to the default", () => {
  const def = PROFILE_THEME_PRESETS.find((p) => p.id === "default")!;
  for (const v of [undefined, null, "", "teal", "#fff", "#12345g", "Ocean"]) {
    assert.equal(resolveAccentHex(v), def.accent, String(v));
    assert.deepEqual(resolveThemeGradient(v), { from: def.from, to: def.to });
  }
});

test("normalizeProfileTheme accepts presets and #rrggbb only", () => {
  assert.equal(normalizeProfileTheme("ember"), "ember");
  assert.equal(normalizeProfileTheme("#AbCdEf"), "#abcdef");
  for (const v of [undefined, null, "", "Ocean", "#abc", "abcdef", "#abcdef0", "red"]) {
    assert.equal(normalizeProfileTheme(v), null, String(v));
  }
});

// The server keeps an identical copy. When both repos sit side by side (the
// owner's machine), make sure they have not drifted.
test("drop-server's copy is identical", (t) => {
  const here = dirname(fileURLToPath(import.meta.url));
  const mine = join(here, "../composables/profile-themes.ts");
  const candidates = [
    join(here, "../../../drop-server/server/internal/utils/profile-themes.ts"),
    process.env.DROP_SERVER_DIR
      ? join(process.env.DROP_SERVER_DIR, "server/internal/utils/profile-themes.ts")
      : "",
  ].filter((p) => p && existsSync(p));
  if (candidates.length === 0) {
    t.skip("drop-server checkout not found next to drop-app");
    return;
  }
  assert.equal(
    readFileSync(candidates[0]!, "utf8"),
    readFileSync(mine, "utf8"),
  );
});
