/**
 * BPM Ambient Lighting
 *
 * Provides time-of-day ambient color shifting via CSS filters.
 * Creates a subtle atmosphere that changes throughout the day.
 */

import { ref, readonly, computed, onMounted, onUnmounted } from "vue";

// ── Theme color profiles ─────────────────────────────────────────────────────

interface AmbientProfile {
  hour: number; // Starting hour (24h format)
  label: string;
  filter: string;
}

const AMBIENT_PROFILES: AmbientProfile[] = [
  { hour: 0, label: "Night", filter: "hue-rotate(5deg) brightness(0.95) saturate(0.98)" },
  { hour: 6, label: "Dawn", filter: "hue-rotate(3deg) brightness(1.02) saturate(1.02)" },
  { hour: 10, label: "Day", filter: "none" },
  { hour: 18, label: "Dusk", filter: "hue-rotate(-3deg) brightness(0.98) saturate(1.05)" },
  { hour: 22, label: "Night", filter: "hue-rotate(5deg) brightness(0.95) saturate(0.98)" },
];

// ── Singleton state ──────────────────────────────────────────────────────────

function readEnabledPref(): boolean {
  if (typeof localStorage !== "undefined") {
    const stored = localStorage.getItem("bpm:ambient");
    return stored !== "false";
  }
  return true;
}

const enabled = ref(readEnabledPref());
let updateIntervalId: NodeJS.Timeout | null = null;

// ── Composable ───────────────────────────────────────────────────────────────

export function useBpmAmbient() {
  const currentHour = ref(new Date().getHours());

  function getFilterForHour(hour: number): string {
    // Find the profile that applies to this hour
    for (let i = AMBIENT_PROFILES.length - 1; i >= 0; i--) {
      if (hour >= AMBIENT_PROFILES[i].hour) {
        return AMBIENT_PROFILES[i].filter;
      }
    }
    return AMBIENT_PROFILES[0].filter;
  }

  const cssFilter = computed(() => {
    if (!enabled.value) return "none";
    return getFilterForHour(currentHour.value);
  });

  function setEnabled(val: boolean) {
    enabled.value = val;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("bpm:ambient", String(val));
    }
  }

  function updateHour() {
    currentHour.value = new Date().getHours();
  }

  function startUpdating() {
    updateHour();
    updateIntervalId = setInterval(updateHour, 60000); // Update every minute
  }

  function stopUpdating() {
    if (updateIntervalId !== null) {
      clearInterval(updateIntervalId);
      updateIntervalId = null;
    }
  }

  onMounted(startUpdating);
  onUnmounted(stopUpdating);

  return {
    enabled: readonly(enabled),
    setEnabled,
    cssFilter,
  };
}
