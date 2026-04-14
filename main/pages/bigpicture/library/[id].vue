<template>
  <div class="flex flex-col h-full overflow-y-auto">
    <!-- Hero banner -->
    <div class="relative shrink-0 h-96">
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
        class="absolute inset-0 bg-gradient-to-r from-zinc-950 via-zinc-950/80 to-zinc-950/20"
      />
      <div
        v-if="game"
        class="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/60 to-transparent"
      />

      <!-- Game info overlay -->
      <div v-if="game" class="absolute bottom-0 left-0 right-0 p-8">
        <h1 class="text-5xl font-bold font-display text-zinc-100 mb-2" style="text-shadow: 0 2px 8px rgba(0,0,0,0.8), 0 0 2px rgba(0,0,0,0.6)">
          {{ game?.mName }}
        </h1>
        <p
          v-if="game?.mShortDescription"
          class="text-lg text-zinc-400 max-w-2xl mb-6"
          style="text-shadow: 0 1px 4px rgba(0,0,0,0.8)"
        >
          {{ game.mShortDescription }}
        </p>

        <!-- Action buttons -->
        <div class="flex items-center gap-3">
          <button
            v-if="status?.type === 'Installed'"
            :ref="(el: any) => registerAction(el, { onSelect: launchGame })"
            class="inline-flex items-center px-8 py-4 text-lg gap-3 bg-blue-600 hover:bg-blue-400 text-white font-semibold rounded-xl transition-all shadow-lg shadow-blue-600/20 hover:shadow-blue-500/30 hover:scale-105"
            @click="launchGame"
          >
            <PlayIcon class="size-6" />
            Play
          </button>

          <button
            v-else-if="status?.type === 'Running'"
            :ref="(el: any) => registerAction(el, { onSelect: killGame })"
            class="inline-flex items-center px-8 py-4 text-lg gap-3 bg-red-600 hover:bg-red-500 text-white font-semibold rounded-xl transition-colors"
            @click="killGame"
          >
            <StopIcon class="size-6" />
            Stop
          </button>

          <button
            v-else-if="status?.type === 'Downloading' || status?.type === 'Queued'"
            class="inline-flex items-center px-8 py-4 text-lg gap-3 bg-zinc-700 text-zinc-300 font-semibold rounded-xl cursor-not-allowed"
            disabled
          >
            <ArrowDownTrayIcon class="size-6 animate-bounce" />
            {{ status?.type === "Downloading" ? "Downloading..." : "Queued" }}
          </button>

          <button
            v-else
            :ref="(el: any) => registerAction(el, { onSelect: downloadGame })"
            class="inline-flex items-center px-8 py-4 text-lg gap-3 bg-green-600 hover:bg-green-500 text-white font-semibold rounded-xl transition-colors"
            @click="downloadGame"
          >
            <ArrowDownTrayIcon class="size-6" />
            Install
          </button>

          <!-- Add to Library (without installing) — shows for Remote games not yet in library -->
          <button
            v-if="status?.type === 'Remote' && !inLibrary"
            :ref="(el: any) => registerAction(el, { onSelect: addToLibrary })"
            class="inline-flex items-center px-6 py-4 text-lg gap-3 bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300 rounded-xl transition-colors backdrop-blur-sm"
            @click="addToLibrary"
          >
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="size-5 text-blue-400">
              <path fill-rule="evenodd" d="M12 3.75a.75.75 0 01.75.75v6.75h6.75a.75.75 0 010 1.5h-6.75v6.75a.75.75 0 01-1.5 0v-6.75H4.5a.75.75 0 010-1.5h6.75V4.5a.75.75 0 01.75-.75z" clip-rule="evenodd" />
            </svg>
            {{ libraryLoading ? "Adding..." : "Add to Library" }}
          </button>
          <span
            v-if="status?.type === 'Remote' && inLibrary"
            class="inline-flex items-center px-4 py-3 text-sm text-zinc-500 gap-2"
          >
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="size-4 text-green-500">
              <path fill-rule="evenodd" d="M19.916 4.626a.75.75 0 01.208 1.04l-9 13.5a.75.75 0 01-1.154.114l-6-6a.75.75 0 011.06-1.06l5.353 5.353 8.493-12.739a.75.75 0 011.04-.208z" clip-rule="evenodd" />
            </svg>
            In Library
          </span>

          <!-- Controller, Quality & Widescreen cycle buttons — only for emulated games -->
          <template v-if="isEmulatedGame">
            <button
              :ref="(el: any) => registerAction(el, { onSelect: cycleController })"
              class="inline-flex items-center gap-1.5 px-4 py-3 text-sm bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300 rounded-xl transition-colors backdrop-blur-sm"
              @click="cycleController"
              :title="`Controller: ${controllerLabel}`"
            >
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="size-4 text-blue-400">
                <path d="M17.5 3.5a3.5 3.5 0 00-3.5 3.5 3.5 3.5 0 003.5 3.5A3.5 3.5 0 0021 7a3.5 3.5 0 00-3.5-3.5zm-11 0A3.5 3.5 0 003 7a3.5 3.5 0 003.5 3.5A3.5 3.5 0 0010 7 3.5 3.5 0 006.5 3.5zM12 14c-3.3 0-10 1.7-10 5v2h20v-2c0-3.3-6.7-5-10-5z" />
              </svg>
              <span class="font-medium">{{ controllerLabel }}</span>
            </button>

            <button
              :ref="(el: any) => registerAction(el, { onSelect: cycleQuality })"
              class="inline-flex items-center gap-1.5 px-4 py-3 text-sm bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300 rounded-xl transition-colors backdrop-blur-sm"
              @click="cycleQuality"
              :title="`Quality: ${qualityLabel}`"
            >
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="size-4 text-purple-400">
                <path fill-rule="evenodd" d="M9 4.5a.75.75 0 01.721.544l.813 2.846a3.75 3.75 0 002.576 2.576l2.846.813a.75.75 0 010 1.442l-2.846.813a3.75 3.75 0 00-2.576 2.576l-.813 2.846a.75.75 0 01-1.442 0l-.813-2.846a3.75 3.75 0 00-2.576-2.576l-2.846-.813a.75.75 0 010-1.442l2.846-.813A3.75 3.75 0 007.466 7.89l.813-2.846A.75.75 0 019 4.5zM18 1.5a.75.75 0 01.728.568l.258 1.036c.236.94.97 1.674 1.91 1.91l1.036.258a.75.75 0 010 1.456l-1.036.258c-.94.236-1.674.97-1.91 1.91l-.258 1.036a.75.75 0 01-1.456 0l-.258-1.036a2.625 2.625 0 00-1.91-1.91l-1.036-.258a.75.75 0 010-1.456l1.036-.258a2.625 2.625 0 001.91-1.91l.258-1.036A.75.75 0 0118 1.5z" clip-rule="evenodd" />
              </svg>
              <span class="font-medium">{{ qualityLabel }}</span>
            </button>

            <button
              :ref="(el: any) => registerAction(el, { onSelect: toggleWidescreen })"
              class="inline-flex items-center gap-1.5 px-4 py-3 text-sm rounded-xl transition-colors backdrop-blur-sm"
              :class="[
                aspectRatio !== 'Standard'
                  ? 'bg-green-600/80 hover:bg-green-500 text-white'
                  : 'bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300',
              ]"
              @click="toggleWidescreen"
              :title="`Aspect Ratio: ${aspectLabel}`"
            >
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="size-4" :class="aspectRatio !== 'Standard' ? 'text-white' : 'text-green-400'">
                <rect x="2" y="5" width="20" height="14" rx="2" />
                <path v-if="aspectRatio !== 'Standard'" d="M7 9l3 3-3 3M13 9h4M13 15h4" />
              </svg>
              <span class="font-medium">{{ aspectLabel }}</span>
            </button>
          </template>
        </div>
      </div>
    </div>

    <!-- Content tabs -->
    <div class="px-8 pt-4">
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
          class="px-5 py-3 text-sm font-medium transition-colors relative"
          :class="[
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
    <div class="flex-1 px-8 py-6">
      <!-- Achievements -->
      <div v-if="activeTab === 'achievements'" class="space-y-4">
        <!-- Achievement summary progress -->
        <div v-if="achievements.length > 0" class="flex items-center gap-3 px-1">
          <div class="flex-1 h-2 bg-zinc-800 rounded-full overflow-hidden">
            <div class="h-full bg-blue-500 rounded-full transition-all" :style="{ width: `${achievementPercent}%` }" />
          </div>
          <span class="text-sm font-medium text-zinc-400 flex-shrink-0">
            {{ unlockedCount }}/{{ achievements.length }}
          </span>
        </div>

        <!-- Achievement items -->
        <div class="space-y-2">
          <div
            v-for="achievement in achievements"
            :key="achievement.id"
            class="flex items-center gap-4 bg-zinc-900/50 rounded-xl p-4"
            :class="{ 'opacity-50': !achievement.unlocked }"
          >
            <img
              v-if="achievement.iconUrl"
              :src="achievement.iconUrl"
              class="size-12 rounded-lg bg-zinc-800"
              referrerpolicy="no-referrer"
              loading="lazy"
              @error="onAchievementIconError"
            />
            <div
              v-if="!achievement.iconUrl"
              class="size-12 rounded-lg bg-zinc-800 flex items-center justify-center"
            >
              <TrophyIcon
                class="size-6"
                :class="achievement.unlocked ? 'text-yellow-400' : 'text-zinc-600'"
              />
            </div>
            <div class="flex-1 min-w-0">
              <p class="text-sm font-medium text-zinc-200">
                {{ achievement.title }}
              </p>
              <p class="text-sm text-zinc-500 truncate">
                {{ achievement.description }}
              </p>
              <!-- Rarity bar -->
              <div v-if="achievement.rarity != null" class="flex items-center gap-2 mt-1.5">
                <div class="flex-1 h-1 bg-zinc-800 rounded-full overflow-hidden">
                  <div
                    class="h-full rounded-full transition-all"
                    :class="rarityColor(achievement.rarity)"
                    :style="{ width: `${Math.max(achievement.rarity, 2)}%` }"
                  />
                </div>
                <span class="text-xs tabular-nums flex-shrink-0" :class="rarityTextColor(achievement.rarity)">
                  {{ achievement.rarity.toFixed(1) }}%
                </span>
              </div>
            </div>
            <TrophyIcon
              v-if="achievement.unlocked"
              class="size-4 text-yellow-400"
            />
          </div>
        </div>

        <p
          v-if="achievements.length === 0"
          class="text-zinc-500 text-center py-8 text-sm"
        >
          No achievements available for this game.
        </p>
      </div>

      <!-- Details (markdown rendered) -->
      <div v-else-if="activeTab === 'details'" class="max-w-3xl">
        <div
          v-if="game?.mDescription"
          class="prose prose-invert prose-zinc max-w-none text-zinc-300 leading-relaxed"
          v-html="renderedDescription"
        />
        <p v-else class="text-zinc-500">No description available.</p>
      </div>

      <!-- Gallery -->
      <div
        v-else-if="activeTab === 'gallery'"
        class="grid grid-cols-2 gap-4"
      >
        <div
          v-for="(imgId, idx) in game?.mImageCarouselObjectIds"
          :key="idx"
          class="aspect-video rounded-lg overflow-hidden bg-zinc-800"
        >
          <img
            :src="objectUrl(imgId)"
            class="w-full h-full object-cover"
            loading="lazy"
          />
        </div>
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
      @confirm="dismissLaunchError"
    />

    <!-- Uninstall confirmation dialog -->
    <BigPictureDialog
      :visible="confirmUninstall"
      title="Uninstall Game"
      :message="`Are you sure you want to uninstall ${game?.mName ?? 'this game'}? This will delete all local game files.`"
      confirm-label="Uninstall"
      cancel-label="Cancel"
      :destructive="true"
      @confirm="doUninstall"
      @cancel="confirmUninstall = false"
    />

    <!-- Remove from library confirmation dialog -->
    <BigPictureDialog
      :visible="confirmRemoveFromLibrary"
      title="Remove from Library"
      :message="`Are you sure you want to remove ${game?.mName ?? 'this game'} from your library?`"
      confirm-label="Remove"
      cancel-label="Cancel"
      :destructive="true"
      @confirm="doRemoveFromLibrary"
      @cancel="confirmRemoveFromLibrary = false"
    />

    <!-- Options menu overlay — fully gamepad-navigable -->
    <Teleport to="body">
      <Transition
        enter-active-class="transition-opacity duration-200"
        leave-active-class="transition-opacity duration-200"
        enter-from-class="opacity-0"
        leave-to-class="opacity-0"
      >
        <div
          v-if="showOptions"
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
        >
          <div class="bg-zinc-900 border border-zinc-700/50 rounded-2xl shadow-2xl p-6 max-w-md w-full mx-4">
            <h2 class="text-xl font-semibold font-display text-zinc-100 mb-4">Game Options</h2>

            <div class="space-y-1.5">
              <button
                v-for="(item, idx) in optionsMenuItems"
                :key="item.id"
                class="w-full flex items-center justify-between px-4 py-3 rounded-xl text-sm transition-colors"
                :class="optionsFocusIdx === idx
                  ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/20'
                  : 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700'"
                @click="item.action()"
              >
                <span class="font-medium">{{ item.label }}</span>
                <span v-if="item.valueLabel" class="text-xs opacity-75">{{ item.valueLabel }}</span>
              </button>
            </div>

            <!-- Hints -->
            <div class="flex gap-6 mt-4 text-xs text-zinc-500 justify-end">
              <BigPictureButtonPrompt button="A" label="Select" size="sm" />
              <BigPictureButtonPrompt button="B" label="Close" size="sm" />
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
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
import BigPictureButtonPrompt from "~/components/bigpicture/BigPictureButtonPrompt.vue";
import {
  useGame,
  type LaunchResult,
  type VersionOption,
} from "~/composables/game";
import { serverUrl } from "~/composables/use-server-fetch";
import type {
  AspectRatio,
  ControllerType,
  QualityPreset,
  Game,
  GameStatus,
  GameVersion,
} from "~/types";

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { GamepadButton, useGamepad } from "~/composables/gamepad";

