<template>
  <div class="flex h-full" :class="deck.isDeckMode.value ? 'flex-col' : ''">
    <!-- Desktop: Settings sidebar -->
    <div
      v-if="!deck.isDeckMode.value"
      class="w-64 bg-zinc-950/50 border-r border-zinc-800/30 py-6"
    >
      <button
        v-for="section in sections"
        :key="section.value"
        :ref="
          (el: any) =>
            registerSidebar(el, {
              onSelect: () => (activeSection = section.value),
            })
        "
        class="w-full px-6 py-3 text-left text-sm font-medium transition-colors"
        :class="[
          activeSection === section.value
            ? 'text-blue-400 bg-blue-600/10 border-r-2 border-blue-500'
            : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/30',
        ]"
        @click="activeSection = section.value"
      >
        {{ section.label }}
      </button>
    </div>

    <!-- Deck: horizontal tab strip -->
    <div
      v-if="deck.isDeckMode.value"
      class="flex gap-2 px-5 pt-3 pb-2 border-b border-zinc-800/30 overflow-x-auto"
    >
      <button
        v-for="section in sections"
        :key="section.value"
        :ref="
          (el: any) =>
            registerSidebar(el, {
              onSelect: () => (activeSection = section.value),
            })
        "
        class="px-4 py-2 rounded-lg text-sm font-medium transition-colors whitespace-nowrap"
        :class="[
          activeSection === section.value
            ? 'bg-blue-600/20 text-blue-400'
            : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50',
        ]"
        @click="activeSection = section.value"
      >
        {{ section.label }}
      </button>
    </div>

    <!-- Settings content -->
    <div
      class="flex-1 overflow-y-auto"
      :class="deck.isDeckMode.value ? 'px-5 py-4' : 'px-8 py-6'"
    >
      <!-- ═══════ Interface ═══════ -->
      <div
        v-if="activeSection === 'interface'"
        class="space-y-5"
        :class="deck.isDeckMode.value ? '' : 'max-w-xl'"
      >
        <h3
          class="font-semibold text-zinc-200 font-display"
          :class="deck.isDeckMode.value ? 'text-base' : 'text-lg'"
        >
          Interface
        </h3>

        <!-- Gamescope / Steam Deck detection banner -->
        <div
          v-if="deck.isGamescope.value || deck.isSteamDeckHardware.value"
          class="bg-emerald-900/20 border border-emerald-700/30 rounded-xl p-4"
        >
          <div class="flex items-center gap-2 mb-1">
            <div class="size-2 rounded-full bg-emerald-500 animate-pulse" />
            <p class="font-medium text-emerald-300 text-sm">
              {{
                deck.isGamescope.value
                  ? "Steam Deck Game Mode"
                  : "Steam Deck Detected"
              }}
            </p>
          </div>
          <p class="text-emerald-400/60 text-xs">
            <template v-if="deck.isGamescope.value">
              Running in Gamescope session. Big Picture Mode is active and Deck
              layout is locked on.
            </template>
            <template v-else>
              Steam Deck hardware detected in Desktop Mode. Switch to Game Mode
              for the best experience.
            </template>
          </p>
        </div>

        <!-- Start in Big Picture toggle -->
        <div
          :ref="
            (el: any) => registerContent(el, { onSelect: () => toggleStartInBP() })
          "
          class="flex items-center justify-between bg-zinc-900/50 rounded-xl cursor-pointer p-4"
        >
          <div>
            <p class="font-medium text-zinc-200 text-sm">
              Start in Big Picture Mode
            </p>
            <p class="text-zinc-500 text-xs mt-0.5">
              {{
                deck.isGamescope.value
                  ? "Always on in Game Mode"
                  : "Launch Drop in fullscreen Big Picture mode"
              }}
            </p>
          </div>
          <button
            class="w-12 h-7 rounded-full transition-colors relative shrink-0 ml-4"
            :class="[
              startInBP || deck.isGamescope.value
                ? 'bg-blue-600'
                : 'bg-zinc-700',
              deck.isGamescope.value ? 'opacity-50 cursor-not-allowed' : '',
            ]"
            :disabled="deck.isGamescope.value"
            @click.stop="toggleStartInBP()"
          >
            <div
              class="absolute top-0.5 size-6 rounded-full bg-white shadow transition-transform"
              :class="
                startInBP || deck.isGamescope.value
                  ? 'translate-x-5'
                  : 'translate-x-0.5'
              "
            />
          </button>
        </div>

        <!-- Deck Mode selector -->
        <div class="bg-zinc-900/50 rounded-xl p-4">
          <div class="mb-3">
            <p class="font-medium text-zinc-200 text-sm">Deck Mode</p>
            <p class="text-zinc-500 text-xs mt-0.5">
              {{
                deck.isGamescope.value
                  ? "Locked to Deck layout in Game Mode"
                  : "Switch between desktop and handheld-optimised layout"
              }}
            </p>
          </div>
          <div class="flex gap-2">
            <button
              v-for="option in deckOptions"
              :key="option.value"
              :ref="
                (el: any) =>
                  registerContent(el, {
                    onSelect: () => setDeckMode(option.value),
                  })
              "
              class="flex-1 py-2.5 rounded-lg text-sm font-medium transition-all border"
              :class="[
                deck.isGamescope.value ? 'opacity-50 cursor-not-allowed' : '',
                deck.forceOverride.value === option.value
                  ? 'bg-blue-600/20 text-blue-400 border-blue-500/50'
                  : 'bg-zinc-800/50 text-zinc-400 border-zinc-700/50 hover:text-zinc-200 hover:bg-zinc-800',
              ]"
              :disabled="deck.isGamescope.value"
              @click="setDeckMode(option.value)"
            >
              {{ option.label }}
            </button>
          </div>
          <p class="text-zinc-600 text-xs mt-2">
            <template v-if="deck.isGamescope.value">
              Deck layout is automatic in Game Mode.
            </template>
            <template v-else>
              Auto detects Steam Deck and small screens. Currently:
              {{ deck.isDeckMode.value ? "Deck" : "Desktop" }}
              <span
                v-if="deck.forceOverride.value !== 'auto'"
                class="text-blue-400"
                >(forced)</span
              >
            </template>
          </p>
        </div>
      </div>

      <!-- ═══════ Performance ═══════ -->
      <div
        v-if="activeSection === 'performance'"
        class="space-y-5"
        :class="deck.isDeckMode.value ? '' : 'max-w-xl'"
      >
        <h3
          class="font-semibold text-zinc-200 font-display"
          :class="deck.isDeckMode.value ? 'text-base' : 'text-lg'"
        >
          Performance
        </h3>

        <!-- MangoHud overlay (Linux only) -->
        <div class="bg-zinc-900/50 rounded-xl p-4">
          <div class="mb-3">
            <p class="font-medium text-zinc-200 text-sm">Performance Overlay</p>
            <p class="text-zinc-500 text-xs mt-0.5">
              Show MangoHud FPS/performance overlay in games (Linux only)
            </p>
          </div>
          <div class="flex gap-2">
            <button
              v-for="option in mangohudOptions"
              :key="option.value"
              :ref="
                (el: any) =>
                  registerContent(el, {
                    onSelect: () => (mangohudPreset = option.value),
                  })
              "
              class="flex-1 py-2.5 rounded-lg text-xs font-medium transition-all border"
              :class="[
                mangohudPreset === option.value
                  ? 'bg-blue-600/20 text-blue-400 border-blue-500/50'
                  : 'bg-zinc-800/50 text-zinc-400 border-zinc-700/50 hover:text-zinc-200 hover:bg-zinc-800',
              ]"
              @click="mangohudPreset = option.value"
            >
              {{ option.label }}
            </button>
          </div>
          <p class="text-zinc-600 text-xs mt-2">
            Applies per-game via MANGOHUD environment variable. Requires
            MangoHud installed.
          </p>
        </div>
      </div>

      <!-- ═══════ Steam Integration ═══════ -->
      <div
        v-if="activeSection === 'steam'"
        class="space-y-5"
        :class="deck.isDeckMode.value ? '' : 'max-w-xl'"
      >
        <h3
          class="font-semibold text-zinc-200 font-display"
          :class="deck.isDeckMode.value ? 'text-base' : 'text-lg'"
        >
          Steam Integration
        </h3>

        <!-- Add to Steam Library -->
        <div class="bg-zinc-900/50 rounded-xl p-4">
          <div class="mb-3">
            <p class="font-medium text-zinc-200 text-sm">Add Drop to Steam</p>
            <p class="text-zinc-500 text-xs mt-0.5">
              Register Drop as a non-Steam game so it appears in SteamOS Game
              Mode
            </p>
          </div>
          <button
            :ref="(el: any) => registerContent(el, { onSelect: addToSteam })"
            class="px-4 py-2.5 rounded-lg text-sm font-medium transition-all border"
            :class="[
              steamShortcutResult?.success
                ? 'bg-emerald-600/20 text-emerald-400 border-emerald-500/50'
                : 'bg-blue-600/20 text-blue-400 border-blue-500/50 hover:bg-blue-600/30',
            ]"
            :disabled="steamShortcutLoading"
            @click="addToSteam"
          >
            {{
              steamShortcutLoading
                ? "Adding..."
                : steamShortcutResult?.success
                  ? "Added to Steam"
                  : "Add to Steam Library"
            }}
          </button>
          <p
            v-if="steamShortcutResult"
            class="text-xs mt-2"
            :class="
              steamShortcutResult.success
                ? 'text-emerald-400/60'
                : 'text-red-400/60'
            "
          >
            {{ steamShortcutResult.message }}
          </p>
        </div>
      </div>

      <!-- ═══════ Controller ═══════ -->
      <div
        v-if="activeSection === 'controller'"
        class="space-y-5"
        :class="deck.isDeckMode.value ? '' : 'max-w-xl'"
      >
        <h3
          class="font-semibold text-zinc-200 font-display"
          :class="deck.isDeckMode.value ? 'text-base' : 'text-lg'"
        >
          Controller
        </h3>
        <div class="bg-zinc-900/50 rounded-xl p-4">
          <div class="flex items-center gap-3 mb-3">
            <div
              class="size-3 rounded-full"
              :class="gamepad.connected.value ? 'bg-green-500' : 'bg-zinc-600'"
            />
            <p class="font-medium text-zinc-200 text-sm">
              {{
                gamepad.connected.value
                  ? gamepad.controllerName.value || "Controller Connected"
                  : "No Controller Detected"
              }}
            </p>
          </div>
          <p class="text-zinc-500 text-xs">
            Connect a controller to use Big Picture Mode with a gamepad.
          </p>
        </div>

        <!-- Haptic feedback toggle -->
        <div
          :ref="
            (el: any) =>
              registerContent(el, {
                onSelect: () => (hapticEnabled = !hapticEnabled),
              })
          "
          class="flex items-center justify-between bg-zinc-900/50 rounded-xl cursor-pointer p-4"
        >
          <div>
            <p class="font-medium text-zinc-200 text-sm">Haptic Feedback</p>
            <p class="text-zinc-500 text-xs mt-0.5">
              Vibration on navigation and selection
            </p>
          </div>
          <button
            class="w-12 h-7 rounded-full transition-colors relative shrink-0 ml-4"
            :class="hapticEnabled ? 'bg-blue-600' : 'bg-zinc-700'"
            @click.stop="hapticEnabled = !hapticEnabled"
          >
            <div
              class="absolute top-0.5 size-6 rounded-full bg-white shadow transition-transform"
              :class="hapticEnabled ? 'translate-x-5' : 'translate-x-0.5'"
            />
          </button>
        </div>
      </div>

      <!-- ═══════ Storage ═══════ -->
      <div
        v-if="activeSection === 'storage'"
        class="space-y-5"
        :class="deck.isDeckMode.value ? '' : 'max-w-xl'"
      >
        <h3
          class="font-semibold text-zinc-200 font-display"
          :class="deck.isDeckMode.value ? 'text-base' : 'text-lg'"
        >
          Storage
        </h3>

        <!-- Install directories -->
        <div class="bg-zinc-900/50 rounded-xl p-4">
          <div class="mb-3">
            <p class="font-medium text-zinc-200 text-sm">Install Directories</p>
            <p class="text-zinc-500 text-xs mt-0.5">
              Where games are installed on this device
            </p>
          </div>
          <div v-if="installDirs.length > 0" class="space-y-2 mb-3">
            <div
              v-for="(dir, idx) in installDirs"
              :key="idx"
              class="flex items-center justify-between bg-zinc-800/50 rounded-lg px-3 py-2"
            >
              <span class="text-xs text-zinc-300 truncate">{{ dir }}</span>
            </div>
          </div>
          <p v-else class="text-zinc-600 text-xs mb-3">
            No install directories configured.
          </p>
        </div>

        <!-- SD Card / Removable storage -->
        <div
          v-if="removableStorage.length > 0"
          class="bg-zinc-900/50 rounded-xl p-4"
        >
          <div class="mb-3">
            <p class="font-medium text-zinc-200 text-sm">Removable Storage</p>
            <p class="text-zinc-500 text-xs mt-0.5">
              SD cards and USB drives detected on this device
            </p>
          </div>
          <div class="space-y-2">
            <div
              v-for="(path, idx) in removableStorage"
              :key="idx"
              class="flex items-center justify-between bg-zinc-800/50 rounded-lg px-3 py-2"
            >
              <span class="text-xs text-zinc-300 truncate">{{ path }}</span>
              <button
                :ref="
                  (el: any) =>
                    registerContent(el, {
                      onSelect: () => addStorageAsInstallDir(path),
                    })
                "
                class="text-xs text-blue-400 hover:text-blue-300 ml-2 whitespace-nowrap"
                @click="addStorageAsInstallDir(path)"
              >
                Use for games
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- ═══════ About ═══════ -->
      <div
        v-if="activeSection === 'about'"
        class="space-y-5"
        :class="deck.isDeckMode.value ? '' : 'max-w-xl'"
      >
        <h3
          class="font-semibold text-zinc-200 font-display"
          :class="deck.isDeckMode.value ? 'text-base' : 'text-lg'"
        >
          About Drop
        </h3>
        <div class="bg-zinc-900/50 rounded-xl p-4 space-y-2">
          <p class="text-zinc-300 text-sm">
            Drop is an open-source, self-hosted game distribution platform.
          </p>
          <p class="text-zinc-500 text-xs">Big Picture Mode v1.0</p>
          <div class="border-t border-zinc-800/50 pt-2 mt-2 space-y-1">
            <p class="text-zinc-500 text-xs">Session: {{ sessionLabel }}</p>
            <p class="text-zinc-500 text-xs">
              Layout: {{ deck.isDeckMode.value ? "Deck" : "Desktop" }}
            </p>
            <p v-if="packageFormat" class="text-zinc-500 text-xs">
              Package: {{ packageFormatLabel }}
            </p>
            <p
              v-if="deck.isSteamDeckHardware.value"
              class="text-zinc-500 text-xs"
            >
              Hardware: Steam Deck
            </p>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useGamepad } from "~/composables/gamepad";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useDeckMode } from "~/composables/deck-mode";

