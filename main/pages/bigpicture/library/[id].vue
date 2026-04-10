<template>
  <div class="flex flex-col h-full overflow-y-auto">
    <!-- Hero banner or skeleton -->
    <div
      class="relative shrink-0"
      :class="deck.isDeckMode.value ? 'h-56' : 'h-96'"
    >
      <div v-if="!game" class="w-full h-full bg-zinc-800/50 animate-pulse" />
      <template v-else>
        <img
          v-if="game.mBannerObjectId"
          :src="objectUrl(game.mBannerObjectId)"
          class="w-full h-full object-cover"
        />
        <div v-else class="w-full h-full bg-zinc-800/30" />
      </template>
      <div
        v-if="game"
        class="absolute inset-0 bg-gradient-to-r from-zinc-950/95 via-zinc-950/70 to-transparent"
      />
      <div
        v-if="game"
        class="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/40 to-transparent"
      />

      <!-- Game info overlay -->
      <div
        v-if="game"
        class="absolute bottom-0 left-0 right-0"
        :class="deck.isDeckMode.value ? 'p-4' : 'p-8'"
      >
        <h1
          class="font-bold font-display text-zinc-100 mb-1"
          :class="deck.isDeckMode.value ? 'text-2xl' : 'text-5xl mb-2'"
        >
          {{ game?.mName }}
        </h1>
        <p
          v-if="game?.mShortDescription"
          class="text-zinc-400 max-w-2xl mb-4"
          :class="deck.isDeckMode.value ? 'text-sm mb-3' : 'text-lg mb-6'"
        >
          {{ game.mShortDescription }}
        </p>

        <!-- Action buttons -->
        <div class="flex items-center gap-3">
          <button
            v-if="status?.type === 'Installed'"
            :ref="(el: any) => registerAction(el, { onSelect: launchGame })"
            class="inline-flex items-center gap-2 bg-blue-600 hover:bg-blue-500 text-white font-semibold rounded-xl transition-colors shadow-lg shadow-blue-600/20"
            :class="
              deck.isDeckMode.value
                ? 'px-5 py-2.5 text-sm'
                : 'px-8 py-4 text-lg gap-3'
            "
            @click="launchGame"
          >
            <PlayIcon :class="deck.isDeckMode.value ? 'size-4' : 'size-6'" />
            Play
          </button>

          <button
            v-else-if="status?.type === 'Running'"
            :ref="(el: any) => registerAction(el, { onSelect: killGame })"
            class="inline-flex items-center gap-2 bg-red-600 hover:bg-red-500 text-white font-semibold rounded-xl transition-colors"
            :class="
              deck.isDeckMode.value
                ? 'px-5 py-2.5 text-sm'
                : 'px-8 py-4 text-lg gap-3'
            "
            @click="killGame"
          >
            <StopIcon :class="deck.isDeckMode.value ? 'size-4' : 'size-6'" />
            Stop
          </button>

          <button
            v-else-if="
              status?.type === 'Downloading' || status?.type === 'Queued'
            "
            class="inline-flex items-center gap-2 bg-zinc-700 text-zinc-300 font-semibold rounded-xl cursor-not-allowed"
            :class="
              deck.isDeckMode.value
                ? 'px-5 py-2.5 text-sm'
                : 'px-8 py-4 text-lg gap-3'
            "
            disabled
          >
            <ArrowDownTrayIcon
              :class="[
                deck.isDeckMode.value ? 'size-4' : 'size-6',
                'animate-bounce',
              ]"
            />
            {{ status?.type === "Downloading" ? "Downloading..." : "Queued" }}
          </button>

          <button
            v-else
            :ref="(el: any) => registerAction(el, { onSelect: downloadGame })"
            class="inline-flex items-center gap-2 bg-green-600 hover:bg-green-500 text-white font-semibold rounded-xl transition-colors"
            :class="
              deck.isDeckMode.value
                ? 'px-5 py-2.5 text-sm'
                : 'px-8 py-4 text-lg gap-3'
            "
            @click="downloadGame"
          >
            <ArrowDownTrayIcon
              :class="deck.isDeckMode.value ? 'size-4' : 'size-6'"
            />
            Install
          </button>

          <!-- Controller, Quality & Widescreen cycle buttons — only for emulated (RetroArch) games -->
          <template v-if="isEmulatedGame">
            <button
              :ref="(el: any) => registerAction(el, { onSelect: cycleController })"
              class="inline-flex items-center gap-1.5 bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300 rounded-xl transition-colors backdrop-blur-sm"
              :class="
                deck.isDeckMode.value
                  ? 'px-3 py-2 text-xs'
                  : 'px-4 py-3 text-sm'
              "
              @click="cycleController"
              :title="`Controller: ${controllerLabel}`"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="currentColor"
                class="size-4 text-blue-400"
              >
                <path
                  d="M17.5 3.5a3.5 3.5 0 00-3.5 3.5 3.5 3.5 0 003.5 3.5A3.5 3.5 0 0021 7a3.5 3.5 0 00-3.5-3.5zm-11 0A3.5 3.5 0 003 7a3.5 3.5 0 003.5 3.5A3.5 3.5 0 0010 7 3.5 3.5 0 006.5 3.5zM12 14c-3.3 0-10 1.7-10 5v2h20v-2c0-3.3-6.7-5-10-5z"
                />
              </svg>
              <span class="font-medium">{{ controllerLabel }}</span>
            </button>

            <button
              :ref="(el: any) => registerAction(el, { onSelect: cycleQuality })"
              class="inline-flex items-center gap-1.5 bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300 rounded-xl transition-colors backdrop-blur-sm"
              :class="
                deck.isDeckMode.value
                  ? 'px-3 py-2 text-xs'
                  : 'px-4 py-3 text-sm'
              "
              @click="cycleQuality"
              :title="`Quality: ${qualityLabel}`"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="currentColor"
                class="size-4 text-purple-400"
              >
                <path
                  fill-rule="evenodd"
                  d="M9 4.5a.75.75 0 01.721.544l.813 2.846a3.75 3.75 0 002.576 2.576l2.846.813a.75.75 0 010 1.442l-2.846.813a3.75 3.75 0 00-2.576 2.576l-.813 2.846a.75.75 0 01-1.442 0l-.813-2.846a3.75 3.75 0 00-2.576-2.576l-2.846-.813a.75.75 0 010-1.442l2.846-.813A3.75 3.75 0 007.466 7.89l.813-2.846A.75.75 0 019 4.5zM18 1.5a.75.75 0 01.728.568l.258 1.036c.236.94.97 1.674 1.91 1.91l1.036.258a.75.75 0 010 1.456l-1.036.258c-.94.236-1.674.97-1.91 1.91l-.258 1.036a.75.75 0 01-1.456 0l-.258-1.036a2.625 2.625 0 00-1.91-1.91l-1.036-.258a.75.75 0 010-1.456l1.036-.258a2.625 2.625 0 001.91-1.91l.258-1.036A.75.75 0 0118 1.5z"
                  clip-rule="evenodd"
                />
              </svg>
              <span class="font-medium">{{ qualityLabel }}</span>
            </button>

            <button
              :ref="(el: any) => registerAction(el, { onSelect: toggleWidescreen })"
              class="inline-flex items-center gap-1.5 rounded-xl transition-colors backdrop-blur-sm"
              :class="[
                deck.isDeckMode.value
                  ? 'px-3 py-2 text-xs'
                  : 'px-4 py-3 text-sm',
                widescreenEnabled
                  ? 'bg-green-600/80 hover:bg-green-500 text-white'
                  : 'bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300',
              ]"
              @click="toggleWidescreen"
              :title="`Widescreen: ${widescreenEnabled ? 'On' : 'Off'}`"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                class="size-4"
                :class="widescreenEnabled ? 'text-white' : 'text-green-400'"
              >
                <rect x="2" y="5" width="20" height="14" rx="2" />
                <path
                  v-if="widescreenEnabled"
                  d="M7 9l3 3-3 3M13 9h4M13 15h4"
                />
              </svg>
              <span class="font-medium">{{
                widescreenEnabled ? "16:9" : "4:3"
              }}</span>
            </button>
          </template>
        </div>
      </div>
    </div>

    <!-- Content tabs -->
    <div :class="deck.isDeckMode.value ? 'px-4 pt-2' : 'px-8 pt-4'">
      <div class="relative flex items-center gap-1 border-b border-zinc-800/50">
        <button
          v-for="tab in tabs"
          :key="tab.value"
          :ref="
            (el: any) => {
              registerTabRef(tab.value, el);
              registerTab(el, { onSelect: () => (activeTab = tab.value) });
            }
          "
          class="font-medium transition-colors relative"
          :class="[
            deck.isDeckMode.value ? 'px-3 py-2 text-xs' : 'px-5 py-3 text-sm',
            activeTab === tab.value
              ? 'text-blue-400'
              : 'text-zinc-400 hover:text-zinc-200',
          ]"
          @click="activeTab = tab.value"
        >
          {{ tab.label }}
        </button>

        <!-- Animated underline indicator -->
        <div
          class="absolute bottom-0 h-0.5 bg-blue-500 transition-all duration-300 ease-out"
          :style="tabIndicatorStyle"
        />
      </div>
    </div>

    <!-- Tab content -->
    <div
      class="flex-1"
      :class="deck.isDeckMode.value ? 'px-4 py-3' : 'px-8 py-6'"
    >
      <!-- Achievements -->
      <div v-if="activeTab === 'achievements'" class="space-y-2">
        <div
          v-for="achievement in achievements"
          :key="achievement.id"
          class="flex items-center gap-3 bg-zinc-900/50 rounded-xl"
          :class="[
            deck.isDeckMode.value ? 'p-3 gap-3' : 'p-4 gap-4',
            { 'opacity-50': !achievement.unlocked },
          ]"
        >
          <img
            v-if="achievement.iconUrl"
            :src="achievement.iconUrl"
            :class="
              deck.isDeckMode.value ? 'size-8 rounded-md' : 'size-12 rounded-lg'
            "
          />
          <div
            v-else
            :class="[
              deck.isDeckMode.value
                ? 'size-8 rounded-md'
                : 'size-12 rounded-lg',
              'bg-zinc-800 flex items-center justify-center',
            ]"
          >
            <TrophyIcon
              :class="[
                deck.isDeckMode.value ? 'size-4' : 'size-6',
                achievement.unlocked ? 'text-yellow-400' : 'text-zinc-600',
              ]"
            />
          </div>
          <div class="flex-1 min-w-0">
            <p
              class="font-medium text-zinc-200"
              :class="deck.isDeckMode.value ? 'text-xs' : 'text-sm'"
            >
              {{ achievement.title }}
            </p>
            <p
              class="text-zinc-500 truncate"
              :class="deck.isDeckMode.value ? 'text-[10px]' : 'text-sm'"
            >
              {{ achievement.description }}
            </p>
          </div>
          <TrophyIcon
            v-if="achievement.unlocked"
            class="size-4 text-yellow-400"
          />
        </div>
        <p
          v-if="achievements.length === 0"
          class="text-zinc-500 text-center py-8 text-sm"
        >
          No achievements available for this game.
        </p>
      </div>

      <!-- Details -->
      <div v-else-if="activeTab === 'details'" class="max-w-3xl">
        <p
          class="text-zinc-300 leading-relaxed whitespace-pre-line"
          :class="deck.isDeckMode.value ? 'text-sm' : ''"
        >
          {{ game?.mDescription || "No description available." }}
        </p>
      </div>

      <!-- Gallery -->
      <div
        v-else-if="activeTab === 'gallery'"
        class="grid gap-3"
        :class="deck.isDeckMode.value ? 'grid-cols-1' : 'grid-cols-2 gap-4'"
      >
        <img
          v-for="(imgId, idx) in game?.mImageCarouselObjectIds"
          :key="idx"
          :src="objectUrl(imgId)"
          class="w-full rounded-lg"
          loading="lazy"
        />
        <p
          v-if="!game?.mImageCarouselObjectIds?.length"
          class="text-zinc-500 col-span-2 text-center py-8 text-sm"
        >
          No screenshots available.
        </p>
      </div>
    </div>

    <!-- Launch error dialog -->
    <BigPictureDialog
      :visible="launchError !== null"
      title="Launch Failed"
      :message="launchError || ''"
      confirm-label="Dismiss"
      :show-cancel="false"
      @confirm="launchError = null"
    />

    <!-- Options menu dialog -->
    <BigPictureDialog
      :visible="showOptions"
      title="Game Options"
      @cancel="showOptions = false"
    >
      <div class="space-y-3">
        <!-- Set Account Name — only for native/Goldberg games -->
        <button
          v-if="isNativeGame"
          class="w-full text-left px-4 py-2 bg-zinc-800/50 hover:bg-zinc-700 rounded-lg transition-colors text-zinc-200 text-sm"
          @click="applyProfileName"
        >
          Set Account Name
        </button>

        <!-- Controller Layout — only for emulated (RetroArch) games -->
        <div v-if="isEmulatedGame" class="px-4 py-2 bg-zinc-800/50 rounded-lg">
          <p class="text-zinc-400 text-xs font-medium mb-2">
            Controller Layout
          </p>
          <div class="flex gap-1.5">
            <button
              v-for="opt in controllerOptions"
              :key="opt.label"
              class="flex-1 px-2 py-1.5 rounded text-xs font-medium transition-colors"
              :class="
                selectedController === opt.value
                  ? 'bg-blue-600 text-white'
                  : 'bg-zinc-700 text-zinc-300 hover:bg-zinc-600'
              "
              @click="setController(opt.value)"
            >
              {{ opt.label }}
            </button>
          </div>
        </div>

        <!-- Quality Preset -->
        <div v-if="isEmulatedGame" class="px-4 py-2 bg-zinc-800/50 rounded-lg">
          <p class="text-zinc-400 text-xs font-medium mb-2">Quality Preset</p>
          <div class="flex gap-1.5">
            <button
              v-for="opt in qualityOptions"
              :key="opt.label"
              class="flex-1 px-2 py-1.5 rounded text-xs font-medium transition-colors"
              :class="
                selectedQuality === opt.value
                  ? 'bg-purple-600 text-white'
                  : 'bg-zinc-700 text-zinc-300 hover:bg-zinc-600'
              "
              @click="setQuality(opt.value)"
            >
              {{ opt.label }}
            </button>
          </div>
        </div>

        <!-- Widescreen toggle -->
        <div v-if="isEmulatedGame" class="px-4 py-2 bg-zinc-800/50 rounded-lg">
          <div class="flex justify-between items-center">
            <p class="text-zinc-300 text-sm">Widescreen (16:9)</p>
            <button
              class="relative inline-flex h-6 w-11 items-center rounded-full transition-colors"
              :class="widescreenEnabled ? 'bg-green-600' : 'bg-zinc-700'"
              @click="toggleWidescreen"
            >
              <span
                class="inline-block h-4 w-4 rounded-full bg-white transition-transform"
                :class="widescreenEnabled ? 'translate-x-6' : 'translate-x-1'"
              />
            </button>
          </div>
        </div>

        <div class="border-t border-zinc-800 my-1" />

        <button
          class="w-full text-left px-4 py-2 bg-zinc-800/50 hover:bg-zinc-700 rounded-lg transition-colors text-zinc-200 text-sm"
          @click="openStore"
        >
          View on Store
        </button>
        <button
          class="w-full text-left px-4 py-2 bg-zinc-800/50 hover:bg-zinc-700 rounded-lg transition-colors text-zinc-200 text-sm"
          @click="checkForUpdates"
        >
          Check for Updates
        </button>
      </div>
    </BigPictureDialog>
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import {
  PlayIcon,
  StopIcon,
  ArrowDownTrayIcon,
  TrophyIcon,
} from "@heroicons/vue/24/solid";
import BigPictureDialog from "~/components/bigpicture/BigPictureDialog.vue";
import {
  useGame,
  type LaunchResult,
  type VersionOption,
} from "~/composables/game";
import { serverUrl } from "~/composables/use-server-fetch";
import type {
  ControllerType,
  QualityPreset,
  Game,
  GameStatus,
  GameVersion,
} from "~/types";