definePageMeta({ layout: "bigpicture" });

console.log("[BPM:GAME] >>> Script setup executing (synchronous) <<<");

const route = useRoute();
const gameId = route.params.id as string;
console.log(`[BPM:GAME] Route param gameId: ${gameId}`);

const game = ref<Game | null>(null);
const statusRef = shallowRef<any>(null);
const status = computed<GameStatus | null>(() => statusRef.value?.value ?? null);
const version = ref<GameVersion | null>(null);
const versionOptions = ref<VersionOption[] | null>(null);
const activeTab = ref("achievements");
// Plain object — NOT reactive. Storing DOM refs in a reactive ref causes
// infinite update loops when set from :ref callbacks during render.
const tabRefs: Record<string, HTMLElement | null> = {};
const tabIndicatorStyle = ref({ left: "0", width: "0" });
const launchError = ref<string | null>(null);
const diagnosticsRan = ref(false);

/** Run launch diagnostics and log to console for debug capture */
async function runDiagnostics() {
  if (diagnosticsRan.value) return;
  diagnosticsRan.value = true;
  try {
    const diag = await invoke("diagnose_launch_environment");
    console.log("[BPM:DIAG] === LAUNCH DIAGNOSTICS ===");
    console.log("[BPM:DIAG] UMU installed:", (diag as any).umu_installed, "path:", (diag as any).umu_path);
    console.log("[BPM:DIAG] Proton default:", (diag as any).proton_default, "valid:", (diag as any).proton_default_valid);
    console.log("[BPM:DIAG] Proton autodiscovered:", (diag as any).proton_autodiscovered);
    console.log("[BPM:DIAG] Session:", (diag as any).session_type, "gamescope:", (diag as any).gamescope_detected);
    console.log("[BPM:DIAG] Env:", { display: (diag as any).env_display, wayland: (diag as any).env_wayland, gamescope: (diag as any).env_gamescope, xdg: (diag as any).env_xdg_runtime });
    console.log("[BPM:DIAG] Installed games:", (diag as any).installed_games);
    console.log("[BPM:DIAG] === END DIAGNOSTICS ===");
  } catch (e) {
    console.warn("[BPM:DIAG] Diagnostics not available:", e);
  }
}
const showOptions = ref(false);
const optionsFocusIdx = ref(0);
let optionsLockId = 0;

