<template>
  <div
    class="mx-auto w-full relative flex flex-col justify-center pt-72 overflow-hidden"
  >
    <!-- Options gear menu — pinned top-left above the banner. -->
    <GameDetailOptionsMenu
      :config="config"
      :has-achievements="stats.achievements.value.length > 0"
      @reset-achievements="resetConfirmOpen = true"
      @remove-from-library="removeConfirmOpen = true"
    />

    <!-- Banner, title, action buttons, stat bar. -->
    <GameDetailHeader
      :game="game"
      :status="status"
      :version="version"
      :banner-url="bannerUrl"
      :stats-loading="stats.statsLoading.value"
      :game-stats="stats.gameStats"
      :dev-mode="devMode"
      @install="installCtl.openInstallFlow()"
      @launch="launchCtl.launch()"
      @queue="goToQueue()"
      @uninstall="launchCtl.uninstall()"
      @kill="launchCtl.kill()"
      @options="configureModalOpen = true"
      @resume="launchCtl.resumeDownload()"
      @compat-result="onCompatTestResult"
    />

    <!-- Tabbed content — About / Gallery / Achievements. Mirrors the BPM
         detail page so desktop users get the same per-tab depth. -->
    <div class="relative z-10">
      <div class="mt-8 w-full bg-zinc-900 px-8">
        <div class="flex items-center gap-1 border-b border-zinc-700/50">
          <button
            v-for="tab in detailTabs"
            :key="tab.value"
            class="relative px-5 py-3 text-sm font-medium transition-colors"
            :class="
              activeDetailTab === tab.value
                ? 'text-blue-400'
                : 'text-zinc-400 hover:text-zinc-200'
            "
            @click="activeDetailTab = tab.value"
          >
            {{ tab.label }}
            <span
              v-if="activeDetailTab === tab.value"
              class="absolute bottom-0 left-0 right-0 h-0.5 bg-blue-500 rounded-full"
            />
          </button>
        </div>

        <div class="pt-6 pb-2">
          <!-- About — the rendered description. -->
          <div
            v-if="activeDetailTab === 'about'"
            class="bg-zinc-800/50 rounded-xl backdrop-blur-sm overflow-hidden"
          >
            <div class="px-6 py-5">
              <div
                v-html="htmlDescription"
                class="prose prose-invert prose-blue max-w-none"
              />
            </div>
          </div>

          <!-- Gallery — image carousel + fullscreen viewer. -->
          <GameDetailGallery
            v-else-if="activeDetailTab === 'gallery'"
            :image-ids="game.mImageCarouselObjectIds"
            :game-name="game.mName"
          />

          <!-- Achievements — ROM hash banner + list. -->
          <GameDetailAchievements
            v-else-if="activeDetailTab === 'achievements'"
            :achievements="stats.achievements.value"
            :loading="stats.achievementsLoading.value"
            :unlocked-count="stats.achievementsUnlocked.value"
            :rom-hash-result="stats.romHashResult.value"
          />
        </div>
      </div>
    </div>
  </div>

  <!-- Install flow modal. -->
  <GameDetailInstallModal
    :game="game"
    :install="installCtl"
    :game-compat="gameCompat"
  />

  <!-- Launch-options picker. -->
  <GameDetailLaunchOptionsModal
    :open="launchCtl.launchOptionsOpen.value"
    :game-name="game.mName"
    :options="launchCtl.launchOptions.value ?? []"
    @select="launchCtl.launchIndex($event)"
    @cancel="launchCtl.launchOptions.value = undefined"
  />

  <!--
  Dear future DecDuck,
  This v-if is necessary for Vue rendering reasons
  (it tries to access the game version for not installed games)
  You have already tried to remove it
  Don't.
  -->
  <GameOptionsModal
    v-if="
      status.type === 'Installed' &&
      (status.install_type.type == InstalledType.Installed ||
        status.install_type.type == InstalledType.SetupRequired)
    "
    v-model="configureModalOpen"
    :game-id="game.id"
  />

  <DependencyRequiredModal
    v-if="launchCtl.dependencyRequiredModal.value"
    v-model="launchCtl.dependencyRequiredModal.value"
  />

  <!-- Achievement reset confirmation. -->
  <Transition
    enter-active-class="ease-out duration-200"
    enter-from-class="opacity-0"
    enter-to-class="opacity-100"
    leave-active-class="ease-in duration-150"
    leave-from-class="opacity-100"
    leave-to-class="opacity-0"
  >
    <div
      v-if="resetConfirmOpen"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      @click.self="resetConfirmOpen = false"
    >
      <div
        class="w-full max-w-sm rounded-xl bg-zinc-900 border border-zinc-700 shadow-2xl"
      >
        <div class="px-6 py-5">
          <h3 class="text-base font-semibold font-display text-zinc-100">
            Reset Achievements
          </h3>
          <p class="mt-2 text-sm text-zinc-400">
            Reset all achievements for
            <span class="text-zinc-200 font-medium">{{ game.mName }}</span
            >? This cannot be undone.
          </p>
        </div>
        <div class="flex justify-end gap-3 border-t border-zinc-700 px-6 py-4">
          <button
            @click="resetConfirmOpen = false"
            class="rounded-md px-4 py-2 text-sm font-medium text-zinc-300 hover:bg-zinc-800 transition-colors"
          >
            Cancel
          </button>
          <button
            @click="executeResetAchievements"
            :disabled="stats.resetBusy.value"
            class="rounded-md px-4 py-2 text-sm font-medium text-white bg-red-600 hover:bg-red-700 disabled:opacity-50 transition-colors"
          >
            {{ stats.resetBusy.value ? "Resetting..." : "Reset" }}
          </button>
        </div>
      </div>
    </div>
  </Transition>

  <!-- Remove-from-library confirmation. -->
  <Transition
    enter-active-class="ease-out duration-200"
    enter-from-class="opacity-0"
    enter-to-class="opacity-100"
    leave-active-class="ease-in duration-150"
    leave-from-class="opacity-100"
    leave-to-class="opacity-0"
  >
    <div
      v-if="removeConfirmOpen"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      @click.self="removeConfirmOpen = false"
    >
      <div
        class="w-full max-w-sm rounded-xl bg-zinc-900 border border-zinc-700 shadow-2xl"
      >
        <div class="px-6 py-5">
          <h3 class="text-base font-semibold font-display text-zinc-100">
            Remove from Library
          </h3>
          <p class="mt-2 text-sm text-zinc-400">
            Remove
            <span class="text-zinc-200 font-medium">{{ game.mName }}</span>
            from your library? Your local install isn't touched, but the
            game won't appear in your library again until you re-add it
            from the store.
          </p>
          <p v-if="removeError" class="mt-2 text-sm text-red-400">
            {{ removeError }}
          </p>
        </div>
        <div class="flex justify-end gap-3 border-t border-zinc-700 px-6 py-4">
          <button
            @click="removeConfirmOpen = false"
            class="rounded-md px-4 py-2 text-sm font-medium text-zinc-300 hover:bg-zinc-800 transition-colors"
          >
            Cancel
          </button>
          <button
            @click="executeRemoveFromLibrary"
            :disabled="removeBusy"
            class="rounded-md px-4 py-2 text-sm font-medium text-white bg-red-600 hover:bg-red-700 disabled:opacity-50 transition-colors"
          >
            {{ removeBusy ? "Removing..." : "Remove" }}
          </button>
        </div>
      </div>
    </div>
  </Transition>

  <!-- Cloud save conflict resolution. -->
  <SaveConflictDialog
    v-model="saveConflictOpen"
    :game-id="game.id"
    :conflicts="saveConflicts"
  />