/** Load object images via server:// protocol (object:// doesn't work in dev) */
function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}
import { useDeckMode } from "~/composables/deck-mode";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { GamepadButton, useGamepad } from "~/composables/gamepad";

definePageMeta({ layout: "bigpicture" });

const route = useRoute();
const gameId = route.params.id as string;
const deck = useDeckMode();

// M1 fix: proper types instead of any
const game = ref<Game | null>(null);
// statusRef holds the reactive Ref returned by useGame — we read through it
const statusRef = shallowRef<any>(null);
const status = computed<GameStatus | null>(() => statusRef.value?.value ?? null);
const version = ref<GameVersion | null>(null);
const activeTab = ref("achievements");
const tabRefs = ref<Record<string, HTMLElement | null>>({});
const tabIndicatorStyle = ref({ left: "0", width: "0" });
const launchError = ref<string | null>(null);
const showOptions = ref(false);

const registerAction = useBpFocusableGroup("content");
const registerTab = useBpFocusableGroup("content");

const gamepad = useGamepad();
// C4 fix: store all gamepad unsubscribes for cleanup
const _unsubs: (() => void)[] = [];

// ── Game type detection ─────────────────────────────────────────────────
const isEmulatedGame = computed(() => {
  const ver = version.value;
  return ver?.launches?.some((l) => l.emulator != null) ?? false;
});
const isNativeGame = computed(() => !isEmulatedGame.value);