const focusNav = useFocusNavigation();
const registerAction = useBpFocusableGroup("content");
const registerTab = useBpFocusableGroup("content");

const gamepad = useGamepad();
const _unsubs: (() => void)[] = [];

// ── Markdown rendering (H fix) ──────────────────────────────────────────
const renderedDescription = computed(() => {
  if (!game.value?.mDescription) return "";
  return renderMarkdown(game.value.mDescription);
});

/**
 * Lightweight markdown → HTML renderer for game descriptions.
 * Handles: headings, bold, italic, links, lists, paragraphs.
 */
function renderMarkdown(md: string): string {
  let html = md
    // Escape HTML entities
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    // Headings (## heading)
    .replace(/^### (.+)$/gm, "<h3>$1</h3>")
    .replace(/^## (.+)$/gm, "<h2>$1</h2>")
    .replace(/^# (.+)$/gm, "<h1>$1</h1>")
    // Bold (**text** or __text__)
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/__(.+?)__/g, "<strong>$1</strong>")
    // Italic (*text* or _text_)
    .replace(/\*(.+?)\*/g, "<em>$1</em>")
    .replace(/_(.+?)_/g, "<em>$1</em>")
    // Images ![alt](url) — must come before links to avoid ![...] matching [...]
    .replace(
      /!\[([^\]]*)\]\(([^)]+)\)/g,
      (_m: string, alt: string, url: string) => {
        // Rewrite relative URLs (starting with /) through the server proxy
        const src = url.startsWith("/") ? serverUrl(url.slice(1)) : url;
        return `<img src="${src}" alt="${alt}" class="rounded-lg max-w-full my-2" loading="lazy" />`;
      },
    )
    // Links [text](url)
    .replace(
      /\[([^\]]+)\]\(([^)]+)\)/g,
      '<a href="$2" target="_blank" rel="noopener" class="text-blue-400 hover:underline">$1</a>',
    )
    // Unordered lists (- item or * item)
    .replace(/^[\-\*] (.+)$/gm, "<li>$1</li>")
    // Horizontal rules (--- or ***)
    .replace(/^[\-\*]{3,}$/gm, '<hr class="border-zinc-700 my-4">')
    // Paragraphs (double newlines)
    .replace(/\n\n+/g, "</p><p>")
    // Single newlines → line breaks
    .replace(/\n/g, "<br>");

  // Wrap list items in <ul>
  html = html.replace(
    /(<li>.*?<\/li>)+/gs,
    '<ul class="list-disc pl-6 space-y-1">$&</ul>',
  );

  return `<p>${html}</p>`;
}

