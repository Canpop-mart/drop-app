import { listen } from "@tauri-apps/api/event";
import type { EventCallback, EventName, UnlistenFn } from "@tauri-apps/api/event";
import { onMounted, onUnmounted } from "vue";
import { devLog, isDevEnabled } from "./dev-mode";

/**
 * Subscribe to a Tauri backend event with automatic cleanup on unmount.
 *
 * The raw `listen()` API returns a promise for the unlisten function, which
 * means naive call sites often drop it on the floor or invoke it against an
 * unresolved promise. This wrapper handles both the subscribe and the cleanup
 * inside Vue's lifecycle hooks, so every listener is guaranteed to be torn
 * down when the owning component unmounts.
 *
 * Usage:
 * ```ts
 * useListen("update_state", (event) => {
 *   // handle event.payload
 * });
 * ```
 */
export function useListen<T>(event: EventName, handler: EventCallback<T>): void {
  let unlisten: UnlistenFn | undefined;
  let cancelled = false;

  onMounted(async () => {
    devLog("event", `subscribe "${event}"`);
    // Wrap the handler so we can observe every payload when dev mode is on,
    // but keep the no-op fast path for production. `isDevEnabled` is a
    // cheap boolean check that short-circuits when dev mode is off.
    const wrapped: EventCallback<T> = (evt) => {
      if (isDevEnabled("event")) {
        const preview = summarisePayload(evt.payload);
        devLog("event", `recv  "${event}"  ${preview}`);
      }
      handler(evt);
    };
    const fn = await listen<T>(event, wrapped);
    if (cancelled) {
      // Component unmounted before listen() resolved — detach immediately.
      devLog("event", `subscribe "${event}" — cancelled before resolved`);
      fn();
      return;
    }
    unlisten = fn;
  });

  onUnmounted(() => {
    cancelled = true;
    devLog("event", `unsubscribe "${event}"`);
    unlisten?.();
  });
}

/** Best-effort short preview for a payload — avoids flooding the overlay. */
function summarisePayload(payload: unknown): string {
  if (payload === null || payload === undefined) return String(payload);
  if (typeof payload === "string") return payload.slice(0, 120);
  if (typeof payload === "number" || typeof payload === "boolean") {
    return String(payload);
  }
  try {
    const s = JSON.stringify(payload);
    return s.length > 120 ? s.slice(0, 117) + "..." : s;
  } catch {
    return "[unserialisable]";
  }
}
