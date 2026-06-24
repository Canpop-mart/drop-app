<template>
  <div
    class="mx-auto w-full relative flex flex-col justify-center pt-72 overflow-hidden"
  >
    <!-- Options gear menu — pinned top-left above the banner. Now
         carries every per-game action including Configure and
         Uninstall, which used to live in a separate chevron dropdown
         on the Play button (consolidated v3.6 — single discoverable
         options surface). -->
    <GameDetailOptionsMenu
      :config="config"
      :has-achievements="stats.achievements.value.length > 0"
      :show-configure="
        status.type === 'Installed' &&
        status.install_type.type !== InstalledType.PartiallyInstalled
      "
      :show-uninstall="status.type === 'Installed'"
      :show-install-vcredist="
        status.type === 'Installed' &&
        !config.isEmulatedGame.value &&
        isLinuxHost
      "
      @configure="configureModalOpen = true"
      @uninstall="launchCtl.uninstall()"
      @install-runtime="installRuntime"
      @reset-achievements="resetConfirmOpen = true"
      @remove-from-library="removeConfirmOpen = true"
    />

    <!--
      Incognito launch — intentionally has NO visible UI.  Activation is
      Shift+click on the Play button (handled inside GameStatusButton
      via a `launch-incognito` emit when shiftKey is held).  No badge,
      no pill, no purple indicator during an active session — pure
      stealth so casual eyes on the screen see nothing that hints at
      the feature.  See use-game-launch.ts for the suppression flow.
    -->

    <!-- Banner, title, action buttons, stat bar (now includes the Friends
         pill as an inline 4th stat — was previously a standalone tile
         below the stat bar, which made the page feel cluttered). -->
    <GameDetailHeader
      :game="game"
      :status="status"
      :version="version"
      :banner-url="bannerUrl"
      :stats-loading="stats.statsLoading.value"
      :game-stats="stats.gameStats"
      :dev-mode="devMode"
      :players="friendsExcludingMe"
      :launch-in-flight="launchCtl.launchInFlight.value"
      :prep-status="launchCtl.prepStatus.value"
      @install="installCtl.openInstallFlow()"
      @launch="launchCtl.launch()"
      @launch-incognito="launchCtl.launchIncognito()"
      @queue="goToQueue()"
      @kill="launchCtl.kill()"
      @resume="launchCtl.resumeDownload()"
      @compat-result="onCompatTestResult"
      @open-community="activeDetailTab = 'community'"
    />

    <!-- Tabbed content — About / Gallery / Achievements / Community /
         Cloud Saves. Cloud Saves used to sit perma-visible below the
         tabbed area, which made the page noisy on games you don't have
         saves for; promoting it to a tab keeps the surface available
         without competing for space. -->
    <div class="relative z-10">
      <div class="mt-6 w-full bg-zinc-900 px-8">
        <div class="flex items-center gap-1 border-b border-zinc-700/50">
          <button
            v-for="tab in visibleDetailTabs"
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
          <!-- About — description + gallery, both as collapsible
               sections.  Gallery used to live as its own tab; folded
               into About now since the two are sibling "background
               about the game" surfaces. -->
          <div
            v-if="activeDetailTab === 'about'"
            class="space-y-4"
          >
            <CollapsibleSection title="Description">
              <div
                v-html="htmlDescription"
                class="prose prose-invert prose-blue max-w-none"
              />
            </CollapsibleSection>

            <CollapsibleSection
              v-if="game.mImageCarouselObjectIds.length > 0"
              title="Gallery"
              :badge="`${game.mImageCarouselObjectIds.length} images`"
            >
              <GameDetailGallery
                :image-ids="game.mImageCarouselObjectIds"
                :game-name="game.mName"
              />
            </CollapsibleSection>
          </div>

          <!-- Community — two-column layout.  Main column holds the
               achievement list (the meat of the tab); the sidebar
               column carries the wider community signal (leaderboard
               + first-to-unlock + recent activity).  Every section is
               a collapsible so users can fold away what they don't
               want to see. -->
          <div
            v-else-if="activeDetailTab === 'community'"
            class="grid xl:grid-cols-[1fr_360px] gap-4"
          >
            <div class="min-w-0 space-y-4">
              <CollapsibleSection
                title="Achievements"
                :badge="
                  stats.achievements.value.length > 0
                    ? `${stats.achievementsUnlocked.value} / ${stats.achievements.value.length}`
                    : undefined
                "
              >
                <GameDetailAchievements
                  :achievements="stats.achievements.value"
                  :loading="stats.achievementsLoading.value"
                  :unlocked-count="stats.achievementsUnlocked.value"
                  :rom-hash-result="stats.romHashResult.value"
                  :firsts-map="gameFirstsMap"
                />
              </CollapsibleSection>
            </div>
            <aside class="space-y-4 min-w-0">
              <CollapsibleSection
                title="Leaderboard"
                :badge="
                  gamePlayers.length > 0
                    ? `${gamePlayers.length} ${
                        gamePlayers.length === 1 ? 'player' : 'players'
                      }`
                    : undefined
                "
              >
                <GameCommunityTab
                  :game-id="game.id"
                  :players="gamePlayers"
                  :firsts="gameFirsts"
                />
              </CollapsibleSection>
            </aside>
          </div>

          <CloudSavesPanel
            v-else-if="activeDetailTab === 'saves' && devMode.enabled.value"
            :game-id="game.id"
            :game-name="game.mName"
            :is-native-game="config.isNativeGame.value"
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
import { renderMarkdown } from "~/composables/render-markdown";
import { invoke } from "@tauri-apps/api/core";
import { platform } from "@tauri-apps/plugin-os";
import { useListen } from "~/composables/useListen";
import { InstalledType } from "~/types";
import { useGameInstall } from "~/composables/game-detail/use-game-install";
import { useGameLaunch } from "~/composables/game-detail/use-game-launch";
import { useGameStats } from "~/composables/game-detail/use-game-stats";
import { useGameConfig } from "~/composables/game-detail/use-game-config";
import {
  useServerApi,
  type GamePlayerEntry,
  type GameAchievementFirst,
} from "~/composables/use-server-api";
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

