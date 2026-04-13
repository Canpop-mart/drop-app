<template>
  <div class="flex h-full">
    <!-- Settings sidebar -->
    <div class="w-64 bg-zinc-950/50 border-r border-zinc-800/30 py-6">
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

    <!-- Settings content -->
    <div class="flex-1 overflow-y-auto px-8 py-6">
      <!-- ═══════ Interface ═══════ -->
      <div
        v-if="activeSection === 'interface'"
        class="space-y-5 max-w-xl"
      >
        <h3 class="text-lg font-semibold text-zinc-200 font-display">
          Interface
        </h3>

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
              Launch Drop in fullscreen Big Picture mode
            </p>
          </div>
          <button
            class="w-12 h-7 rounded-full transition-colors relative shrink-0 ml-4"
            :class="startInBP ? 'bg-blue-600' : 'bg-zinc-700'"
            @click.stop="toggleStartInBP()"
          >
            <div
              class="absolute top-0.5 size-6 rounded-full bg-white shadow transition-transform"
              :class="startInBP ? 'translate-x-5' : 'translate-x-0.5'"
            />
          </button>
        </div>
      </div>

      <!-- ═══════ Performance ═══════ -->
      <div
        v-if="activeSection === 'performance'"
        class="space-y-5 max-w-xl"
      >
        <h3 class="text-lg font-semibold text-zinc-200 font-display">
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
        class="space-y-5 max-w-xl"
      >
        <h3 class="text-lg font-semibold text-zinc-200 font-display">
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
        class="space-y-5 max-w-xl"
      >
        <h3 class="text-lg font-semibold text-zinc-200 font-display">
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
        class="space-y-5 max-w-xl"
      >
        <h3 class="text-lg font-semibold text-zinc-200 font-display">
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

      <!-- ═══════ Compatibility ═══════ -->
      <div
        v-if="activeSection === 'compatibility'"
        class="space-y-5 max-w-xl"
      >
        <h3 class="text-lg font-semibold text-zinc-200 font-display">
          Compatibility
        </h3>

        <!-- Proton / UMU status -->
        <div class="bg-zinc-900/50 rounded-xl p-4">
          <div class="mb-3">
            <p class="font-medium text-zinc-200 text-sm">
              Proton (Windows Game Support)
            </p>
            <p class="text-zinc-500 text-xs mt-0.5">
              Select a Proton installation to run Windows games on Linux.
            </p>
          </div>

          <div v-if="protonLoading" class="text-zinc-500 text-xs py-2">
            Discovering Proton installations...
          </div>

          <div v-else-if="allProtonPaths.length === 0" class="py-2">
            <p class="text-yellow-400/80 text-xs">
              No Proton installations found. Install Proton via Steam
              (Settings → Compatibility → Enable Steam Play) or place it in
              <code class="bg-zinc-800 px-1 rounded">~/.steam/root/compatibilitytools.d/</code>.
            </p>
          </div>

          <div v-else class="space-y-2">
            <button
              v-for="proton in allProtonPaths"
              :key="proton.path"
              :ref="(el: any) => registerContent(el, { onSelect: () => setDefaultProton(proton.path) })"
              class="w-full flex items-center justify-between bg-zinc-800/50 rounded-lg px-3 py-2.5 text-left transition-colors"
              :class="[
                proton.path === selectedProtonDefault
                  ? 'ring-1 ring-blue-500/50 bg-blue-600/10'
                  : 'hover:bg-zinc-700/50',
              ]"
              @click="setDefaultProton(proton.path)"
            >
              <div class="min-w-0">
                <span class="text-xs font-medium text-zinc-200 block truncate">{{ proton.name }}</span>
                <span class="text-[10px] text-zinc-500 block truncate">{{ proton.path }}</span>
              </div>
              <div
                v-if="proton.path === selectedProtonDefault"
                class="size-2 rounded-full bg-blue-500 shrink-0 ml-2"
              />
            </button>
          </div>

          <p v-if="protonSaveError" class="text-red-400/80 text-xs mt-2">
            {{ protonSaveError }}
          </p>
        </div>
      </div>

      <!-- ═══════ About ═══════ -->
      <div
        v-if="activeSection === 'about'"
        class="space-y-5 max-w-xl"
      >
        <h3 class="text-lg font-semibold text-zinc-200 font-display">
          About Drop
        </h3>
        <div class="bg-zinc-900/50 rounded-xl p-4 space-y-2">
          <p class="text-zinc-300 text-sm">
            Drop is an open-source, self-hosted game distribution platform.
          </p>
          <p class="text-zinc-500 text-xs">Big Picture Mode v1.0</p>
          <div class="border-t border-zinc-800/50 pt-2 mt-2 space-y-1">
            <p v-if="packageFormat" class="text-zinc-500 text-xs">
              Package: {{ packageFormatLabel }}
            </p>
          </div>
        </div>

        <!-- Bug Report -->
        <div
          :ref="(el: any) => registerContent(el, { onSelect: () => navigateTo('/bigpicture/bugreport') })"
          class="flex items-center justify-between bg-zinc-900/50 rounded-xl cursor-pointer p-4 hover:bg-zinc-800/50 transition-colors"
          @click="navigateTo('/bigpicture/bugreport')"
        >
          <div>
            <p class="font-medium text-zinc-200 text-sm">Submit Bug Report</p>
            <p class="text-zinc-500 text-xs mt-0.5">
              Report an issue with system diagnostics and logs attached automatically
            </p>
          </div>
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="size-5 text-zinc-500">
            <path fill-rule="evenodd" d="M8.22 5.22a.75.75 0 0 1 1.06 0l4.25 4.25a.75.75 0 0 1 0 1.06l-4.25 4.25a.75.75 0 0 1-1.06-1.06L11.94 10 8.22 6.28a.75.75 0 0 1 0-1.06Z" clip-rule="evenodd" />
          </svg>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useGamepad } from "~/composables/gamepad";
