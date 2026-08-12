/**
 * Delays a loading indicator so short loads never flash one.
 *
 * Drop's bar is Steam: it paints instantly, and only work that genuinely takes
 * a moment gets a throbber. Most of our fetches resolve well under 200ms once
 * the object/proto caches are warm, so a skeleton bound straight to `loading`
 * appears and vanishes inside a frame or two, which reads as a flicker rather
 * than as feedback.
 *
 * Bind the skeleton to the returned ref and the real content to `!loading`, so
 * the sub-threshold window renders nothing at all:
 *
 *   const showSkeleton = useDeferredLoading(() => loading.value);
 *   <Skeleton v-if="showSkeleton" />
 *   <Content v-else-if="!loading" />
 *
 * Pairing the skeleton with `v-else` instead would render the content branch
 * during the threshold window, which is how an in-flight fetch ends up showing
 * an "empty library" screen.
 */

/** Matches the `<Suspense :timeout>` used for page-level suspension in app.vue. */
export const LOADING_INDICATOR_DELAY_MS = 180;

export function useDeferredLoading(
  source: () => boolean,
  delayMs: number = LOADING_INDICATOR_DELAY_MS,
) {
  const visible = ref(false);
  let timer: ReturnType<typeof setTimeout> | null = null;

  function clear() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  }

  watch(
    source,
    (loading) => {
      clear();
      if (!loading) {
        visible.value = false;
        return;
      }
      timer = setTimeout(() => {
        timer = null;
        visible.value = true;
      }, delayMs);
    },
    { immediate: true },
  );

  onScopeDispose(clear);

  return visible;
}