// ── Controller & Quality presets ─────────────────────────────────────────
const controllerOptions: { label: string; value: ControllerType | null }[] = [
  { label: "Auto", value: null },
  { label: "Xbox", value: "Xbox" },
  { label: "PS", value: "PlayStation" },
  { label: "Nintendo", value: "Nintendo" },
];
const qualityOptions: { label: string; value: QualityPreset | null }[] = [
  { label: "Auto", value: null },
  { label: "Low", value: "Low" },
  { label: "Med", value: "Medium" },
  { label: "High", value: "High" },
];

const selectedController = ref<ControllerType | null>(null);
const selectedQuality = ref<QualityPreset | null>(null);
const widescreenEnabled = ref(false);

async function saveUserConfig() {
  const ver = version.value;
  if (!ver) return;
  try {
    const currentConfig = ver.userConfiguration ?? {
      launchTemplate: "{}",
      overrideProtonPath: null,
      enableUpdates: false,
    };
    const config = {
      ...currentConfig,
      controllerType: selectedController.value,
      qualityPreset: selectedQuality.value,
      widescreen: widescreenEnabled.value,
    };
    await invoke("update_game_configuration", {
      gameId: gameId,
      options: config,
    });
  } catch (e) {
    console.error("Failed to save config:", e);
  }
}

