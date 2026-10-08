<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="fixed inset-0 z-[10000] overflow-y-auto bg-zinc-950/75"
      @click.self="close"
    >
      <div
        class="flex min-h-full items-start justify-center p-4 sm:items-center"
        @click.self="close"
      >
        <div
          role="dialog"
          aria-modal="true"
          class="relative w-full max-w-2xl rounded-lg bg-zinc-900 text-left shadow-xl border border-zinc-800"
        >
          <div class="px-6 pt-5 pb-4 space-y-4">
            <h3 class="text-base font-semibold font-display text-zinc-100">
              Update {{ gameName }}
            </h3>

            <!-- What "Repair update" did, above the re-checked review. -->
            <p
              v-if="update.repairNote.value && phase.kind !== 'repairing'"
              class="rounded-md bg-blue-500/10 p-3 text-sm text-blue-200 outline outline-1 outline-blue-500/20"
            >
              {{ update.repairNote.value }}
            </p>

            <!-- Loading -->
            <div
              v-if="phase.kind === 'loading' || phase.kind === 'repairing'"
              class="flex items-center gap-3 py-6 text-sm text-zinc-400"
            >
              <span
                class="size-4 rounded-full border-2 border-zinc-500/40 border-t-blue-400 animate-spin"
              />
              {{
                phase.kind === "repairing"
                  ? "Repairing the unfinished update..."
                  : "Checking what the update changes..."
              }}
            </div>

            <!-- Error (planning failed, or the apply was refused) -->
            <div
              v-else-if="phase.kind === 'error'"
              class="rounded-md bg-red-500/10 p-3 outline outline-1 outline-red-500/20"
            >
              <p class="text-sm text-red-300">
                {{ ERROR_LEAD[phase.during] }}
                {{ phase.message }}
              </p>
            </div>

            <!-- Nothing to do -->
            <p
              v-else-if="phase.kind === 'up_to_date'"
              class="py-4 text-sm text-zinc-300"
            >
              This install is already up to date.
            </p>

            <!-- Queued -->
            <p
              v-else-if="phase.kind === 'queued'"
              class="py-4 text-sm text-zinc-300"
            >
              The update is in the download queue.
            </p>

            <!-- The review -->
            <template v-else-if="plan">
              <p class="text-sm text-zinc-400">
                {{ targetLine(plan, versionName ?? ((v) => v)) }}
              </p>
              <div class="flex flex-wrap items-center gap-x-6 gap-y-1 text-sm">
                <span class="text-zinc-200">{{ countsLine(plan) }}</span>
                <span class="text-zinc-400">{{ downloadLine(plan) }}</span>
              </div>

              <div
                v-if="plan.baselineSource === 'none'"
                class="rounded-md bg-amber-500/10 p-3 outline outline-1 outline-amber-500/20"
              >
                <p class="text-xs text-amber-300">{{ BASELINE_NONE_NOTE }}</p>
              </div>

              <div
                v-if="skippedLinkedLine(plan)"
                class="rounded-md bg-amber-500/10 p-3 outline outline-1 outline-amber-500/20"
              >
                <p class="text-xs text-amber-300">{{ skippedLinkedLine(plan) }}</p>
              </div>

              <!-- Files replaced or removed with the player's copy kept as
                   .bak: information only, no choice. -->
              <div v-if="backupLine(plan)" class="space-y-2">
                <div class="flex flex-wrap items-center justify-between gap-2">
                  <p class="text-sm text-zinc-300">{{ backupLine(plan) }}</p>
                  <button
                    type="button"
                    class="rounded-md bg-zinc-800 px-3 py-1.5 text-xs font-medium text-zinc-200 hover:bg-zinc-700"
                    @click="showBackups = !showBackups"
                  >
                    {{ showBackups ? "Hide files" : "Show files" }}
                  </button>
                </div>
                <ul
                  v-if="showBackups"
                  class="max-h-48 overflow-y-auto divide-y divide-zinc-800 rounded-md border border-zinc-800"
                >
                  <li
                    v-for="path in plan.backupPaths"
                    :key="path"
                    class="truncate px-3 py-1.5 font-mono text-xs text-zinc-300"
                    :title="path"
                  >
                    {{ path }}
                  </li>
                </ul>
              </div>

              <div v-if="plan.conflicts.length > 0" class="space-y-2">
                <div class="flex flex-wrap items-center justify-between gap-2">
                  <p class="text-sm text-zinc-300">
                    {{ conflictsLine(plan.conflicts.length) }}
                  </p>
                  <div class="flex gap-2">
                    <button
                      type="button"
                      class="rounded-md bg-zinc-800 px-3 py-1.5 text-xs font-medium text-zinc-200 hover:bg-zinc-700"
                      :disabled="phase.kind !== 'ready'"
                      @click="update.chooseEvery('take_update')"
                    >
                      Take update for all
                    </button>
                    <button
                      type="button"
                      class="rounded-md bg-zinc-800 px-3 py-1.5 text-xs font-medium text-zinc-200 hover:bg-zinc-700"
                      :disabled="phase.kind !== 'ready'"
                      @click="update.chooseEvery('keep_mine')"
                    >
                      Keep mine for all
                    </button>
                  </div>
                </div>
                <ul
                  class="max-h-80 overflow-y-auto divide-y divide-zinc-800 rounded-md border border-zinc-800"
                >
                  <li
                    v-for="c in plan.conflicts"
                    :key="c.path"
                    class="flex items-center gap-3 px-3 py-2"
                  >
                    <div class="min-w-0 flex-1">
                      <p class="truncate font-mono text-xs text-zinc-100" :title="c.path">
                        {{ c.path }}
                      </p>
                      <p class="text-xs text-zinc-500">
                        {{ CONFLICT_KIND_LABEL[c.kind] }}.
                        {{ resolutionDetail(c.kind, choices[c.path]) }}
                      </p>
                    </div>
                    <div
                      class="inline-flex shrink-0 overflow-hidden rounded-md ring-1 ring-zinc-700"
                    >
                      <button
                        v-for="r in RESOLUTIONS"
                        :key="r"
                        type="button"
                        class="px-2.5 py-1 text-xs font-medium transition-colors"
                        :class="
                          choices[c.path] === r
                            ? 'bg-blue-600 text-white'
                            : 'bg-zinc-800 text-zinc-300 hover:bg-zinc-700'
                        "
                        :disabled="phase.kind !== 'ready'"
                        @click="update.choose(c.path, r)"
                      >
                        {{ RESOLUTION_LABEL[r] }}
                      </button>
                    </div>
                  </li>
                </ul>
              </div>
            </template>
          </div>

          <div
            class="flex items-center justify-end gap-3 border-t border-zinc-800 px-6 py-4"
          >
            <span
              v-if="phase.kind === 'ready' && update.unresolved.value.length > 0"
              class="mr-auto text-xs text-amber-300"
            >
              Choose for {{ update.unresolved.value.length }} more
              {{ update.unresolved.value.length === 1 ? "file" : "files" }}
            </span>
            <button
              type="button"
              class="rounded-md px-4 py-2 text-sm font-medium text-zinc-300 hover:bg-zinc-800"
              @click="close"
            >
              {{ phase.kind === "queued" || phase.kind === "up_to_date" ? "Close" : "Cancel" }}
            </button>
            <button
              v-if="phase.kind === 'error'"
              type="button"
              class="rounded-md bg-zinc-700 px-4 py-2 text-sm font-medium text-zinc-100 hover:bg-zinc-600"
              @click="update.recheck()"
            >
              Check again
            </button>
            <!-- An earlier update is stuck: repair it, then re-check. -->
            <button
              v-if="phase.kind === 'error' && phase.needsRecovery"
              type="button"
              class="rounded-md bg-blue-600 px-4 py-2 text-sm font-semibold text-white hover:bg-blue-500"
              @click="update.repairThenRecheck()"
            >
              Repair update
            </button>
            <button
              v-if="phase.kind === 'ready' || phase.kind === 'applying'"
              type="button"
              class="rounded-md bg-blue-600 px-4 py-2 text-sm font-semibold text-white hover:bg-blue-500 disabled:opacity-50"
              :disabled="!update.canApply.value"
              @click="update.apply()"
            >
              {{ phase.kind === "applying" ? "Starting..." : "Apply update" }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
/**
 * Desktop review for an in-place game update: what the update adds, changes
 * and removes, the download size, and every file the player changed that the
 * update also touches, each with "take update" or "keep mine". Apply queues
 * it. All state lives in the `useGameUpdate` instance the page passes in.
 *
 * Plain teleported overlay like InstallModal (the Headless UI Dialog did not
 * show in the packaged WebView2 build), so it handles Escape itself.
 */
import {
  BASELINE_NONE_NOTE,
  CONFLICT_KIND_LABEL,
  RESOLUTION_LABEL,
  backupLine,
  skippedLinkedLine,
  conflictsLine,
  countsLine,
  downloadLine,
  resolutionDetail,
  targetLine,
  type Resolution,
} from "~/composables/game-detail/update-review";
import type { GameUpdateController } from "~/composables/game-detail/use-game-update";

const props = defineProps<{
  update: GameUpdateController;
  gameName: string;
  /** Display name for a version id (falls back to the id). */
  versionName?: (versionId: string) => string;
}>();

const showBackups = ref(false);

const RESOLUTIONS: Resolution[] = ["take_update", "keep_mine"];

/** The sentence before the engine's error, by what was being attempted. */
const ERROR_LEAD = {
  plan: "Could not check this update.",
  apply: "The update was not started.",
  repair: "Could not repair the update.",
} as const;

const phase = computed(() => props.update.phase.value);
const plan = computed(() => props.update.plan.value);
const choices = computed(() => props.update.choices.value);
const open = computed(() => phase.value.kind !== "idle");

function close() {
  // An apply already handed to the engine finishes in the queue either way;
  // closing only drops the review.
  props.update.close();
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && open.value) close();
}

onMounted(() => window.addEventListener("keydown", onKeydown));
onUnmounted(() => window.removeEventListener("keydown", onKeydown));
</script>
