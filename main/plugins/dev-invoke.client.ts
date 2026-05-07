/**
 * Client-side plugin that wraps `window.__TAURI_INTERNALS__.invoke` so every
 * Tauri command call is traced when dev mode is on.
 *
 * Tauri 2 routes every `invoke()` from `@tauri-apps/api/core` through
 * `__TAURI_INTERNALS__.invoke(cmd, payload, options)` at the end of the
 * call chain. Intercepting there means we don't have to rewrite the ~48
 * call sites in the Nuxt app individually — every command, no matter
 * which file dispatched it, flows through this patched function.
 *
 * The wrapped function is transparent when dev mode is off (no overhead
 * beyond a single boolean check) and logs call + resolve / reject + duration
 * when it's on.
 *
 * Tauri 2.9 defines the `invoke` method as a non-writable / potentially
 * non-configurable property on a frozen `__TAURI_INTERNALS__` object,
 * so naive `internals.invoke = wrapped` throws
 * `Cannot assign to read only property 'invoke'`. We try three fallbacks
 * in order of compatibility, and if none work we log a single warning and
 * degrade gracefully — the rest of dev mode (gamepad, focus, events,
 * downloads, etc.) is untouched.
 */

import { devLog, devError, isDevEnabled } from "~/composables/dev-mode";

type TauriInternals = {
  invoke?: (cmd: string, payload?: unknown, options?: unknown) => Promise<unknown>;
};

let patched = false;

export default defineNuxtPlugin(() => {
  if (!import.meta.client) return;
  if (patched) return;

  const internals = (window as unknown as { __TAURI_INTERNALS__?: TauriInternals })
    .__TAURI_INTERNALS__;
  if (!internals || typeof internals.invoke !== "function") return;

  const origInvoke = internals.invoke.bind(internals);

  const wrappedInvoke: TauriInternals["invoke"] = (cmd, payload, options) => {
    // Cheap short-circuit — when dev mode is off this costs one boolean.
    if (!isDevEnabled("invoke")) {
      return origInvoke(cmd, payload, options);
    }

    const t0 = performance.now();
    const preview = previewArg(payload);
    devLog("invoke", `call  ${cmd}${preview ? "  " + preview : ""}`);
    // Tag launch-related commands with a second, filterable category so
    // users chasing "why won't this game boot" can isolate them from
    // the general invoke firehose.
    const launchRelated = isLaunchCommand(cmd);
    if (launchRelated) {
      devLog("launch", `call  ${cmd}${preview ? "  " + preview : ""}`);
    }

    const p = origInvoke(cmd, payload, options);
    p.then(
      (result) => {
        const ms = (performance.now() - t0).toFixed(1);
        const line = `${cmd}  ${ms}ms${previewResult(result) ? "  " + previewResult(result) : ""}`;
        devLog("invoke", `ok    ${line}`);
        if (launchRelated) devLog("launch", `ok    ${line}`);
      },
      (err) => {
        const ms = (performance.now() - t0).toFixed(1);
        const line = `${cmd}  ${ms}ms  ${stringifyError(err)}`;
        devError("invoke", `fail  ${line}`);
        if (launchRelated) devError("launch", `fail  ${line}`);
      },
    );
    return p;
  };

  // Fallback 1: Object.defineProperty with configurable:true
  // Works if the original property is writable:false but still configurable.
  try {
    Object.defineProperty(internals, "invoke", {
      value: wrappedInvoke,
      writable: true,
      configurable: true,
      enumerable: true,
    });
    if (internals.invoke === wrappedInvoke) {
      patched = true;
      devLog("invoke","[DEV:INVOKE] Tauri invoke interception installed (defineProperty)");
      return;
    }
  } catch {
    // Property is non-configurable — fall through.
  }

  // Fallback 2: Replace the whole __TAURI_INTERNALS__ object on window.
  // If the outer window property allows replacement, we proxy the original
  // internals and intercept only the invoke method.
  try {
    const replacement = new Proxy(internals, {
      get(target, prop, receiver) {
        if (prop === "invoke") return wrappedInvoke;
        return Reflect.get(target, prop, receiver);
      },
    });
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: replacement,
      writable: true,
      configurable: true,
      enumerable: true,
    });
    if (
      (window as unknown as { __TAURI_INTERNALS__?: TauriInternals })
        .__TAURI_INTERNALS__?.invoke === wrappedInvoke
    ) {
      patched = true;
      devLog("invoke","[DEV:INVOKE] Tauri invoke interception installed (window proxy)");
      return;
    }
  } catch {
    // Window property is also locked — nothing more we can do.
  }

  // Fallback 3: give up on auto-tracing invoke, but keep the rest of dev
  // mode working. Emit one warning so the user knows why invoke logs are
  // quiet even with the category on.
  console.warn(
    "[DEV:INVOKE] Could not patch __TAURI_INTERNALS__ — Tauri runtime has frozen it. Invoke-category tracing is disabled; all other dev-mode categories still work.",
  );
});

function previewArg(arg: unknown): string {
  if (arg === undefined || arg === null) return "";
  try {
    const s = JSON.stringify(arg);
    if (!s) return "";
    return s.length > 120 ? s.slice(0, 117) + "..." : s;
  } catch {
    return "[unserialisable]";
  }
}

function previewResult(result: unknown): string {
  if (result === undefined) return "";
  if (result === null) return "null";
  if (typeof result === "string") {
    return `"${result.length > 80 ? result.slice(0, 77) + "..." : result}"`;
  }
  if (typeof result === "number" || typeof result === "boolean") {
    return String(result);
  }
  try {
    const s = JSON.stringify(result);
    if (!s) return "";
    // Longer cap than args because results often carry useful shape info.
    return s.length > 160 ? s.slice(0, 157) + "..." : s;
  } catch {
    return "[unserialisable]";
  }
}

/**
 * Commands that relate to launching or terminating a game. Tagging them
 * under the `launch` category (in addition to `invoke`) lets users filter
 * the firehose down to just launch-related activity when chasing a
 * black-screen / immediate-exit bug.
 */
function isLaunchCommand(cmd: string): boolean {
  return (
    cmd === "launch_game" ||
    cmd === "run_game" ||
    cmd === "stop_game" ||
    cmd === "cancel_launch" ||
    cmd === "fetch_game_version_options" ||
    cmd === "download_game" ||
    cmd.startsWith("proton_") ||
    cmd.startsWith("umu_")
  );
}

function stringifyError(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}