function setController(value: ControllerType | null) {
  selectedController.value = value;
  saveUserConfig();
}

function setQuality(value: QualityPreset | null) {
  selectedQuality.value = value;
  saveUserConfig();
}

// ── Cycle helpers for inline buttons next to Play ───────────────────────
function cycleController() {
  const values = controllerOptions.map((o) => o.value);
  const idx = values.indexOf(selectedController.value);
  const next = values[(idx + 1) % values.length];
  setController(next);
}

function cycleQuality() {
  const values = qualityOptions.map((o) => o.value);
  const idx = values.indexOf(selectedQuality.value);
  const next = values[(idx + 1) % values.length];
  setQuality(next);
}

const controllerLabel = computed(() => {
  const match = controllerOptions.find(
    (o) => o.value === selectedController.value,
  );
  return match?.label ?? "Auto";
});

const qualityLabel = computed(() => {
  const match = qualityOptions.find((o) => o.value === selectedQuality.value);
  return match?.label ?? "Auto";
});

function toggleWidescreen() {
  widescreenEnabled.value = !widescreenEnabled.value;
  saveUserConfig();
}

async function applyProfileName() {
  showOptions.value = false;
  try {
    const msg = await invoke<string>("configure_game_emulator", {
      gameId: gameId,
    });
    console.log("[EMU]", msg);
  } catch (e) {
    console.error("[EMU] Failed to apply profile:", e);
  }
}