// ── Game type detection ─────────────────────────────────────────────────
const isEmulatedGame = computed(() => {
  const ver = version.value;
  return ver?.launches?.some((l) => l.emulator != null) ?? false;
});
const isNativeGame = computed(() => !isEmulatedGame.value);
const isWindowsGame = computed(() => {
  // Check launch configs from the loaded version data first
  const ver = version.value;
  if (ver?.launches?.some((l) => l.platform?.toLowerCase() === "windows")) {
    return true;
  }
  // Fallback to version options (loaded async for install/launch UI)
  return versionOptions.value?.some((v) => v.platform?.toLowerCase() === "windows") ?? false;
});

// ── Controller & Quality presets ─────────────────────────────────────────
const controllerOptions: { label: string; value: ControllerType | null }[] = [
  { label: "Auto", value: null },
  { label: "Xbox", value: "Xbox" },
  { label: "Nintendo", value: "Nintendo" },
];
const qualityOptions: { label: string; value: QualityPreset | null }[] = [
  { label: "Auto", value: null },
  { label: "Low", value: "Low" },
  { label: "Med", value: "Medium" },
  { label: "High", value: "High" },
  { label: "Ultra", value: "Ultra" },
];

const selectedController = ref<ControllerType | null>(null);
const selectedQuality = ref<QualityPreset | null>(null);
const aspectRatio = ref<AspectRatio>("Standard");
const crtShaderEnabled = ref(false);

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
      widescreen: aspectRatio.value,
      crtShader: crtShaderEnabled.value,
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
  const match = controllerOptions.find((o) => o.value === selectedController.value);
  return match?.label ?? "Auto";
});