const htmlDescription = renderMarkdown(game.mDescription);

// ── Composables ──────────────────────────────────────────────────────────
const installCtl = useGameInstall(game);
const launchCtl = useGameLaunch(game, status);
const stats = useGameStats(game.id);
const config = useGameConfig(game, version);

// VC++ install only applies to Windows games launched via Proton — i.e. on a
// Linux host. Gate the menu item on this so it never shows on Windows/macOS.
const isLinuxHost = computed(() => platform() === "linux");

// Clear the active-incognito latch as soon as the backend reports the
// process has exited. The event payload carries only the game id so we
// gate on that to avoid clearing a sibling page's overlay. The Rust side
// emits `game_process_exited` from `on_process_finish` regardless of how
// the game ended (clean exit, crash, manual kill).
useListen<string>("game_process_exited", (evt) => {
  if (evt.payload === game.id) launchCtl.incognitoActive.value = false;
});

// ── Modal / tab UI state ─────────────────────────────────────────────────
const configureModalOpen = ref(false);
const resetConfirmOpen = ref(false);
const removeConfirmOpen = ref(false);
const removeBusy = ref(false);
const removeError = ref<string | undefined>();

// Achievements + community used to be two separate tabs; they're now
// stacked under "Community" since the achievement list is itself a
// community signal (your progress vs server firsts). Gallery used to
// be its own tab too but is now folded into About as a sibling
// collapsible.
const detailTabs = [
  { label: "About", value: "about" },
  { label: "Community", value: "community" },
  { label: "Cloud Saves", value: "saves" },
] as const;
const activeDetailTab =
  ref<(typeof detailTabs)[number]["value"]>("about");

// Cloud saves is dev-gated, so its tab only appears when dev mode is on.
const visibleDetailTabs = computed(() =>
  detailTabs.filter((t) => t.value !== "saves" || devMode.enabled.value),
);

// ── Community surfaces ───────────────────────────────────────────────────
// Per-game players + first-to-unlock are fetched once at the page level
// and threaded into both the Friends tile (above the tabs), the Community
// tab body, and the Achievements tab (which uses the firsts map to mark
// achievements with a gold ring). Soft-fail to empty arrays so a missing
// Agent C endpoint doesn't blank the page.
const api = useServerApi();
const gamePlayers = ref<GamePlayerEntry[]>([]);
const gameFirsts = ref<GameAchievementFirst[]>([]);

const gameFirstsMap = computed(() => {
  const m: Record<string, GameAchievementFirst> = {};
  for (const f of gameFirsts.value) m[f.achievementId] = f;
  return m;
});

// Track the current user's id so we can drop ourselves from the
// "Friends Played" tile — "friends" doesn't include "me". The community
// leaderboard (which DOES include the caller) keeps the full list.
const myUserId = ref<string | null>(null);

// Same list with the caller filtered out — drives the Friends Played
// header chip. Empty until `myUserId` resolves, at which point the
// computed re-runs and the chip count drops by one if needed.
const friendsExcludingMe = computed(() =>
  myUserId.value
    ? gamePlayers.value.filter((p) => p.userId !== myUserId.value)
    : gamePlayers.value,
);

onMounted(() => {
  api.community
    .gamePlayers(game.id)
    .then((p) => (gamePlayers.value = p))
    .catch(() => (gamePlayers.value = []));
  api.community
    .gameFirsts(game.id)
    .then((f) => (gameFirsts.value = f))
    .catch(() => (gameFirsts.value = []));
  // Resolve `me` once; soft-fail because the page works without it
  // (the friends chip just won't strip the caller from its count).
  api.profile
    .me()
    .then((me) => (myUserId.value = me.id))
    .catch(() => (myUserId.value = null));
});

function goToQueue() {
  router.push("/queue");
}

// ── Manual VC++ runtime install ──────────────────────────────────────────
// Runs winetricks (vcrun2022 + d3dcompiler_47) against this game's Proton
// prefix on demand. Progress surfaces automatically via the existing
// `game_prep_status` listener in useGameLaunch (the header shows
// "Installing Visual C++ runtime..."). The guard prevents a second concurrent
// run if the user re-opens the menu mid-install.
const installingVc = ref(false);
async function installRuntime(set: string) {
  if (installingVc.value) return;
  installingVc.value = true;
  try {
    await invoke("install_redists", { gameId: game.id, runtimes: [set] });
    createModal(
      ModalType.Notification,
      {
        title: `Runtime installed: ${game.mName}`,
        description:
          "The selected runtime was installed into this game's Proton prefix. " +
          "If the game was failing with a missing-DLL error, try launching it again.",
        buttonText: "OK",
      },
      (_e, c) => c(),
    );
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    createModal(
      ModalType.Notification,
      {
        title: "Couldn't install runtime",
        description: `Drop couldn't install the runtime for "${game.mName}": ${msg}`,
        buttonText: "Close",
      },
      (_e, c) => c(),
    );
  } finally {
    installingVc.value = false;
  }
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
      "(Server didn't accept the result. It may be offline or unauthenticated.)",
    );
  }
  createModal(
    ModalType.Notification,
    {
      title: `Compatibility test: ${game.mName}`,
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
