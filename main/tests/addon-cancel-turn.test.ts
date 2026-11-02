import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import vm from "node:vm";
import * as logic from "../composables/addon-play-logic.ts";

const require = createRequire(import.meta.url);
const ts = require("typescript");
const vue = require("vue");

function setup(state = "mine", running = false, response: unknown = { ok: true }) {
  let releases = 0;
  const invokes: string[] = [];
  const data = { addons: [{ name: "ncaa14", title: "League", available: true,
    contexts: [{ id: "1", title: "Test", state, slot: {}, lockBase: 4 }] }] };
  const api = { addons: {
    forGame: async () => data,
    release: async () => { releases++; return response; },
  } };
  const context: any = { ...vue, ...logic, exports: {}, console,
    useServerApi: () => api,
    invoke: async (command: string) => { invokes.push(command); return null; },
  };
  const source = readFileSync(new URL("../composables/addon-play.ts", import.meta.url), "utf8")
    .replace(/import\s+[\s\S]*?from\s+["'][^"']+["'];/g, "");
  vm.runInNewContext(ts.transpileModule(source, { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
  } }).outputText, context);
  const panel = context.exports.useAddonPlay(() => "game", () => running);
  return { panel, invokes, releases: () => releases };
}

test("cancel requires confirmation, then releases the turn without save writes or uploads", async () => {
  const { panel, invokes, releases } = setup();
  await panel.load();
  await panel.cancelTurn("ncaa14/1");
  assert.equal(releases(), 0);
  panel.askCancelTurn("ncaa14/1");
  await panel.cancelTurn("ncaa14/1");
  assert.equal(releases(), 1);
  assert.ok(invokes.every((command) => command === "addon_local_state"));
  assert.equal(panel.cancelling.value, null);
  assert.equal(panel.notice.value.kind, "ok");
});

for (const [state, running] of [["free", false], ["held", false], ["mine", true]] as const) {
  test(`cancel refuses state=${state}, running=${running}`, async () => {
    const { panel, releases } = setup(state, running);
    await panel.load();
    panel.askCancelTurn("ncaa14/1");
    await panel.cancelTurn("ncaa14/1");
    assert.equal(releases(), 0);
    assert.equal(panel.cancelling.value, null);
  });
}

test("server refusal stays visible and does not report cancellation success", async () => {
  const { panel } = setup("mine", false, { ok: false, code: "not_holder", message: "Turn changed" });
  await panel.load();
  panel.askCancelTurn("ncaa14/1");
  await panel.cancelTurn("ncaa14/1");
  assert.equal(panel.notice.value.kind, "error");
  assert.equal(panel.notice.value.message, "Turn changed");
  assert.equal(panel.busy.value, null);
});