const qualityLabel = computed(() => {
  const match = qualityOptions.find((o) => o.value === selectedQuality.value);
  return match?.label ?? "Auto";
});

const ASPECT_CYCLE: AspectRatio[] = ["Standard", "Wide16_9", "Wide16_10"];
const aspectLabel = computed(() => {
  switch (aspectRatio.value) {
    case "Wide16_9": return "16:9";
    case "Wide16_10": return "16:10";
    default: return "4:3";
  }
});

function toggleWidescreen() {
  const idx = ASPECT_CYCLE.indexOf(aspectRatio.value);
  aspectRatio.value = ASPECT_CYCLE[(idx + 1) % ASPECT_CYCLE.length];
  saveUserConfig();
}

function toggleCrtShader() {
  crtShaderEnabled.value = !crtShaderEnabled.value;
  saveUserConfig();
}

async function applyProfileName() {
  showOptions.value = false;
  try {
    const msg = await invoke<string>("configure_game_emulator", { gameId });
    console.log("[EMU]", msg);
  } catch (e) {
    console.error("[EMU] Failed to apply profile:", e);
  }
}

// ── Options menu: gamepad-navigable list ──────────────────────────────────
interface OptionsMenuItem {
  id: string;
  label: string;
  valueLabel?: string;
  action: () => void;
}

const optionsMenuItems = computed<OptionsMenuItem[]>(() => {
  const items: OptionsMenuItem[] = [];

  if (isEmulatedGame.value) {
    items.push({
      id: "controller",
      label: "Controller Layout",
      valueLabel: controllerLabel.value,
      action: cycleController,
    });
    items.push({
      id: "quality",
      label: "Quality Preset",
      valueLabel: qualityLabel.value,
      action: cycleQuality,
    });
    items.push({
      id: "widescreen",
      label: "Aspect Ratio",
      valueLabel: aspectLabel.value,
      action: toggleWidescreen,
    });
    items.push({
      id: "crt-shader",
      label: "CRT Shader",
      valueLabel: crtShaderEnabled.value ? "On" : "Off",
      action: toggleCrtShader,
    });
  }

  if (isNativeGame.value && isWindowsGame.value) {
    items.push({
      id: "profile",
      label: "Set Account Name",
      action: applyProfileName,
    });
  }

  items.push({
    id: "updates",
    label: "Check for Updates",
    action: () => {
      showOptions.value = false;
      checkForUpdates();
    },
  });

  items.push({
    id: "remove-library",
    label: "Remove from Library",
    action: removeFromLibrary,
  });

  if (status.value?.type === "Installed") {
    items.push({
      id: "uninstall",
      label: "Uninstall",
      action: uninstallGame,
    });
  }

  return items;
});

const _optionsSubs: (() => void)[] = [];

function wireOptionsGamepad() {
  unwireOptionsGamepad();

  _optionsSubs.push(
    gamepad.onButton(GamepadButton.DPadUp, () => {
      if (!showOptions.value) return;
      optionsFocusIdx.value = Math.max(0, optionsFocusIdx.value - 1);
    }),
  );
  _optionsSubs.push(
    gamepad.onButton(GamepadButton.DPadDown, () => {
      if (!showOptions.value) return;
      optionsFocusIdx.value = Math.min(
        optionsMenuItems.value.length - 1,
        optionsFocusIdx.value + 1,
      );
    }),
  );
  _optionsSubs.push(
    gamepad.onButton(GamepadButton.South, () => {
      if (!showOptions.value) return;
      const item = optionsMenuItems.value[optionsFocusIdx.value];
      if (item) item.action();
    }),
  );
  _optionsSubs.push(
    gamepad.onButton(GamepadButton.East, () => {
      if (!showOptions.value) return;
      showOptions.value = false;
    }),
  );
}

function unwireOptionsGamepad() {
  for (const unsub of _optionsSubs) unsub();
  _optionsSubs.length = 0;
}

watch(showOptions, (v) => {
  if (v) {
    optionsFocusIdx.value = 0;
    optionsLockId = focusNav.acquireInputLock();
    wireOptionsGamepad();
  } else {
    unwireOptionsGamepad();
    focusNav.releaseInputLock(optionsLockId);
  }
});

const confirmUninstall = ref(false);

function uninstallGame() {
  showOptions.value = false;
  confirmUninstall.value = true;
}

async function doUninstall() {
  confirmUninstall.value = false;
  try {
    await invoke("uninstall_game", { gameId });

    // The Tauri uninstall runs in a background thread: it deletes local
    // files, sets the game status to "Remote", and emits "update_library".
    // We do NOT remove the game from the server-side collection — the game
    // should remain in the user's library as "not installed" after uninstall.
    // Give the background thread a moment to update the local DB before
    // navigating, otherwise the library page may show stale state.
    await new Promise((resolve) => setTimeout(resolve, 500));

    navigateTo("/bigpicture/library");
  } catch (e) {
    console.error("[BPM:GAME] Uninstall failed:", e);
    launchError.value = `Uninstall failed: ${e instanceof Error ? e.message : String(e)}`;
  }
}