import { useBpFocusableGroup } from "~/composables/bp-focusable";

definePageMeta({ layout: "bigpicture" });

const gamepad = useGamepad();
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
  { label: "Compatibility", value: "compatibility" },
  { label: "About", value: "about" },
];

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

// ── Proton / Compatibility ─────────────────────────────────────────────────

interface ProtonPath {
  path: string;
  name: string;
}

const protonLoading = ref(true);
const allProtonPaths = ref<ProtonPath[]>([]);
const selectedProtonDefault = ref<string | null>(null);
const protonSaveError = ref<string | null>(null);

onMounted(async () => {
  try {
    const result = await invoke<{
      autodiscovered: ProtonPath[];
      custom: ProtonPath[];
      default: string | null;
    }>("fetch_proton_paths");

    allProtonPaths.value = [...result.autodiscovered, ...result.custom];
    selectedProtonDefault.value = result.default;

    // Auto-set default if one is discovered but none is selected
    if (!result.default && allProtonPaths.value.length > 0) {
      const firstPath = allProtonPaths.value[0].path;
      try {
        await invoke("set_default", { path: firstPath });
        selectedProtonDefault.value = firstPath;
        console.log("[BPM:SETTINGS] Auto-selected Proton default:", firstPath);
      } catch (e) {
        console.warn("[BPM:SETTINGS] Failed to auto-set Proton default:", e);
      }
    }
  } catch (e) {
    console.warn("[BPM:SETTINGS] Proton discovery failed:", e);
  } finally {
    protonLoading.value = false;
  }
});

async function setDefaultProton(path: string) {
  protonSaveError.value = null;
  try {
    await invoke("set_default", { path });
    selectedProtonDefault.value = path;
  } catch (e) {
    protonSaveError.value = `Failed to set default: ${e instanceof Error ? e.message : String(e)}`;
  }
}
</script>