definePageMeta({ layout: "bigpicture" });

const gamepad = useGamepad();
const deck = useDeckMode();
const activeSection = ref("interface");

// C7 fix: persist BPM settings in localStorage
const startInBP = ref(
  typeof localStorage !== "undefined"
    ? localStorage.getItem("drop:startInBPM") === "true"
    : false,
);
watch(startInBP, (val) => {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem("drop:startInBPM", String(val));
  }
});

function toggleStartInBP() {
  if (deck.isGamescope.value) return;
  startInBP.value = !startInBP.value;
}

const registerSidebar = useBpFocusableGroup("content");
const registerContent = useBpFocusableGroup("content");

const sections = [
  { label: "Interface", value: "interface" },
  { label: "Performance", value: "performance" },
  { label: "Steam", value: "steam" },
  { label: "Controller", value: "controller" },
  { label: "Storage", value: "storage" },
  { label: "About", value: "about" },
];

const deckOptions = [
  { label: "Auto", value: "auto" as const },
  { label: "Deck", value: "deck" as const },
  { label: "Desktop", value: "desktop" as const },
];

function setDeckMode(value: "auto" | "deck" | "desktop") {
  if (deck.isGamescope.value) return;
  deck.forceOverride.value = value;
}

const sessionLabel = computed(() => {
  switch (deck.sessionType.value) {
    case "gamescope":
      return "Gamescope (Game Mode)";
    case "steamDeckDesktop":
      return "Steam Deck (Desktop Mode)";
    default:
      return "Desktop";
  }
});

