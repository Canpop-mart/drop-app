/**
 * Shared bits for the two achievement toasts.
 *
 * Desktop shows `AchievementToast` (mounted in app.vue); Big Picture shows the
 * themed `BpmAchievementToast` through `BpmAchievementToastHost` (mounted in
 * the bigpicture layout). Both listen to the same `achievement_unlocked`
 * event, so the host registers itself here while mounted and the desktop toast
 * stands down, which is what keeps the two from showing at once.
 */
import { ref } from "vue";

/** Payload of the `achievement_unlocked` Tauri event (launch.rs). */
export interface AchievementUnlockedPayload {
  id: string;
  title: string;
  description: string;
  iconUrl: string;
  /** Present from builds that send it; older payloads omit it. */
  gameId?: string;
  gameName?: string;
}

/** How many Big Picture toast hosts are mounted (0 or 1 in practice). */
export const bpmToastHosts = ref(0);

/**
 * Drops repeats of the same achievement id inside `windowMs`. Backends can
 * re-fire an unlock (retry, reconnect, multi-source sync). Returns true when
 * `id` should be shown. Mutates `seen`.
 */
export function acceptUnlock(
  seen: Map<string, number>,
  id: string,
  now: number,
  windowMs: number,
): boolean {
  for (const [k, ts] of seen) {
    if (now - ts > windowMs) seen.delete(k);
  }
  const last = seen.get(id);
  if (last !== undefined && now - last < windowMs) return false;
  seen.set(id, now);
  return true;
}
