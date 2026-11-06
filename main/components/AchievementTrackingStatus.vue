<template>
  <section
    class="achievement-tracking mb-4 rounded-xl border p-4"
    :class="{ 'achievement-tracking-bpm': bpm }"
  >
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <h3 class="font-semibold">Achievement tracking</h3>
        <p class="mt-1 text-sm opacity-80">{{ summary }}</p>
      </div>
      <button
        :ref="(el: any) => register(el, refresh)"
        class="tracking-action"
        :disabled="busy"
        @click="refresh"
      >
        Retry
      </button>
    </div>
    <p v-if="error" class="mt-3 text-sm text-red-400" role="alert">
      {{ error }}
    </p>
    <p v-if="fetchError" class="mt-3 text-sm text-red-400" role="alert">
      {{ fetchError }}
    </p>
    <p v-if="notice" class="mt-3 text-sm text-amber-400" role="status">
      {{ notice }}
    </p>
    <template v-if="view">
      <p
        v-for="problem in view.journalErrors"
        :key="problem"
        class="mt-3 text-sm text-red-400"
        role="alert"
      >
        {{ problem }}
      </p>
      <p
        v-if="view.status?.error"
        class="mt-3 text-sm text-red-400"
        role="alert"
      >
        {{ view.status.error }}
      </p>
      <p
        v-if="view.status?.syncError"
        class="mt-2 text-sm text-amber-400"
        role="status"
      >
        {{ view.status.syncError }}
      </p>
      <p
        v-if="view.status?.popupError"
        class="mt-2 text-sm text-red-400"
        role="alert"
      >
        {{ view.status.popupError }}
      </p>
      <p v-if="view.pending" class="mt-2 text-sm">
        {{ view.pending }} events waiting to sync.
      </p>
      <p v-if="view.status?.sources.length" class="mt-2 text-xs opacity-70">
        {{ view.status.sources.join(", ") }}
      </p>
      <p
        v-for="row in unlocks"
        :key="row.event.providerAccount + row.event.externalId"
        class="mt-2 text-sm"
      >
        {{ row.title }}
        <span
          v-if="['pending', 'awaiting_definitions'].includes(row.disposition)"
          class="opacity-70"
          >(waiting to sync)</span
        >
      </p>
      <p
        v-for="state in view.remoteStates?.filter(
          (s) => s.mode !== 'standard' && s.unlocked,
        )"
        :key="state.achievementId + state.mode"
        class="mt-2 text-sm"
      >
        {{ state.title }}:
        {{ state.mode === "hardcore" ? "Hardcore" : "Softcore" }}
        <span v-if="!state.confirmed">(awaiting confirmation)</span>
      </p>
      <div
        v-for="row in rows"
        :key="
          row.event.provider +
          row.event.providerAccount +
          row.event.externalId +
          row.event.kind +
          row.event.mode
        "
        class="mt-3 text-sm"
      >
        <div class="flex justify-between gap-3">
          <span>{{ row.title }}</span
          ><span>{{ row.event.current }} / {{ row.event.target }}</span>
        </div>
        <progress
          class="mt-1 h-2 w-full"
          :value="progressPercent(row.event.current, row.event.target) ?? 0"
          max="100"
        />
      </div>
      <p
        v-for="row in problems"
        :key="row.event.externalId + row.event.kind"
        class="mt-2 text-sm text-amber-400"
      >
        {{ row.title }}: {{ trackingReason(row.reason ?? row.disposition) }}
      </p>
      <p v-if="view.status?.lastSyncAt" class="mt-3 text-xs opacity-60">
        Last sync: {{ new Date(view.status.lastSyncAt).toLocaleString() }}
      </p>
      <div v-if="view.popupSupported" class="mt-3 flex flex-wrap gap-3">
        <button
          :ref="(el: any) => register(el, togglePopups)"
          class="tracking-action"
          :disabled="busy"
          :aria-pressed="view.popupEnabled"
          @click="togglePopups"
        >
          Game popups: {{ view.popupEnabled ? "On" : "Off" }}
        </button>
        <button
          :ref="(el: any) => register(el, previewPopup)"
          class="tracking-action"
          :disabled="busy"
          @click="previewPopup"
        >
          Test popup
        </button>
        <p class="w-full text-xs opacity-70">
          Windows popup preview. Exclusive fullscreen support is not verified.
          Steam and emulator popups remain available.
        </p>
      </div>
    </template>
    <template v-if="bpm">
      <button
        v-if="!confirmReset"
        :ref="
          (el: any) =>
            register(el, () => {
              confirmReset = true;
            })
        "
        class="tracking-action mt-3"
        @click="confirmReset = true"
      >
        Reset tracking
      </button>
      <div v-else class="mt-3 space-y-3">
        <p class="text-sm">
          Reset this game's tracked achievements? Platform achievements remain
          on their service.
        </p>
        <div class="flex gap-3">
          <button
            :ref="(el: any) => register(el, reset)"
            class="tracking-action"
            :disabled="busy"
            @click="reset"
          >
            Confirm reset
          </button>
          <button
            :ref="
              (el: any) =>
                register(el, () => {
                  confirmReset = false;
                })
            "
            class="tracking-action"
            @click="confirmReset = false"
          >
            Cancel
          </button>
        </div>
      </div>
    </template>
  </section>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import {
  latestTrackingRows,
  progressPercent,
  trackingReason,
  type TrackingView,
} from "~/composables/achievements/tracking";