</template>

<script setup lang="ts">
/**
 * Library game-detail page — banner, tabbed content (About / Gallery /
 * Achievements), install/launch controls, and the per-game options menu.
 *
 * This page is a thin coordinator. The heavy lifting lives in:
 *   - `useGameInstall`  — install-flow state + `download_game` calls
 *   - `useGameLaunch`   — launch / kill / uninstall / resume actions
 *   - `useGameStats`    — stats bar, achievements, ROM-hash verification
 *   - `useGameConfig`   — emulated-game presets + Goldberg account name
 * and the matching `components/game-detail/*` child components. See
 * `docs/audit/desktop-frontend-2026.md` for the decomposition rationale.
 */
import { micromark } from "micromark";
import { useListen } from "~/composables/useListen";
import { InstalledType } from "~/types";
import { rewriteDescriptionImages } from "~/composables/use-server-fetch";
import { useGameInstall } from "~/composables/game-detail/use-game-install";
import { useGameLaunch } from "~/composables/game-detail/use-game-launch";
import { useGameStats } from "~/composables/game-detail/use-game-stats";
import { useGameConfig } from "~/composables/game-detail/use-game-config";
import type { SaveConflict } from "~/types/save-sync";

const route = useRoute();
const router = useRouter();
const id = route.params.id.toString();

const { game, status, version } = await useGame(id);

