#!/usr/bin/env node
// Checks that every repo path mentioned in the agent docs still exists, so maps can't silently rot.
// Usage (from the repo root): node tools/check-docs.mjs (drop-server) or node scripts/check-docs.mjs (drop-app)
// Scans AGENTS.md, CLAUDE.md and docs/**/*.md. A backticked token is treated as a path when it contains "/" and its
// first segment is a top-level entry of this repo (or it starts with ../drop-app / ../drop-server).
// Strips ":123" / ":10-20" line suffixes; checks only the part before any glob (*) or placeholder (<, {).
// Exit code 1 if anything is missing.
import { readdirSync, readFileSync, existsSync, statSync } from "node:fs";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const topLevel = new Set(readdirSync(root));
// Paths that are meant not to exist in the repo (created at runtime or per task).
const IGNORE = [/^tasks\/current\.md$/, /^prisma\/client(\/|$)/];

function mdFiles(dir) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) return mdFiles(p);
    return name.endsWith(".md") ? [p] : [];
  });
}

const files = ["AGENTS.md", "CLAUDE.md"].map((f) => join(root, f)).filter(existsSync).concat(mdFiles(join(root, "docs")));
let checked = 0;
const missing = [];

for (const file of files) {
  const lines = readFileSync(file, "utf8").split("\n");
  lines.forEach((line, i) => {
    for (const m of line.matchAll(/`([^`\s]+)`/g)) {
      let token = m[1].replace(/:\d+(-\d+)?$/, "").replace(/[),.;]+$/, "");
      if (!token.includes("/") || /^(https?:|\/|~|%|[A-Za-z]:)/.test(token)) continue;
      const cut = token.search(/[*<{]/);
      if (cut === 0) continue;
      if (cut > 0) token = token.slice(0, cut);
      token = token.replace(/\/+$/, "");
      const first = token.split("/")[0];
      const sibling = first === ".." && /^\.\.\/drop-(app|server)\//.test(token);
      if (!sibling && !topLevel.has(first)) continue;
      if (IGNORE.some((re) => re.test(token))) continue;
      checked++;
      if (!existsSync(resolve(root, token))) {
        missing.push(`${file.slice(root.length + 1)}:${i + 1}  ${token}`);
      }
    }
  });
}

console.log(`check-docs: ${files.length} docs, ${checked} paths checked, ${missing.length} missing`);
for (const m of missing) console.log("  MISSING " + m);
process.exit(missing.length ? 1 : 0);
