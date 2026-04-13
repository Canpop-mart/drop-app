/**
 * BPM Idle Detection
 *
 * Detects user inactivity to trigger screensaver/dimming features.
 * Uses requestAnimationFrame for smooth idle duration tracking.
 */

import { ref, onMounted, onUnmounted, onScopeDispose } from "vue";

// ── Singleton state ──────────────────────────────────────────────────────────

let idleTimeoutMs = 300000; // 5 minutes default
let lastActivityTime = Date.now();
let trackingRafId: number | null = null;

// ── Composable ───────────────────────────────────────────────────────────────

export function useBpmIdle(timeoutMs: number = 300000) {
  idleTimeoutMs = timeoutMs;
  lastActivityTime = Date.now();

  const isIdle = ref(false);
  const idleDuration = ref(0);

  function resetIdle() {
    lastActivityTime = Date.now();
    isIdle.value = false;
    idleDuration.value = 0;
  }

  function updateIdleDuration() {
    const now = Date.now();
    const elapsed = now - lastActivityTime;
    idleDuration.value = elapsed;

    if (elapsed >= idleTimeoutMs) {
      isIdle.value = true;
    } else if (isIdle.value) {
      isIdle.value = false;
    }

    trackingRafId = requestAnimationFrame(updateIdleDuration);
  }

  function onActivity() {
    resetIdle();
  }

  function startTracking() {
    if (typeof window === "undefined") return;

    // Register activity listeners
    window.addEventListener("mousemove", onActivity);
    window.addEventListener("keydown", onActivity);
    window.addEventListener("mousedown", onActivity);
    window.addEventListener("touchstart", onActivity);
    window.addEventListener("gamepadconnected", onActivity);

    // Start RAF-based duration tracker
    trackingRafId = requestAnimationFrame(updateIdleDuration);
  }

  function stopTracking() {
    if (typeof window === "undefined") return;

    // Remove activity listeners
    window.removeEventListener("mousemove", onActivity);
    window.removeEventListener("keydown", onActivity);
    window.removeEventListener("mousedown", onActivity);
    window.removeEventListener("touchstart", onActivity);
    window.removeEventListener("gamepadconnected", onActivity);

    // Stop RAF
    if (trackingRafId !== null) {
      cancelAnimationFrame(trackingRafId);
      trackingRafId = null;
    }
  }

  onMounted(startTracking);
  onUnmounted(stopTracking);

  onScopeDispose(() => {
    stopTracking();
  });

  return {
    isIdle,
    resetIdle,
    idleDuration,
  };
}
