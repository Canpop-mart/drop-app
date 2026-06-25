/**
 * Lightweight nav-history tracker — gives the global header's back /
 * forward arrows a way to decide whether each direction is reachable.
 *
 * vue-router 4 stores its own metadata on `window.history.state` under
 * the `position` key (an incrementing index into the session history)
 * and the browser keeps `window.history.length` in sync.  We read both
 * after every navigation and derive can-go-back / can-go-forward from
 * them; that's enough for the header chrome without us having to
 * maintain a parallel history stack.
 *
 * Singleton — the back/forward buttons live in the global header, but
 * we want the same reactive state available anywhere (e.g. a future
 * BPM nav rail) without duplicate listeners.  Module-level refs +
 * idempotent init.
 */

import { computed, ref } from "vue";
import { useRouter } from "vue-router";

const position = ref(0);
const length = ref(1);
let initialized = false;

function readState() {
  // vue-router stamps { position, ... } onto history.state on every
  // navigation; if we read between routes it can be null on first
  // mount — fall back to the previously known position.
  const s = (window.history.state ?? null) as { position?: number } | null;
  if (s && typeof s.position === "number") {
    position.value = s.position;
  }
  length.value = window.history.length;
}

export function useNavHistory() {
  const router = useRouter();

  if (!initialized && typeof window !== "undefined") {
    initialized = true;
    readState();
    // After every successful navigation (push, replace, back, forward)
    // the new history state has been committed — re-read.
    router.afterEach(() => {
      // Defer to next microtask so window.history reflects the new
      // entry before we read it.
      Promise.resolve().then(readState);
    });
    // popstate covers browser-back / browser-forward (and Tauri's
    // mouse-side-button if the user has it wired) that aren't
    // initiated via router.push().
    window.addEventListener("popstate", readState);
  }

  // canGoBack: there's at least one entry before us.
  // canGoForward: there's at least one entry after us.  We can't
  // observe the *forward* stack length directly (history.length covers
  // total session length, not just future entries), but `position`
  // increases as we push, and a back-navigation drops position without
  // changing length — so `length - 1 > position` is a reliable check.
  const canGoBack = computed(() => position.value > 0);
  const canGoForward = computed(() => position.value < length.value - 1);

  function back() {
    if (!canGoBack.value) return;
    router.back();
  }

  function forward() {
    if (!canGoForward.value) return;
    router.forward();
  }

  return { canGoBack, canGoForward, back, forward };
}
