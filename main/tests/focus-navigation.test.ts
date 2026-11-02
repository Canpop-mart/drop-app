import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import vm from "node:vm";

const require = createRequire(import.meta.url);
const ts = require("typescript");
const vue = require("vue");

function setup() {
  const nodes: any[] = [];
  class Element {
    isConnected = true;
    tagName = "BUTTON";
    id = "";
    parentElement = null;
    classList = { add() {}, remove() {}, contains() { return false; } };
    setAttribute() {}
    getBoundingClientRect() {
      return { left: 100, top: 100, right: 150, bottom: 150, width: 50, height: 50 };
    }
    compareDocumentPosition(other: any) { return nodes.indexOf(this) < nodes.indexOf(other) ? 4 : 2; }
  }
  const gp = { onButton: () => () => {}, onButtonRelease: () => () => {},
    onAnyButton: () => () => {}, axisValue: () => 0, vibrate() {} };
  const context: any = {
    ...vue, exports: {}, HTMLElement: Element, Node: { DOCUMENT_POSITION_FOLLOWING: 4 },
    document: { querySelectorAll: () => [], activeElement: null },
    window: { innerWidth: 1920, innerHeight: 1080, getComputedStyle: () => ({ overflowY: "visible" }) },
    GamepadButton: {}, useGamepad: () => gp, setGlobalInputLock() {},
    useBpAudio: () => ({ play() {} }), tryGamepadAudioUnlock() {},
    useDeckMode: () => ({ isGamescope: vue.ref(false) }), devLog() {},
    setTimeout, clearTimeout, setInterval, clearInterval, console,
  };
  const source = readFileSync(new URL("../composables/focus-navigation.ts", import.meta.url), "utf8")
    .replace(/import\s+[\s\S]*?from\s+["'][^"']+["'];/g, "");
  vm.runInNewContext(ts.transpileModule(source, { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
  } }).outputText, context);
  const focus = context.exports.useFocusNavigation();
  // Vue treats browser DOM nodes as raw values. Match that in this DOM stub.
  function node() { const el = vue.markRaw(new Element()); nodes.push(el); return el; }
  return { focus, node };
}

test("removing the focused page control releases focus and selects its surviving neighbour", async () => {
  const { focus, node } = setup();
  const first = node();
  const second = node();
  const unregister = focus.registerElement(first, "content");
  focus.registerElement(second, "content");
  focus.focusGroup("content");
  unregister();
  assert.equal(focus.currentFocused.value, null);
  await vue.nextTick();
  assert.equal(focus.currentFocused.value?.el, second);
});

test("Right enters replacement page content after the previous page unmounts", async () => {
  const { focus, node } = setup();
  const rail = node();
  focus.registerElement(rail, "nav");
  const old = node();
  const unregister = focus.registerElement(old, "content");
  focus.focusGroup("content");
  old.isConnected = false;
  unregister();
  const arriving = node();
  focus.registerElement(arriving, "content");
  await vue.nextTick();
  focus.focusGroup("nav");
  focus.navigate("right");
  assert.equal(focus.currentFocused.value?.el, arriving);
});

test("direction input recovers when Vue detaches a ref before its unregister callback", () => {
  const { focus, node } = setup();
  const old = node();
  focus.registerElement(old, "content");
  focus.focusGroup("content");
  old.isConnected = false;
  const arriving = node();
  focus.registerElement(arriving, "content");
  focus.navigate("down");
  assert.equal(focus.currentFocused.value?.el, arriving);
});
