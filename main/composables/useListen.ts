import { listen } from "@tauri-apps/api/event";
import type { EventCallback, EventName, UnlistenFn } from "@tauri-apps/api/event";
import { onMounted, onUnmounted } from "vue";

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
    const fn = await listen<T>(event, handler);
    if (cancelled) {
      // Component unmounted before listen() resolved — detach immediately.
      fn();
      return;
    }
    unlisten = fn;
  });

  onUnmounted(() => {
    cancelled = true;
    unlisten?.();
  });
}
