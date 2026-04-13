/**
 * BPM Clock Widget
 *
 * Provides reactive time data for the Big Picture Mode clock display.
 * Updates every second and persists time format preference to localStorage.
 */

import { ref, readonly, computed, onMounted, onUnmounted } from "vue";

// ── Singleton state ──────────────────────────────────────────────────────────

function read12hPref(): boolean {
  if (typeof localStorage !== "undefined") {
    const stored = localStorage.getItem("bpm:clock12h");
    return stored === "true";
  }
  return false;
}

const use12h = ref(read12hPref());
let intervalId: NodeJS.Timeout | null = null;

// ── Composable ───────────────────────────────────────────────────────────────

export function useBpmClock() {
  const hours = ref(0);
  const minutes = ref(0);
  const time = computed(() => {
    const h = String(hours.value).padStart(2, "0");
    const m = String(minutes.value).padStart(2, "0");

    if (use12h.value) {
      const h12 = hours.value % 12 || 12;
      const suffix = hours.value >= 12 ? "PM" : "AM";
      return `${h12}:${m} ${suffix}`;
    }
    return `${h}:${m}`;
  });

  const date = computed(() => {
    const now = new Date();
    const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    const dayName = days[now.getDay()];
    const monthName = months[now.getMonth()];
    const day = now.getDate();
    return `${dayName}, ${monthName} ${day}`;
  });

  function updateTime() {
    const now = new Date();
    hours.value = now.getHours();
    minutes.value = now.getMinutes();
  }

  function toggle12h() {
    use12h.value = !use12h.value;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("bpm:clock12h", String(use12h.value));
    }
  }

  function startClock() {
    updateTime();
    intervalId = setInterval(updateTime, 1000);
  }

  function stopClock() {
    if (intervalId !== null) {
      clearInterval(intervalId);
      intervalId = null;
    }
  }

  onMounted(startClock);
  onUnmounted(stopClock);

  return {
    time,
    date,
    hours: readonly(hours),
    minutes: readonly(minutes),
    use12h: readonly(use12h),
    toggle12h,
  };
}