const props = defineProps<{ gameId: string; bpm?: boolean }>();
const emit = defineEmits<{ update: [view: TrackingView] }>();
const view = ref<TrackingView | null>(null);
const error = ref<string | null>(null);
const fetchError = ref<string | null>(null);
const notice = ref<string | null>(null);
const busy = ref(false);
const confirmReset = ref(false);
const registerFocusable = useBpFocusableGroup("content");
function register(el: HTMLElement | null, onSelect: () => void) {
  if (props.bpm) registerFocusable(el, { onSelect });
}
const rows = computed(() =>
  latestTrackingRows(view.value?.records ?? []).filter(
    (r) => r.event.kind === "progress",
  ),
);
const unlocks = computed(() =>
  latestTrackingRows(view.value?.records ?? [])
    .filter((r) => r.event.kind === "unlock" && r.disposition !== "invalid")
    .slice(-5),
);
const problems = computed(() =>
  latestTrackingRows(view.value?.records ?? []).filter((r) =>
    ["invalid", "awaiting_definitions"].includes(r.disposition),
  ),
);
const summary = computed(() => {
  if (!view.value)
    return fetchError.value
      ? "Tracking status unavailable."
      : "Loading tracking status.";
  if (!view.value.status?.active)
    return "Tracking starts when you launch this game.";
  if (!view.value.status.sources.length)
    return "Waiting for an achievement source.";
  if (view.value.status.sources.includes("retroachievements-api"))
    return "RetroAchievements syncs through its service. Local sources are tracked when available.";
  return "Local tracking active.";
});
let loading = false;
let disposed = false;
async function load() {
  if (loading) return;
  loading = true;
  const gameId = props.gameId;
  try {
    const result = await invoke<TrackingView>("fetch_achievement_tracking", {
      gameId,
    });
    if (!disposed && gameId === props.gameId) {
      view.value = result;
      fetchError.value = null;
      emit("update", result);
    }
  } catch (e) {
    if (!disposed && gameId === props.gameId) fetchError.value = String(e);
  } finally {
    loading = false;
  }
}
async function refresh() {
  if (busy.value) return;
  busy.value = true;
  try {
    await invoke("retry_achievement_tracking");
    await load();
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}
async function togglePopups() {
  if (busy.value || !view.value) return;
  busy.value = true;
  try {
    await invoke("update_settings", {
      newSettings: { achievementGamePopups: !view.value.popupEnabled },
    });
    await load();
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}
async function previewPopup() {
  if (busy.value) return;
  busy.value = true;
  error.value = null;
  try {
    await invoke("preview_achievement_popup");
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}
async function reset() {
  if (busy.value) return;
  busy.value = true;
  error.value = null;
  notice.value = null;
  try {
    const response = await fetch(
      serverUrl(
        `api/v1/user/achievements/reset?gameId=${encodeURIComponent(props.gameId)}`,
      ),
      { method: "DELETE" },
    );
    if (!response.ok) throw new Error(`Reset failed (${response.status}).`);
    const local = await invoke<{ status: string; failures: string[] }>(
      "clear_local_achievements",
      { gameId: props.gameId },
    );
    if (local.status === "running")
      notice.value =
        "Tracking reset. Close the game before clearing its local achievement files.";
    else if (local.failures?.length) notice.value = local.failures.join("; ");
    confirmReset.value = false;
    await load();
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}
let timer: ReturnType<typeof setInterval> | undefined;
onMounted(() => {
  void load();
  timer = setInterval(() => {
    void load();
  }, 2000);
});
onUnmounted(() => {
  disposed = true;
  if (timer) clearInterval(timer);
});
</script>

<style scoped>
.achievement-tracking {
  background: #18181b;
  border-color: #3f3f46;
  color: #f4f4f5;
}
.achievement-tracking-bpm {
  background: var(--bpm-surface);
  border-color: var(--bpm-border);
  color: var(--bpm-text);
}
.tracking-action {
  padding: 0.5rem 1rem;
  border-radius: 0.5rem;
  border: 1px solid currentColor;
  font-size: 0.875rem;
}
.tracking-action:disabled {
  opacity: 0.5;
}
progress {
  accent-color: #a855f7;
}
</style>