const tabs = [
  { label: "Achievements", value: "achievements" },
  { label: "Details", value: "details" },
  { label: "Gallery", value: "gallery" },
];

interface AchievementItem {
  id: string;
  title: string;
  description: string;
  iconUrl?: string;
  unlocked: boolean;
}

const achievements: Ref<AchievementItem[]> = ref([]);

function registerTabRef(value: string, el: any) {
  if (el) {
    tabRefs.value[value] = el;
    updateTabIndicator();
  }
}

function updateTabIndicator() {
  const activeEl = tabRefs.value[activeTab.value];
  if (activeEl) {
    tabIndicatorStyle.value = {
      left: `${activeEl.offsetLeft}px`,
      width: `${activeEl.offsetWidth}px`,
    };
  }
}

watch(activeTab, () => {
  nextTick(() => {
    updateTabIndicator();
  });
});

onMounted(async () => {
  try {
    const result = await useGame(gameId);
    game.value = result.game;
    statusRef.value = result.status;
    // Unwrap the version ref (useGame returns Ref<GameVersion | undefined>)
    version.value = result.version?.value ?? null;

    // Initialize controller/quality from saved config
    const ver = version.value;
    if (ver?.userConfiguration) {
      selectedController.value = ver.userConfiguration.controllerType ?? null;
      selectedQuality.value = ver.userConfiguration.qualityPreset ?? null;
      widescreenEnabled.value = ver.userConfiguration.widescreen ?? false;
    }
  } catch (e) {
    console.error("Failed to load game:", e);
  }

  try {
    const res = await fetch(serverUrl(`api/v1/games/${gameId}/achievements`));
    if (res.ok) {
      const data = await res.json();
      achievements.value = Array.isArray(data)
        ? data
        : (data.achievements ?? []);
    }
  } catch {
    // No achievements available
  }

  // Update tab indicator after content is mounted
  nextTick(() => {
    updateTabIndicator();
  });

  // C4 fix: store gamepad unsubscribes
  _unsubs.push(
    gamepad.onButton(GamepadButton.Start, () => {
      showOptions.value = true;
    }),
  );

  // Status updates are handled automatically by useGame's internal
  // listener on `update_game/{gameId}` which updates the shared status
  // ref in gameStatusRegistry.
});

