/**
 * Stray bold markers left by the Steam importer are dropped before rendering.
 *
 *   node --test main/tests/markdown-cleanup.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { dropOrphanEmphasis } from "../composables/markdown-cleanup.ts";

test("drops lines holding only ** or __", () => {
  const md = "# V 1.0 Out Now!\n\n![](img)\n\n**\n\n# DISCORD\n\n  __  \nText";
  assert.equal(
    dropOrphanEmphasis(md),
    "# V 1.0 Out Now!\n\n![](img)\n\n\n# DISCORD\n\nText",
  );
});

test("keeps real bold, rules and other text", () => {
  const md = "**bold** text\n***\n---\n* list item\nsome ** inline";
  assert.equal(dropOrphanEmphasis(md), md);
});

test("handles Windows line endings", () => {
  assert.equal(dropOrphanEmphasis("a\r\n**\r\nb"), "a\nb");
});

test("never touches code blocks", () => {
  const fenced = "```\n**\n```\n~~~~\n__\n```\n__\n~~~~";
  assert.equal(dropOrphanEmphasis(fenced), fenced);
  const indented = "text\n\n    **\n";
  assert.equal(dropOrphanEmphasis(indented), indented);
});

test("bold markers after a closed fence are still dropped", () => {
  assert.equal(dropOrphanEmphasis("```\ncode\n```\n**\nafter"), "```\ncode\n```\nafter");
});