// ── MangoHud ────────────────────────────────────────────────────────────────

const mangohudOptions = [
  { label: "Off", value: "off" },
  { label: "FPS Only", value: "minimal" },
  { label: "Standard", value: "standard" },
  { label: "Full", value: "full" },
];

const mangohudPreset = ref("off");

// Load the current global MangoHud preset from backend settings
onMounted(async () => {
  try {
    const settings = await invoke<{ globalMangohud?: string }>(
      "fetch_settings",
    );
    if (settings.globalMangohud) {
      mangohudPreset.value = settings.globalMangohud;
    }
  } catch {
    // Settings not available — keep default
  }
});

watch(mangohudPreset, async (val) => {
  try {
    await invoke("update_settings", {
      newSettings: { globalMangohud: val === "off" ? null : val },
    });
  } catch (e) {
    console.warn("Failed to save MangoHud setting:", e);
  }
});

// ── Haptic feedback ─────────────────────────────────────────────────────────

const hapticEnabled = ref(
  typeof localStorage !== "undefined"
    ? localStorage.getItem("drop:haptic") !== "false"
    : true,
);
watch(hapticEnabled, (val) => {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem("drop:haptic", String(val));
  }
});

// ── Steam shortcut ──────────────────────────────────────────────────────────

const steamShortcutLoading = ref(false);
const steamShortcutResult = ref<{ success: boolean; message: string } | null>(
  null,
);

