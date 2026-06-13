// Re-applies the Nuxt vite-builder dev-server fix after every install.
//
// Why this exists: Nuxt 3.21.x's vite-builder regressed for our SPA
// (ssr: false) Tauri setup — `resolveServerEntry` throws
//     "No entry found in rollupOptions.input"
// at dev startup because our rollupOptions.input carries no server entry.
// The documented fix is to return null instead of throwing.
//
// We deliberately do NOT touch the `configureServer` `|| !nuxt.options.ssr`
// guard: `resolveServer` is what configures the vite-node IPC socket
// (sets NUXT_VITE_NODE_OPTIONS.socketPath), so it must keep running for SPA
// or every dev render dies with "Vite Node IPC socket path not configured".
//
// The patch is keyed to the CODE PATTERN, not a version, because our
// "nuxt": "~3.21.x" range keeps bumping the patch version (3.21.7 -> 3.21.8
// -> ...) and each bump installs a fresh, unpatched copy — which is exactly
// what kept wiping a hand-applied edit, and what would silently defeat a
// version-pinned `pnpm patch`. Running it from `postinstall` means a bump
// can't outrun it. Idempotent: once the throw is gone (here or upstream) it
// no-ops, so it's safe to leave wired in forever.
import { readdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const pnpmDir = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "node_modules",
  ".pnpm",
);

// Match ONLY resolveServerEntry's throw — the one immediately preceded by the
// `input.server` early-return. A sibling resolver has a byte-identical throw
// string that must stay intact, so we anchor on the preceding code.
const NEEDLE =
  /(input\.server\)\s*return input\.server;\s*\}\s*)throw new Error\("No entry found in rollupOptions\.input"\);/;

let scanned = 0;
let patched = 0;
let alreadyOk = 0;

let dirs = [];
try {
  dirs = readdirSync(pnpmDir);
} catch {
  console.log("[patch-vite-builder] no node_modules/.pnpm yet — skipping");
}

for (const d of dirs) {
  if (!d.startsWith("@nuxt+vite-builder@")) continue;
  const file = join(
    pnpmDir,
    d,
    "node_modules",
    "@nuxt",
    "vite-builder",
    "dist",
    "index.mjs",
  );
  if (!existsSync(file)) continue;
  scanned++;
  const src = readFileSync(file, "utf8");
  if (!NEEDLE.test(src)) {
    alreadyOk++;
    continue;
  }
  writeFileSync(file, src.replace(NEEDLE, "$1return null;"));
  patched++;
  console.log(`[patch-vite-builder] patched ${d}`);
}

console.log(
  `[patch-vite-builder] copies: scanned=${scanned} patched=${patched} already-ok=${alreadyOk}`,
);
