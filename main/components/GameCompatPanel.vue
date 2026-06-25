<template>
  <div
    v-if="hasResults"
    class="bg-zinc-800/50 rounded-xl backdrop-blur-sm overflow-hidden"
  >
    <button
      class="w-full flex items-center justify-between p-6 text-left hover:bg-zinc-700/30 transition-colors"
      @click="open = !open"
    >
      <h2 class="text-xl font-display font-semibold text-zinc-100">
        Compatibility
      </h2>
      <ChevronDownIcon
        class="size-5 text-zinc-400 transition-transform duration-200"
        :class="{ 'rotate-180': open }"
      />
    </button>
    <Transition
      enter-active-class="transition-all duration-300 ease-out"
      enter-from-class="max-h-0 opacity-0"
      enter-to-class="max-h-[600px] opacity-100"
      leave-active-class="transition-all duration-200 ease-in"
      leave-from-class="max-h-[600px] opacity-100"
      leave-to-class="max-h-0 opacity-0"
    >
      <div v-show="open" class="overflow-hidden">
        <div class="px-6 pb-6 space-y-3">
          <p class="text-xs text-zinc-500">
            Test results from your devices. The latest result per platform
            wins.
          </p>
          <div
            v-for="row in rows"
            :key="row.platform"
            class="flex items-start gap-3 text-sm"
          >
            <span
              :class="[
                'shrink-0 px-2 py-0.5 rounded-md text-xs font-bold leading-tight',
                row.colorClasses,
              ]"
            >
              {{ row.platform }}
            </span>
            <div class="flex-1 min-w-0">
              <div class="text-zinc-100 font-medium">{{ row.label }}</div>
              <div
                v-if="row.signature"
                class="text-zinc-500 text-xs truncate font-mono"
              >
                {{ row.signature }}
              </div>
              <div class="text-zinc-600 text-[10px] mt-0.5">
                <span v-if="row.protonVersion">{{ row.protonVersion }} • </span>
                {{ formatTime(row.testedAt) }}
              </div>

              <!-- Quick-promote buttons for batch-tested AliveNoRender
                   results. The orchestrator can't tell from the outside
                   whether the screen showed real frames, so it tags
                   inconclusive results "needs review" — these buttons
                   let the user clear the queue without re-launching the
                   game. -->
              <div
                v-if="row.status === 'AliveNoRender'"
                class="mt-2 flex items-center gap-2"
              >
                <button
                  class="text-xs px-2 py-0.5 rounded bg-emerald-600 hover:bg-emerald-500 text-white font-semibold disabled:opacity-50 transition-colors"
                  :disabled="confirming === row.platform"
                  @click="confirmRendered(row.platform, true)"
                >
                  ✓ Played correctly
                </button>
                <button
                  class="text-xs px-2 py-0.5 rounded bg-rose-700 hover:bg-rose-600 text-white font-semibold disabled:opacity-50 transition-colors"
                  :disabled="confirming === row.platform"
                  @click="confirmRendered(row.platform, false)"
                >
                  ✗ Didn't render
                </button>
                <span
                  v-if="confirming === row.platform"
                  class="text-zinc-500 text-[10px]"
                >
                  saving…
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ChevronDownIcon } from "@heroicons/vue/20/solid";
import { invoke } from "@tauri-apps/api/core";
import type {
  GameCompatibilityStatus,
  ClientPlatform,
  GameCompatSummary,
} from "~/composables/use-compat-summary";

const { compat = undefined, gameId } = defineProps<{
  /**
   * Compat summary for this game. Pass `useCompatSummary().value[gameId]`
   * from the parent. Component renders nothing if there's no data.
   */
  compat?: GameCompatSummary | undefined;
  /**
   * Required when there are pending AliveNoRender rows the user might
   * want to promote to AliveRenders — the confirm action POSTs the new
   * status keyed by gameId. Pass undefined to hide the buttons (e.g.
   * read-only contexts where promotion shouldn't fire).
   */
  gameId?: string;
}>();

const open = ref(true);

// Tracks which platform's confirm button is mid-flight so we can show
// "saving…" and disable both buttons for that row. Cleared as soon as
// the post resolves (success or fail).
const confirming = ref<ClientPlatform | null>(null);

async function confirmRendered(platform: ClientPlatform, rendered: boolean) {
  if (!gameId) return;
  confirming.value = platform;
  try {
    await invoke("confirm_compat_render", { gameId, rendered });
    // Refresh the cached summary so the panel reflects the promoted
    // status without a hard reload. Safe to call even if no other
    // component is consuming the composable — it's idempotent.
    await refreshCompatSummary();
  } catch (err) {
    console.warn("[compat] confirm_compat_render failed:", err);
  } finally {
    confirming.value = null;
  }
}

const STATUS_LABEL: Record<GameCompatibilityStatus, string> = {
  AliveRenders: "Plays correctly",
  AliveNoRender: "Launches but doesn't render",
  EarlyExit: "Exits before main menu",
  Crash: "Crashes on launch",
  NoLaunch: "Doesn't launch",
  InstallFailed: "Install failed",
  Installing: "Install in progress",
  Testing: "Test in progress",
  Untested: "Not tested yet",
};

const STATUS_COLOR: Record<GameCompatibilityStatus, string> = {
  AliveRenders: "bg-emerald-600 text-white",
  AliveNoRender: "bg-amber-500 text-zinc-900",
  EarlyExit: "bg-rose-600 text-white",
  Crash: "bg-rose-700 text-white",
  NoLaunch: "bg-zinc-700 text-zinc-300",
  InstallFailed: "bg-zinc-700 text-zinc-300",
  Installing: "bg-blue-600 text-white",
  Testing: "bg-blue-600 text-white",
  Untested: "bg-zinc-800 text-zinc-500",
};

const ORDER: ClientPlatform[] = ["Windows", "Linux", "macOS"];

const rows = computed(() => {
  if (!compat) return [];
  return ORDER.filter((p) => compat[p]).map((p) => {
    const r = compat[p]!;
    return {
      platform: p,
      label: STATUS_LABEL[r.status],
      colorClasses: STATUS_COLOR[r.status],
      signature: r.signature,
      protonVersion: r.protonVersion,
      testedAt: r.testedAt,
      status: r.status,
    };
  });
});

const hasResults = computed(() => rows.value.length > 0);

function formatTime(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString();
}
</script>
