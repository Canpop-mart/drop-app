/**
 * BPM Idle Detection
 *
 * Detects user inactivity to trigger screensaver/dimming features. A 1 Hz
 * tick is enough here — the previous 60 Hz RAF loop kept the iGPU busy and
 * caused every component watching `idleDuration` to re-render every frame.
 */

import { ref, onMounted, onUnmounted, onScopeDispose } from "vue";

let idleTimeoutMs = 300000;
let lastActivityTime = Date.now();
let trackingIntervalId: ReturnType<typeof setInterval> | null = null;

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

  function tick() {
    const elapsed = Date.now() - lastActivityTime;
    idleDuration.value = elapsed;
    const nowIdle = elapsed >= idleTimeoutMs;
    if (nowIdle !== isIdle.value) isIdle.value = nowIdle;
  }

  function onActivity() {
    resetIdle();
  }

  function startTracking() {
    if (typeof window === "undefined") return;
    window.addEventListener("mousemove", onActivity, { passive: true });
    window.addEventListener("keydown", onActivity, { passive: true });
    window.addEventListener("mousedown", onActivity, { passive: true });
    window.addEventListener("touchstart", onActivity, { passive: true });
    window.addEventListener("gamepadconnected", onActivity, { passive: true });
    trackingIntervalId = setInterval(tick, 1000);
  }

  function stopTracking() {
    if (typeof window === "undefined") return;
    window.removeEventListener("mousemove", onActivity);
    window.removeEventListener("keydown", onActivity);
    window.removeEventListener("mousedown", onActivity);
    window.removeEventListener("touchstart", onActivity);
    window.removeEventListener("gamepadconnected", onActivity);
    if (trackingIntervalId !== null) {
      clearInterval(trackingIntervalId);
      trackingIntervalId = null;
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
