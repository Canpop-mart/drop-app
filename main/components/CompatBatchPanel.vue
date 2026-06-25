<template>
  <div
    class="rounded-xl bg-zinc-800/50 backdrop-blur-sm overflow-hidden border border-zinc-700/40 max-w-md mx-auto mt-8"
  >
    <div class="px-5 py-4 border-b border-zinc-700/40">
      <h2 class="text-base font-semibold text-zinc-100">
        Batch compatibility test
      </h2>
      <p class="mt-1 text-xs text-zinc-500">
        Walks every installed game in your library that hasn't been tested
        recently, runs a 45-second compat probe on each, and posts the
        result. Stop any time — already-tested games keep their result.
      </p>
    </div>

    <div class="px-5 py-4 space-y-3">
      <div v-if="!running && !done" class="flex items-center justify-center">
        <button
          class="inline-flex items-center gap-x-2 rounded-md bg-emerald-600 px-4 py-2 text-sm font-semibold text-white shadow-sm hover:bg-emerald-500 active:scale-95 transition-transform duration-200"
          @click="start"
        >
          <BeakerIcon class="size-4" />
          Start batch test
        </button>
      </div>

      <div v-if="running" class="space-y-3">
        <div
          class="flex items-center justify-between text-xs text-zinc-400 font-mono"
        >
          <span>
            Tested {{ count.tested }} · Working {{ count.working }} · Broken
            {{ count.broken }} · Needs review {{ count.review }}
          </span>
          <button
            class="text-rose-400 hover:text-rose-300 font-semibold uppercase tracking-wide"
            @click="requestStop"
          >
            Stop
          </button>
        </div>
        <div v-if="currentName" class="text-sm text-zinc-100">
          Now testing:
          <span class="font-semibold">{{ currentName }}</span>
        </div>
        <div v-else class="text-xs italic text-zinc-500">
          {{
            stopRequested
              ? "Stopping after current game…"
              : "Waiting for next item…"
          }}
        </div>
      </div>

      <div v-if="done && !running" class="space-y-2">
        <div class="text-sm text-emerald-400 font-semibold">
          Batch complete — tested {{ count.tested }} game{{
            count.tested === 1 ? "" : "s"
          }}.
        </div>
        <div class="text-xs text-zinc-400">
          {{ count.working }} working · {{ count.broken }} broken ·
          {{ count.review }} pending render-confirmation
        </div>
        <button
          class="text-xs text-zinc-500 hover:text-zinc-300 underline"
          @click="reset"
        >
          Run another batch
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { BeakerIcon } from "@heroicons/vue/24/solid";

type WorkItem = {
  gameId: string;
  name: string;
  metadataId: string;
  lastTestedAt: string | null;
  platform: string | null;
};

type CompatTestOutcome = {
  status: string;
  signature: string | null;
  elapsedSecs: number;
  posted: boolean;
  protonVersion?: string | null;
};

const running = ref(false);
const done = ref(false);
const stopRequested = ref(false);
const currentName = ref<string | null>(null);

const count = reactive({
  tested: 0,
  working: 0,
  broken: 0,
  review: 0,
});

function reset() {
  running.value = false;
  done.value = false;
  stopRequested.value = false;
  currentName.value = null;
  count.tested = 0;
  count.working = 0;
  count.broken = 0;
  count.review = 0;
}

function requestStop() {
  stopRequested.value = true;
}

/**
 * Outer batch loop: ask the server for the next work item, run a test,
 * tally the outcome, repeat until nothing's left or the user stops.
 *
 * Each iteration delegates to the same `start_compat_test` Tauri command
 * the per-game button uses, so behaviour stays consistent. We deliberately
 * skip the render-confirm dialog inside the loop (it'd block the user on
 * each game) — `AliveNoRender` results are tallied as "needs review" and
 * surface on the per-game panels for the user to confirm later.
 */
async function start() {
  reset();
  running.value = true;

  while (!stopRequested.value) {
    let work: WorkItem | null = null;
    try {
      work = await invoke<WorkItem | null>("fetch_next_compat_work");
    } catch (e) {
      console.error("[compat-batch] fetch_next_compat_work failed:", e);
      break;
    }

    if (!work) break;

    currentName.value = work.name;

    let outcome: CompatTestOutcome | null = null;
    try {
      outcome = await invoke<CompatTestOutcome>("start_compat_test", {
        gameId: work.gameId,
        versionIndex: 0,
        options: {
          timeoutSecs: 45,
          leaveRunning: false,
          notes: "batch-tested",
        },
      });
    } catch (e) {
      console.error(
        `[compat-batch] start_compat_test threw for ${work.name}:`,
        e,
      );
      // Don't stop the batch — count it as broken and move on.
      count.tested += 1;
      count.broken += 1;
      continue;
    }

    count.tested += 1;
    switch (outcome.status) {
      case "AliveRenders":
        count.working += 1;
        break;
      case "AliveNoRender":
        count.review += 1;
        break;
      case "EarlyExit":
      case "Crash":
      case "NoLaunch":
      case "InstallFailed":
        count.broken += 1;
        break;
    }

    currentName.value = null;
  }

  // After the loop, refresh the cached summary so the per-game panels
  // (in any open library views) reflect every result the batch posted.
  try {
    await refreshCompatSummary();
  } catch (e) {
    console.warn("[compat-batch] post-batch summary refresh failed:", e);
  }

  running.value = false;
  done.value = true;
  stopRequested.value = false;
  currentName.value = null;
}
</script>