async function addToSteam() {
  if (steamShortcutLoading.value) return;
  steamShortcutLoading.value = true;
  try {
    steamShortcutResult.value = await invoke("register_steam_shortcut");
  } catch (e) {
    steamShortcutResult.value = {
      success: false,
      message: `Failed: ${e instanceof Error ? e.message : String(e)}`,
    };
  } finally {
    steamShortcutLoading.value = false;
  }
}

// ── Package format detection ────────────────────────────────────────────────

const packageFormat = ref<string | null>(null);
onMounted(async () => {
  try {
    packageFormat.value = await invoke("detect_package_format");
  } catch {
    // Command may not exist on all platforms
  }
});

const packageFormatLabel = computed(() => {
  switch (packageFormat.value) {
    case "flatpak": return "Flatpak";
    case "appImage": return "AppImage (self-updating)";
    case "systemPackage": return "System package (DEB/RPM)";
    case "windowsInstaller": return "Windows";
    case "macOsBundle": return "macOS";
    default: return packageFormat.value ?? "Unknown";
  }
});

// ── Storage / Install dirs ──────────────────────────────────────────────────

const installDirs = ref<string[]>([]);
const removableStorage = ref<string[]>([]);

onMounted(async () => {
  try {
    installDirs.value = await invoke("fetch_download_dir_stats");
  } catch { /* ignore */ }

  try {
    removableStorage.value = await invoke("detect_removable_storage");
  } catch { /* ignore on non-Linux */ }
});

async function addStorageAsInstallDir(path: string) {
  const gameDir = `${path}/drop-games`;
  try {
    await invoke("add_download_dir", { newDir: gameDir });
    installDirs.value = await invoke("fetch_download_dir_stats");
  } catch (e) {
    console.error("Failed to add storage dir:", e);
  }
}
</script>