// ── Remove from library ─────────────────────────────────────────────────

const confirmRemoveFromLibrary = ref(false);

function removeFromLibrary() {
  showOptions.value = false;
  confirmRemoveFromLibrary.value = true;
}

async function doRemoveFromLibrary() {
  confirmRemoveFromLibrary.value = false;
  try {
    await fetch(serverUrl("api/v1/client/collection/default/entry"), {
      method: "DELETE",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ id: gameId }),
    });
    navigateTo("/bigpicture/library");
  } catch (e) {
    console.error("[BPM:GAME] Remove from library failed:", e);
    launchError.value = `Failed to remove: ${e instanceof Error ? e.message : String(e)}`;
  }
}

/** When an achievement icon fails to load, swap it for the trophy fallback. */
function onAchievementIconError(event: Event) {
  const img = event.target as HTMLImageElement;
  console.warn("[BPM:GAME] Achievement icon failed to load:", img.src);
  // Hide broken image, show a trophy-colored placeholder
  img.style.display = "none";
  // Insert a fallback element after the broken img
  const fallback = document.createElement("div");
  fallback.className = "size-12 rounded-lg bg-zinc-800 flex items-center justify-center";
  fallback.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="size-6 text-zinc-600"><path fill-rule="evenodd" d="M5.166 2.621v.858c-1.035.148-2.059.33-3.071.543a.75.75 0 0 0-.584.859 6.753 6.753 0 0 0 6.138 5.6 6.73 6.73 0 0 0 2.743 1.346A6.707 6.707 0 0 1 9.279 15H8.54c-1.036 0-1.875.84-1.875 1.875V19.5h-.75a2.25 2.25 0 0 0-2.25 2.25c0 .414.336.75.75.75h15.19a.75.75 0 0 0 .75-.75 2.25 2.25 0 0 0-2.25-2.25h-.75v-2.625c0-1.036-.84-1.875-1.875-1.875h-.739a6.707 6.707 0 0 1-1.112-3.173 6.73 6.73 0 0 0 2.743-1.347 6.753 6.753 0 0 0 6.139-5.6.75.75 0 0 0-.585-.858 47.077 47.077 0 0 0-3.07-.543V2.62a.75.75 0 0 0-.658-.744 49.22 49.22 0 0 0-6.093-.377c-2.063 0-4.096.128-6.093.377a.75.75 0 0 0-.657.744Z" clip-rule="evenodd" /></svg>`;
  img.parentNode?.insertBefore(fallback, img.nextSibling);
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
  rarity?: number;
  unlockCount?: number;
}

const achievements: Ref<AchievementItem[]> = ref([]);

function registerTabRef(value: string, el: any) {
  if (el) {
    tabRefs[value] = el;
    // Do NOT call updateTabIndicator() here — this runs inside a :ref
    // callback during render. Modifying reactive state (tabIndicatorStyle)
    // during render causes an infinite update loop.
  }
}

function updateTabIndicator() {
  const activeEl = tabRefs[activeTab.value];
  if (activeEl) {
    tabIndicatorStyle.value = {
      left: `${activeEl.offsetLeft}px`,
      width: `${activeEl.offsetWidth}px`,
    };
  }
}

watch(activeTab, () => {
  nextTick(() => updateTabIndicator());
});

// ── Achievement rarity display ──────────────────────────────────────────
function rarityColor(rarity: number): string {
  if (rarity < 5) return "bg-yellow-500";      // Ultra Rare - gold
  if (rarity < 20) return "bg-purple-500";     // Rare
  if (rarity < 50) return "bg-blue-500";       // Uncommon
  return "bg-zinc-500";                         // Common
}

function rarityTextColor(rarity: number): string {
  if (rarity < 5) return "text-yellow-400";
  if (rarity < 20) return "text-purple-400";
  if (rarity < 50) return "text-blue-400";
  return "text-zinc-500";
}

const unlockedCount = computed(() => achievements.value.filter(a => a.unlocked).length);
const achievementPercent = computed(() => achievements.value.length > 0 ? (unlockedCount.value / achievements.value.length) * 100 : 0);

// Helper: race a promise against a timeout
function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T | null> {
  return Promise.race([
    promise,
    new Promise<null>((resolve) => setTimeout(() => resolve(null), ms)),
  ]);
}

onMounted(async () => {
  console.log(`[BPM:GAME] === Page mounted for gameId: ${gameId} ===`);
  console.log(`[BPM:GAME] Route: ${route.fullPath}`);

  // Wire up gamepad immediately — don't wait for data to load
  _unsubs.push(
    gamepad.onButton(GamepadButton.Start, () => {
      showOptions.value = true;
    }),
  );

  // Listen for external launch errors (process crashes / wrong binary format)
  const { listen } = await import("@tauri-apps/api/event");
  const unlistenLaunchTrace = await listen("launch_trace", (event) => {
    const p = event.payload as any;
    console.log(`[BPM:TRACE:${p.step}]`, JSON.stringify(p, null, 2));
    // Surface BIOS warnings to the user so they know why a game crashed
    if (p.step === "7_retroarch_config_result" && p.bios_warnings?.length) {
      launchError.value = p.bios_warnings.join("\n");
    }
  });
  _unsubs.push(() => unlistenLaunchTrace());

  const unlistenLaunchError = await listen("launch_external_error", (event) => {
    if (event.payload === gameId) {
      console.error("[BPM:GAME] External launch error for:", gameId);
      launchError.value = "The game may have failed to launch. Check the game's compatibility — Windows games require Proton/UMU on Linux.";
      runDiagnostics();
    }
  });
  _unsubs.push(() => unlistenLaunchError());

  console.log("[BPM:GAME] Gamepad wired. Starting data fetch...");

  // Fire all fetches in parallel — apply results as each resolves instead
  // of waiting for all (avoids a slow fetch blocking the entire page).

  const achievementsUrl = serverUrl(`api/v1/games/${gameId}/achievements`);
  console.log("[BPM:GAME] Achievements URL:", achievementsUrl);

  // Game data — needed for the page header, status, and config
  // useGame is a local Tauri invoke (usually cached) — 5s is generous
  const gamePromise = withTimeout(useGame(gameId), 5000)
    .then((r) => {
      if (!r) { console.warn("[BPM:GAME] useGame TIMED OUT or null"); return; }
      console.log("[BPM:GAME] useGame resolved:", r.game?.mName ?? "null");
      game.value = r.game;
      statusRef.value = r.status;
      version.value = r.version?.value ?? null;
      console.log("[BPM:GAME] Game loaded:", r.game.mName, "| Status:", r.status?.value);
      if (version.value?.userConfiguration) {
        selectedController.value = version.value.userConfiguration.controllerType ?? null;
        selectedQuality.value = version.value.userConfiguration.qualityPreset ?? null;
        const ws = version.value.userConfiguration.widescreen;
        if (ws === true) aspectRatio.value = "Wide16_9";
        else if (ws === false || ws == null) aspectRatio.value = "Standard";
        else aspectRatio.value = ws as AspectRatio;
        crtShaderEnabled.value = version.value.userConfiguration.crtShader ?? false;
      }
    })
    .catch((e) => console.error("[BPM:GAME] useGame FAILED:", e));

  // Version options — can arrive late without blocking the page
  const versionPromise = invoke<VersionOption[]>("fetch_game_version_options", { gameId })
    .then((r) => {
      console.log("[BPM:GAME] version_options resolved:", r?.length ?? 0, "options");
      if (r) versionOptions.value = r;
    })
    .catch((e) => console.warn("[BPM:GAME] version_options failed:", e));

  // Achievements — server:// proxied fetch, 5s timeout
  const achievementsPromise = withTimeout(
    fetch(achievementsUrl).then((res) => {
      console.log("[BPM:GAME] achievements fetch status:", res.status);
      return res.ok ? res.json() : null;
    }),
    5000,
  )
    .then((r) => {
      if (!r) { console.warn("[BPM:GAME] achievements timed out or null"); return; }
      achievements.value = Array.isArray(r) ? r : (r.achievements ?? []);
      console.log("[BPM:GAME] Achievements loaded:", achievements.value.length);
      if (achievements.value.length > 0) {
        const sample = achievements.value.slice(0, 3);
        for (const a of sample) {
          console.log(`[BPM:GAME] Achievement "${a.title}" iconUrl: ${a.iconUrl || "(empty)"}`);
        }
      }
    })
    .catch((e) => console.warn("[BPM:GAME] achievements fetch FAILED:", e));

  // Wait for the critical data (game + achievements) before setting up focus.
  // version_options is intentionally NOT awaited — it can trickle in.
  await Promise.all([gamePromise, achievementsPromise]);
  console.log("[BPM:GAME] Critical data loaded, versions pending:", !versionOptions.value);

  console.log("[BPM:GAME] Setting up focus...");
  nextTick(() => updateTabIndicator());
  focusNav.autoFocusContent("content");
  console.log("[BPM:GAME] === Page setup complete ===");
});

function _onResize() {
  updateTabIndicator();
}
onMounted(() => {
  window.addEventListener("resize", _onResize);
});

onUnmounted(() => {
  for (const unsub of _unsubs) unsub();
  _unsubs.length = 0;
  unwireOptionsGamepad();
  if (showOptions.value) focusNav.releaseInputLock(optionsLockId);
  window.removeEventListener("resize", _onResize);
});

// Guard against re-triggering launch immediately after dismissing error dialog.
// When the user presses A to dismiss the error dialog, the same A press can
// propagate to the Play button underneath, causing an immediate re-launch loop.
let launchGuard = false;

function dismissLaunchError() {
  launchError.value = null;
  launchGuard = true;
  setTimeout(() => { launchGuard = false; }, 300);
}

async function launchGame() {
  if (launchGuard) return;
  try {
    const result: LaunchResult = await invoke("launch_game", {
      id: gameId,
      index: 0,
    });
    if (result.result === "InstallRequired") {
      // Auto-download the required dependency (e.g. runtime/tool)
      const [depGameId, depVersionId] = result.data;
      try {
        const installDirs = await invoke<string[]>("fetch_download_dir_stats");
        await invoke("download_game", {
          gameId: depGameId,
          versionId: depVersionId,
          installDir: 0,
          targetPlatform: versionOptions.value?.[0]?.platform ?? "linux",
          enableUpdates: true,
        });
        launchError.value = `A required dependency is being installed. Please try launching again once the download completes.`;
      } catch (depErr) {
        launchError.value = `A required dependency needs to be installed first, but the download failed: ${depErr instanceof Error ? depErr.message : String(depErr)}`;
      }
    } else if (result.result !== "Success") {
      launchError.value = `Failed to launch: ${result.result}`;
    }
  } catch (e) {
    const errMsg = e instanceof Error ? e.message : String(e);
    console.error("[BPM:GAME] Launch error:", errMsg);
    // Auto-run diagnostics on any launch failure for debug logs
    runDiagnostics();
    // Provide user-friendly hints for common errors
    if (errMsg.includes("exec format error") || errMsg.includes("os error 8")) {
      launchError.value = "This game appears to be a Windows executable that can't run natively on Linux. Check that Proton is configured in Settings and the game's platform is set correctly.";
    } else if (errMsg.includes("NoCompat") || errMsg.includes("compatibility layer")) {
      launchError.value = "No Proton compatibility layer found. Set a default Proton path in Settings or add an override for this game.";
    } else if (errMsg.includes("InvalidPlatform")) {
      launchError.value = "This game can't be played on the current platform. It may need a compatibility layer like Proton.";
    } else {
      launchError.value = `Launch error: ${errMsg}`;
    }
  }
}

async function killGame() {
  try {
    await invoke("kill_game", { id: gameId });
  } catch (e) {
    console.error("Failed to stop game:", e);
  }
}

/**
 * Download/install the game.
 * Fetches version options to find the best version, then starts the download.
 */
async function downloadGame() {
  try {
    // Need version options to know what to download
    if (!versionOptions.value || versionOptions.value.length === 0) {
      versionOptions.value = await invoke<VersionOption[]>(
        "fetch_game_version_options",
        { gameId },
      );
    }

    if (!versionOptions.value || versionOptions.value.length === 0) {
      launchError.value = "No downloadable versions available for this game.";
      return;
    }

    // Pick the first (latest) version option
    const vo = versionOptions.value[0];

    // Get available install directories
    const installDirs = await invoke<string[]>("fetch_download_dir_stats");
    const installDir = 0; // Default to first directory

    await invoke("download_game", {
      gameId,
      versionId: vo.versionId,
      installDir,
      targetPlatform: vo.platform,
      enableUpdates: true,
    });
  } catch (e) {
    console.error("Failed to start download:", e);
    launchError.value = `Download failed: ${e instanceof Error ? e.message : String(e)}`;
  }
}

// ── Add to Library (without installing) ─────────────────────────────────

const inLibrary = ref(false);
const libraryLoading = ref(false);

// Check if this game is already in the user's library on mount
onMounted(async () => {
  try {
    const url = serverUrl("api/v1/collection/default");
    const res = await fetch(url);
    if (res.ok) {
      const collection = await res.json();
      const entries = collection.entries ?? [];
      inLibrary.value = entries.some((e: any) => e.gameId === gameId);
    }
  } catch (e) {
    console.warn("[BPM:GAME] Failed to check library status:", e);
  }
});

async function addToLibrary() {
  if (libraryLoading.value || inLibrary.value) return;
  libraryLoading.value = true;
  try {
    const url = serverUrl("api/v1/collection/default/entry");
    const res = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ id: gameId }),
    });
    if (res.ok) {
      inLibrary.value = true;
    } else {
      console.error("[BPM:GAME] Failed to add to library:", res.status);
    }
  } catch (e) {
    console.error("[BPM:GAME] Add to library error:", e);
  } finally {
    libraryLoading.value = false;
  }
}

function openStore() {
  navigateTo(`/store/${gameId}`);
}

// ── Add to Steam ────────────────────────────────────────────────────────

const addedToSteam = ref(false);
const steamLoading = ref(false);

async function addToSteam() {
  if (steamLoading.value || addedToSteam.value) return;
  steamLoading.value = true;
  try {
    const g = game.value;
    const result = await invoke<{ success: boolean; message: string }>(
      "add_game_to_steam",
      {
        gameId,
        gameName: g?.mName ?? "Unknown Game",
        bannerObjectId: g?.mBannerObjectId || null,
        coverObjectId: g?.mCoverObjectId || null,
        iconObjectId: g?.mIconObjectId || null,
      },
    );
    console.log("[BPM:GAME] Add to Steam result:", result);
    if (result.success) {
      addedToSteam.value = true;
    } else {
      launchError.value = result.message;
    }
  } catch (e) {
    console.error("[BPM:GAME] Add to Steam failed:", e);
    launchError.value = `Failed to add to Steam: ${e instanceof Error ? e.message : String(e)}`;
  } finally {
    steamLoading.value = false;
  }
}

async function checkForUpdates() {
  try {
    await invoke("check_for_updates", { gameId });
  } catch (e) {
    console.error("Failed to check for updates:", e);
  }
}
</script>