const bannerUrl = await useObject(game.mBannerObjectId);

// Compat data scoped to this game. Soft-fails (returns null) so a
// server-side problem with the compat endpoints doesn't 500 the page.
const compatSummaryRef = await useCompatSummary().catch(() => null);
const gameCompat = computed(() => compatSummaryRef?.value?.[id]);

// Dev mode gates the per-game compat-test action button (the display
// panel itself is read-only and renders independent of dev mode).
const devMode = useDevMode();

const htmlDescription = rewriteDescriptionImages(
  micromark(game.mDescription),
);

// ── Composables ──────────────────────────────────────────────────────────
const installCtl = useGameInstall(game);
const launchCtl = useGameLaunch(game, status);
const stats = useGameStats(game.id);
const config = useGameConfig(game, version);

// ── Modal / tab UI state ─────────────────────────────────────────────────
const configureModalOpen = ref(false);
const resetConfirmOpen = ref(false);
const removeConfirmOpen = ref(false);
const removeBusy = ref(false);
const removeError = ref<string | undefined>();

const detailTabs = [
  { label: "About", value: "about" },
  { label: "Gallery", value: "gallery" },
  { label: "Achievements", value: "achievements" },
] as const;
const activeDetailTab =
  ref<(typeof detailTabs)[number]["value"]>("about");

function goToQueue() {
  router.push("/queue");
}

async function executeResetAchievements() {
  const ok = await stats.resetAchievements();
  if (ok) resetConfirmOpen.value = false;
}

/**
 * Remove the current game from the user's library. Mirrors the BPM detail
 * page (DELETE /api/v1/collection/default/entry with `{ id }` body). Local
 * install state is untouched server-side — the user is just dropping the
 * collection entry. Route back to /library on success so the now-missing
 * page doesn't 404 on its own data.
 */
async function executeRemoveFromLibrary() {
  removeBusy.value = true;
  removeError.value = undefined;
  try {
    const resp = await fetch(serverUrl("api/v1/collection/default/entry"), {
      method: "DELETE",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ id: game.id }),
    });
    if (!resp.ok) {
      removeError.value = `Server returned ${resp.status} ${resp.statusText}`;
      return;
    }
    removeConfirmOpen.value = false;
    router.push("/library");
  } catch (e) {
    removeError.value = String(e);
  } finally {
    removeBusy.value = false;
  }
}

// ── Compat test result summary ───────────────────────────────────────────
type CompatTestOutcome = {
  status: string;
  signature: string | null;
  elapsedSecs: number;
  posted: boolean;
  protonVersion?: string | null;
};

const COMPAT_STATUS_LABELS: Record<string, string> = {
  AliveRenders: "Plays correctly",
  AliveNoRender: "Launches but no visible render",
  EarlyExit: "Exits before main menu",
  Crash: "Crashes on launch",
  NoLaunch: "Won't launch",
  InstallFailed: "Install failed",
};

/**
 * Show a one-shot summary of what the compat test found. The result has
 * already been POSTed to drop-server by the Rust side before this fires;
 * the modal is purely informational.
 */
function onCompatTestResult(outcome: unknown) {
  const o = outcome as CompatTestOutcome;
  const label = COMPAT_STATUS_LABELS[o.status] ?? o.status;
  const lines = [`Result: ${label}`, `Observed for ${o.elapsedSecs}s.`];
  if (o.protonVersion) lines.push(`Runtime: ${o.protonVersion}`);
  if (o.signature) lines.push(`Signature: ${o.signature}`);
  if (!o.posted) {
    lines.push(
      "(Server didn't accept the result — may be offline or unauthenticated.)",
    );
  }
  createModal(
    ModalType.Notification,
    {
      title: `Compatibility test — ${game.mName}`,
      description: lines.join("\n"),
      buttonText: "OK",
    },
    (_e, c) => c(),
  );
}

// ── Cloud save conflict resolution ───────────────────────────────────────
const saveConflictOpen = ref(false);
const saveConflicts = ref<SaveConflict[]>([]);

useListen<{ gameId: string; conflicts: SaveConflict[] }>(
  `save_sync_conflict/${game.id}`,
  (event) => {
    saveConflicts.value = event.payload.conflicts;
    saveConflictOpen.value = true;
  },
);
</script>

<style scoped>
.slide-enter-active,
.slide-leave-active {
  transition: all 0.3s ease;
  position: absolute;
}

.slide-enter-from {
  opacity: 0;
  transform: translateX(100%);
}

.slide-leave-to {
  opacity: 0;
  transform: translateX(-100%);
}
</style>