// H4 fix: track window-level resize listener for tab indicator
function _onResize() {
  updateTabIndicator();
}
onMounted(() => {
  window.addEventListener("resize", _onResize);
});

onUnmounted(() => {
  // C4/H4 fix: clean up all subscriptions
  for (const unsub of _unsubs) unsub();
  _unsubs.length = 0;

  window.removeEventListener("resize", _onResize);
});

async function launchGame() {
  try {
    const result: LaunchResult = await invoke("launch_game", {
      id: gameId,
      index: 0,
    });
    if (result.result !== "Success") {
      launchError.value = `Failed to launch: ${result.result}`;
    }
  } catch (e) {
    launchError.value = `Launch error: ${e instanceof Error ? e.message : String(e)}`;
  }
}

async function killGame() {
  try {
    await invoke("kill_game", { id: gameId });
  } catch (e) {
    console.error("Failed to stop game:", e);
  }
}

async function downloadGame() {
  try {
    await invoke("download_game", { gameId });
  } catch (e) {
    console.error("Failed to start download:", e);
  }
}

function openStore() {
  navigateTo(`/store/${gameId}`);
}

async function checkForUpdates() {
  try {
    await invoke("check_for_updates", { gameId });
  } catch (e) {
    console.error("Failed to check for updates:", e);
  }
}
</script>