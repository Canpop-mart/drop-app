<template>
  <div class="flex flex-col h-full overflow-y-auto">
    <!-- Loading state -->
    <div v-if="loading" class="flex-1 flex items-center justify-center">
      <div class="size-12 border-4 border-blue-500/30 border-t-blue-500 rounded-full animate-spin" />
    </div>

    <!-- Load failed. The editor is not shown at all: its fields would be empty,
         and Save would overwrite the real profile and showcase with blanks. -->
    <div v-else-if="loadFailed" class="flex-1 flex flex-col items-center justify-center gap-4 p-8 text-center">
      <p class="text-zinc-300">Could not load your profile.</p>
      <div class="flex gap-3">
        <button
          :ref="(el: any) => registerContent(el, { onSelect: retryLoad })"
          class="px-5 py-2.5 rounded-xl text-sm font-medium bg-blue-600 text-white hover:bg-blue-500 transition-colors"
          @click="retryLoad"
        >
          Try again
        </button>
        <button
          :ref="(el: any) => registerContent(el, { onSelect: goBack })"
          class="px-5 py-2.5 rounded-xl text-sm text-zinc-300 bg-zinc-800/50 hover:bg-zinc-700 transition-colors"
          @click="goBack"
        >
          Back
        </button>
      </div>
    </div>

    <template v-else>
      <!-- Banner preview -->
      <div class="relative shrink-0 h-48">
        <img
          v-if="bannerPreview || profile?.bannerObjectId"
          :src="bannerPreview || objectUrl(profile!.bannerObjectId!)"
          class="w-full h-full object-cover"
        />
        <div
          v-else
          class="w-full h-full"
          :style="{ background: `linear-gradient(135deg, ${themeColors.from}, ${themeColors.to})` }"
        />
        <div class="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/60 to-transparent" />

        <!-- Avatar + name overlay -->
        <div class="absolute bottom-0 left-0 right-0 p-6 flex items-end gap-5">
          <button
            :ref="(el: any) => registerContent(el, { onSelect: openAvatarPicker })"
            class="relative group shrink-0 rounded-full"
            @click="openAvatarPicker"
          >
            <img
              v-if="avatarPreview || profile?.profilePictureObjectId"
              :src="avatarPreview || objectUrl(profile!.profilePictureObjectId!)"
              class="size-24 rounded-full border-4 border-zinc-900 object-cover shadow-2xl"
            />
            <div
              v-else
              class="size-24 rounded-full bg-zinc-700 border-4 border-zinc-900 flex items-center justify-center"
            >
              <UserIcon class="size-10 text-zinc-500" />
            </div>
            <div class="absolute inset-0 rounded-full bg-black/50 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none">
              <CameraIcon class="size-6 text-white" />
            </div>
          </button>
          <div class="flex-1 min-w-0 pb-1">
            <h1 class="text-3xl font-bold font-display text-zinc-100">Edit Profile</h1>
            <p class="text-sm text-zinc-400">@{{ profile?.username }}</p>
          </div>
        </div>
      </div>

      <!-- Form sections -->
      <div class="px-8 py-6 space-y-8 max-w-4xl">

        <!-- Banner upload -->
        <div>
          <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-3">Banner Image</p>
          <button
            :ref="(el: any) => registerContent(el, { onSelect: triggerBannerUpload })"
            class="px-4 py-2.5 rounded-xl text-sm font-medium bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700 transition-colors"
            :class="{ 'opacity-50 pointer-events-none': bannerUploading }"
            @click="triggerBannerUpload"
          >
            {{ bannerUploading ? "Uploading..." : "Change Banner" }}
          </button>
        </div>

        <!-- Display Name -->
        <div>
          <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-3">Display Name</p>
          <button
            :ref="(el: any) => registerContent(el, { onSelect: () => focusInput('displayName') })"
            class="max-w-lg flex items-center rounded-xl border border-zinc-700/50 bg-zinc-800/50 px-4 py-3 text-left transition-colors hover:border-zinc-600"
            @click="focusInput('displayName')"
          >
            <span v-if="displayName" class="text-sm text-zinc-100">{{ displayName }}</span>
            <span v-else class="text-sm text-zinc-600">Your display name</span>
            <PencilIcon class="size-4 text-zinc-500 ml-auto shrink-0" />
          </button>
        </div>

        <!-- Bio -->
        <div>
          <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-3">Bio</p>
          <button
            :ref="(el: any) => registerContent(el, { onSelect: () => focusInput('bio') })"
            class="max-w-lg flex items-start rounded-xl border border-zinc-700/50 bg-zinc-800/50 px-4 py-3 text-left transition-colors hover:border-zinc-600 min-h-[4.5rem]"
            @click="focusInput('bio')"
          >
            <span v-if="bio" class="text-sm text-zinc-100 line-clamp-3 flex-1">{{ bio }}</span>
            <span v-else class="text-sm text-zinc-600 flex-1">Tell everyone about yourself...</span>
            <PencilIcon class="size-4 text-zinc-500 ml-3 mt-0.5 shrink-0" />
          </button>
          <p class="text-xs text-zinc-600 mt-1 text-right">{{ bio.length }}/500</p>
        </div>

        <!-- Profile Theme -->
        <div>
          <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-3">Profile Theme</p>
          <div class="grid grid-cols-4 sm:grid-cols-8 gap-3">
            <button
              v-for="theme in profileThemes"
              :key="theme.id"
              :ref="(el: any) => registerContent(el, { onSelect: () => { selectedTheme = theme.id; } })"
              class="flex flex-col items-center gap-2 p-3 rounded-xl border-2 transition-all"
              :class="selectedTheme === theme.id
                ? 'border-blue-500 bg-zinc-800/80 shadow-lg shadow-blue-500/10'
                : 'border-transparent bg-zinc-800/30 hover:border-zinc-600'"
              @click="selectedTheme = theme.id"
            >
              <div
                class="w-full h-8 rounded-lg"
                :style="{ background: `linear-gradient(135deg, ${theme.from}, ${theme.to})` }"
              />
              <span class="text-[10px] font-medium" :class="selectedTheme === theme.id ? 'text-blue-400' : 'text-zinc-500'">
                {{ theme.label }}
              </span>
            </button>
          </div>
        </div>

        <!-- Game Showcase -->
        <div>
          <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-3">Game Showcase</p>
          <p v-if="showcaseFull" class="text-xs text-zinc-400 -mt-2 mb-3">
            {{ showcaseFullHint }}
          </p>
          <div class="grid grid-cols-3 sm:grid-cols-6 gap-3">
            <template v-for="(slot, idx) in gameSlots" :key="'game-' + idx">
              <button
                v-if="slot"
                :ref="(el: any) => registerContent(el, { onSelect: () => removeGameSlot(idx) })"
                class="relative rounded-xl overflow-hidden bg-zinc-800/30 ring-1 ring-white/5 group text-left"
                @click="removeGameSlot(idx)"
              >
                <div class="aspect-[2/3]">
                  <img
                    v-if="slot.game?.mCoverObjectId"
                    :src="objectUrl(slot.game.mCoverObjectId)"
                    :alt="slot.game?.mName"
                    class="size-full object-cover"
                  />
                  <div v-else class="size-full flex items-center justify-center text-zinc-600">
                    <SparklesIcon class="size-6" />
                  </div>
                  <div class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-zinc-950/90 to-transparent p-2">
                    <p class="text-xs font-medium text-zinc-200 truncate">
                      {{ slot.game?.mName || slot.title }}
                    </p>
                  </div>
                  <div class="absolute top-1.5 right-1.5 p-1.5 rounded-full bg-zinc-900/80 text-zinc-400 group-hover:text-red-400 transition-colors">
                    <XMarkIcon class="size-4" />
                  </div>
                </div>
              </button>
              <button
                v-else
                :ref="(el: any) => registerContent(el, { onSelect: () => openGamePicker(idx) })"
                class="rounded-xl overflow-hidden bg-zinc-800/30 ring-1 ring-white/5 aspect-[2/3] flex flex-col items-center justify-center text-zinc-600 hover:text-zinc-400 transition-colors"
                :class="{ 'opacity-40': showcaseFull }"
                @click="openGamePicker(idx)"
              >
                <PlusIcon class="size-5 mb-1" />
                <span class="text-[10px]">Add</span>
              </button>
            </template>
          </div>
        </div>

        <!-- Achievement Showcase -->
        <div>
          <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-3">Achievement Showcase</p>
          <p v-if="showcaseFull" class="text-xs text-zinc-400 -mt-2 mb-3">
            {{ showcaseFullHint }}
          </p>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
            <template v-for="(slot, idx) in achievementSlots" :key="'ach-' + idx">
              <button
                v-if="slot"
                :ref="(el: any) => registerContent(el, { onSelect: () => removeAchievementSlot(idx) })"
                class="relative rounded-xl bg-zinc-800/30 ring-1 ring-white/5 group text-left"
                @click="removeAchievementSlot(idx)"
              >
                <div class="flex items-center gap-3 p-3">
                  <div class="shrink-0 size-11 rounded-lg overflow-hidden bg-zinc-700/50 flex items-center justify-center">
                    <img
                      v-if="slot.data?.iconUrl"
                      :src="String(slot.data.iconUrl)"
                      class="size-full object-cover"
                    />
                    <TrophyIcon v-else class="size-5 text-yellow-500" />
                  </div>
                  <div class="min-w-0 flex-1">
                    <p class="text-sm font-medium text-zinc-200 truncate">{{ slot.title }}</p>
                    <p class="text-xs text-zinc-500 truncate">{{ slot.game?.mName }}</p>
                  </div>
                  <div class="p-1.5 rounded-full bg-zinc-900/80 text-zinc-400 group-hover:text-red-400 transition-colors shrink-0">
                    <XMarkIcon class="size-4" />
                  </div>
                </div>
              </button>
              <button
                v-else
                :ref="(el: any) => registerContent(el, { onSelect: () => openAchievementPicker(idx) })"
                class="w-full flex items-center justify-center gap-2 p-4 rounded-xl bg-zinc-800/30 ring-1 ring-white/5 text-zinc-600 hover:text-zinc-400 transition-colors"
                :class="{ 'opacity-40': showcaseFull }"
                @click="openAchievementPicker(idx)"
              >
                <PlusIcon class="size-5" />
                <span class="text-xs">Add Achievement</span>
              </button>
            </template>
          </div>
        </div>

        <!-- Actions -->
        <div class="flex items-center gap-4 pb-8">
          <button
            :ref="(el: any) => registerContent(el, { onSelect: saveProfile })"
            class="px-6 py-3 rounded-xl text-sm font-semibold bg-blue-600 text-white hover:bg-blue-500 transition-colors shadow-lg shadow-blue-600/20"
            :class="{ 'opacity-50 pointer-events-none': saving }"
            @click="saveProfile"
          >
            {{ saving ? "Saving..." : "Save Changes" }}
          </button>
          <button
            :ref="(el: any) => registerContent(el, { onSelect: goBack })"
            class="px-6 py-3 rounded-xl text-sm font-medium bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700 transition-colors"
            @click="goBack"
          >
            Cancel
          </button>
          <span v-if="saveMessage" class="text-sm" :class="saveFailed ? 'text-red-400' : 'text-green-400'">{{ saveMessage }}</span>
        </div>
      </div>

      <!-- Hidden file input for banner -->
      <input
        ref="bannerFileInput"
        type="file"
        accept="image/*"
        class="hidden"
        @change="onBannerSelected"
      />
    </template>

    <!-- On-screen keyboard for text input -->
    <BigPictureKeyboard
      :visible="keyboardVisible"
      :model-value="keyboardValue"
      :placeholder="keyboardPlaceholder"
      @update:model-value="onKeyboardInput($event)"
      @close="closeKeyboard"
      @submit="closeKeyboard"
    />

    <!-- Game/Achievement Picker Overlay -->
    <Teleport to="body">
      <Transition
        enter-active-class="transition-opacity duration-200"
        leave-active-class="transition-opacity duration-200"
        enter-from-class="opacity-0"
        leave-to-class="opacity-0"
      >
        <div
          v-if="pickerOpen"
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
        >
          <div class="bg-zinc-900 border border-zinc-700/50 rounded-2xl shadow-2xl p-6 max-w-2xl w-full mx-4 max-h-[80vh] flex flex-col">
            <h2 class="text-lg font-semibold font-display text-zinc-100 mb-4">
              {{ pickerMode === 'achievement' && pickerSelectedGameId ? 'Select Achievement' : 'Select Game' }}
            </h2>

            <!-- Search button -->
            <button
              :ref="(el: any) => registerPicker(el, { onSelect: () => openPickerSearch() })"
              class="w-full flex items-center gap-2 px-4 py-2.5 rounded-xl bg-zinc-800/50 text-zinc-400 text-sm mb-3 hover:bg-zinc-700 transition-colors"
              @click="openPickerSearch"
            >
              <MagnifyingGlassIcon class="size-4" />
              <span v-if="pickerSearch" class="text-zinc-200">{{ pickerSearch }}</span>
              <span v-else class="text-zinc-600">Search games...</span>
            </button>

            <!-- Game list -->
            <div v-if="pickerMode === 'game' || !pickerSelectedGameId" class="flex-1 overflow-y-auto space-y-1 min-h-0">
              <button
                v-for="game in filteredPickerGames"
                :key="game.id"
                :ref="(el: any) => registerPicker(el, { onSelect: () => onPickerGameSelect(game) })"
                class="w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm text-left transition-colors"
                :class="pickerSelectedGameId === game.id
                  ? 'bg-blue-600/20 ring-1 ring-blue-500'
                  : 'bg-zinc-800/30 hover:bg-zinc-700'"
                @click="onPickerGameSelect(game)"
              >
                <img
                  v-if="game.mIconObjectId"
                  :src="objectUrl(game.mIconObjectId)"
                  class="size-8 rounded-lg object-cover shrink-0"
                />
                <div v-else class="size-8 rounded-lg bg-zinc-700 shrink-0" />
                <span class="text-zinc-200 truncate">{{ game.mName }}</span>
              </button>
              <p v-if="gamesLoadFailed" class="text-sm text-red-400 p-3 text-center">
                Could not load the game list.
              </p>
              <p v-else-if="filteredPickerGames.length === 0" class="text-sm text-zinc-500 p-3 text-center">
                No games found
              </p>
            </div>

            <!-- Achievement list -->
            <div v-else-if="pickerMode === 'achievement' && pickerSelectedGameId" class="flex-1 overflow-y-auto space-y-1 min-h-0">
              <div v-if="achievementsLoading" class="flex items-center justify-center p-6">
                <div class="size-6 border-2 border-blue-500/30 border-t-blue-500 rounded-full animate-spin" />
              </div>
              <template v-else>
                <button
                  v-for="ach in pickerAchievements"
                  :key="ach.id"
                  :ref="(el: any) => registerPicker(el, { onSelect: () => confirmAchievementPick(ach) })"
                  class="w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm text-left transition-colors bg-zinc-800/30 hover:bg-zinc-700"
                  @click="confirmAchievementPick(ach)"
                >
                  <div class="shrink-0 size-8 rounded-lg overflow-hidden bg-zinc-700/50 flex items-center justify-center">
                    <img v-if="ach.iconUrl" :src="ach.iconUrl" class="size-full object-cover" />
                    <TrophyIcon v-else class="size-4 text-yellow-500" />
                  </div>
                  <div class="flex-1 min-w-0">
                    <span class="text-zinc-200 truncate block">{{ ach.title }}</span>
                    <span v-if="ach.description" class="text-xs text-zinc-500 truncate block">{{ ach.description }}</span>
                  </div>
                </button>
                <p v-if="pickerAchievements.length === 0" class="text-sm text-zinc-500 p-3 text-center">
                  No achievements found for this game
                </p>
              </template>
            </div>

            <!-- Actions -->
            <div class="flex justify-end gap-2 mt-4 pt-3 border-t border-zinc-800">
              <button
                v-if="pickerMode === 'achievement' && pickerSelectedGameId"
                :ref="(el: any) => registerPicker(el, { onSelect: pickerGoBackToGames })"
                class="px-4 py-2.5 rounded-xl text-sm text-zinc-300 hover:text-zinc-100 bg-zinc-800/50 hover:bg-zinc-700 transition-colors mr-auto"
                @click="pickerGoBackToGames"
              >
                Back to Games
              </button>
              <button
                :ref="(el: any) => registerPicker(el, { onSelect: closePicker })"
                class="px-4 py-2.5 rounded-xl text-sm text-zinc-300 hover:text-zinc-100 bg-zinc-800/50 hover:bg-zinc-700 transition-colors"
                @click="closePicker"
              >
                Cancel
              </button>
              <button
                v-if="pickerMode === 'game' && pickerSelectedGameId"
                :ref="(el: any) => registerPicker(el, { onSelect: confirmGamePick })"
                class="px-4 py-2.5 rounded-xl text-sm font-medium bg-blue-600 text-white hover:bg-blue-500 transition-colors"
                @click="confirmGamePick"
              >
                Add Game
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <!-- Avatar Picker Overlay (canonical component) -->
    <ProfilePicturePicker
      :open="avatarPickerOpen"
      bpm-mode
      return-focus-group="content"
      @close="onAvatarPickerClose"
      @selected="onAvatarSelected"
    />
  </div>
</template>

<script setup lang="ts">
import {
  UserIcon,
  SparklesIcon,
  XMarkIcon,
  PlusIcon,
  CameraIcon,
  PencilIcon,
} from "@heroicons/vue/24/outline";
import { MagnifyingGlassIcon } from "@heroicons/vue/20/solid";
import { TrophyIcon } from "@heroicons/vue/24/solid";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";
import ProfilePicturePicker from "~/components/ProfilePicturePicker.vue";
import { serverUrl } from "~/composables/use-server-fetch";
import { objectImageUrl } from "~/composables/use-object";
import type { UserProfile, StoreGame } from "~/composables/use-server-api";
import { useAppState } from "~/composables/app-state";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { devLog } from "~/composables/dev-mode";
import { useServerApi } from "~/composables/use-server-api";
import {
  MAX_SHOWCASE_ITEMS,
  mergeShowcase,
  untouchedCount,
  type ShowcaseEntry,
} from "~/composables/bigpicture/showcase-merge";

definePageMeta({ layout: "bigpicture" });

function objectUrl(id: string): string {
  return objectImageUrl(id);
}

const THEME_MAP: Record<string, { from: string; to: string }> = {
  default: { from: "#1e3a5f", to: "#581c87" },
  ocean: { from: "#0c4a6e", to: "#164e63" },
  sunset: { from: "#9a3412", to: "#831843" },
  forest: { from: "#14532d", to: "#1a2e05" },
  ember: { from: "#7c2d12", to: "#451a03" },
  arctic: { from: "#0e7490", to: "#1e40af" },
  midnight: { from: "#1e1b4b", to: "#0f172a" },
  rose: { from: "#9f1239", to: "#4c0519" },
};

const profileThemes = [
  { id: "default", label: "Default", from: "#1e3a5f", to: "#581c87" },
  { id: "ocean", label: "Ocean", from: "#0c4a6e", to: "#164e63" },
  { id: "sunset", label: "Sunset", from: "#9a3412", to: "#831843" },
  { id: "forest", label: "Forest", from: "#14532d", to: "#1a2e05" },
  { id: "ember", label: "Ember", from: "#7c2d12", to: "#451a03" },
  { id: "arctic", label: "Arctic", from: "#0e7490", to: "#1e40af" },
  { id: "midnight", label: "Midnight", from: "#1e1b4b", to: "#0f172a" },
  { id: "rose", label: "Rose", from: "#9f1239", to: "#4c0519" },
];

const router = useRouter();
const state = useAppState();
const focusNav = useFocusNavigation();
const gamepad = useGamepad();
const registerContent = useBpFocusableGroup("content");
const registerPicker = useBpFocusableGroup("picker");

// ── State ─────────────────────────────────────────────────────────────────

const api = useServerApi();

const loading = ref(true);
// True when the profile or showcase could not be fetched. The editor is then
// not rendered, so nothing can be saved over the real data.
const loadFailed = ref(false);
const gamesLoadFailed = ref(false);
const saving = ref(false);
const saveMessage = ref("");
const saveFailed = ref(false);
const profile = ref<UserProfile | null>(null);

const displayName = ref("");
const bio = ref("");
const selectedTheme = ref("default");
const avatarPreview = ref<string | null>(null);
const bannerPreview = ref<string | null>(null);
const bannerUploading = ref(false);

const bannerFileInput = ref<HTMLInputElement | null>(null);

const themeColors = computed(
  () => THEME_MAP[selectedTheme.value] ?? THEME_MAP.default,
);

// ── On-screen keyboard ────────────────────────────────────────────────────

const keyboardVisible = ref(false);
const keyboardField = ref<"displayName" | "bio" | null>(null);
const keyboardValue = ref("");
const keyboardPlaceholder = ref("");

function focusInput(field: "displayName" | "bio") {
  keyboardField.value = field;
  keyboardValue.value = field === "displayName" ? displayName.value : bio.value;
  keyboardPlaceholder.value = field === "displayName" ? "Your display name" : "Tell everyone about yourself...";
  keyboardVisible.value = true;
}

function onKeyboardInput(val: string) {
  keyboardValue.value = val;
  if (keyboardField.value === "displayName") {
    displayName.value = val.slice(0, 64);
  } else if (keyboardField.value === "bio") {
    bio.value = val.slice(0, 500);
  }
}

function closeKeyboard() {
  keyboardVisible.value = false;
  keyboardField.value = null;
}

// ── Showcase slots ────────────────────────────────────────────────────────

const MAX_GAME_SLOTS = 6;
const MAX_ACH_SLOTS = 6;

type LocalShowcaseItem = {
  type: string;
  gameId: string | null;
  itemId: string | null;
  title: string;
  data: any;
  game?: {
    id: string;
    mName: string;
    mIconObjectId: string;
    mCoverObjectId: string;
  } | null;
};

const gameSlots = ref<(LocalShowcaseItem | null)[]>(
  Array.from({ length: MAX_GAME_SLOTS }, () => null),
);
const achievementSlots = ref<(LocalShowcaseItem | null)[]>(
  Array.from({ length: MAX_ACH_SLOTS }, () => null),
);

// The showcase as the server last returned it, in stored order. Saving merges
// the slot edits back into this list (see showcase-merge.ts) so items Big
// Picture doesn't show as slots survive the save.
const storedShowcase = ref<ShowcaseEntry[]>([]);

function toEntry(item: ShowcaseEntry): ShowcaseEntry {
  return {
    type: item.type,
    gameId: item.gameId ?? null,
    itemId: item.itemId ?? null,
    title: item.title ?? "",
    data: item.data ?? null,
  };
}

const showcaseFull = computed(() => {
  const filled =
    gameSlots.value.filter(Boolean).length +
    achievementSlots.value.filter(Boolean).length;
  const untouched = untouchedCount(storedShowcase.value, MAX_GAME_SLOTS, MAX_ACH_SLOTS);
  return filled + untouched >= MAX_SHOWCASE_ITEMS;
});

const showcaseFullHint = computed(() => {
  const anySlotFilled =
    gameSlots.value.some(Boolean) || achievementSlots.value.some(Boolean);
  return anySlotFilled
    ? `Your showcase is full (${MAX_SHOWCASE_ITEMS} items). Remove one to add another.`
    : `Your showcase is full (${MAX_SHOWCASE_ITEMS} items) with cards you can only change from the profile page on desktop.`;
});

/**
 * Fill the slots from a stored showcase list (server order). The slots show
 * the first MAX_GAME_SLOTS FavoriteGame and MAX_ACH_SLOTS Achievement items,
 * the same window `mergeShowcase` assumes when saving. `knownGames` supplies
 * cover art for items the server returned without it (the PUT response).
 */
function applyStoredShowcase(
  stored: LocalShowcaseItem[],
  knownGames: Map<string, NonNullable<LocalShowcaseItem["game"]>> = new Map(),
) {
  const withGame = stored.map((item) => ({
    ...item,
    game: item.game ?? (item.gameId ? knownGames.get(item.gameId) ?? null : null),
  }));
  for (const item of withGame) {
    if (item.game) storedGameArt.set(item.game.id, item.game);
  }
  storedShowcase.value = withGame.map(toEntry);
  const games = withGame.filter((i) => i.type === "FavoriteGame");
  const achs = withGame.filter((i) => i.type === "Achievement");
  gameSlots.value = Array.from({ length: MAX_GAME_SLOTS }, (_, i) => games[i] ?? null);
  achievementSlots.value = Array.from({ length: MAX_ACH_SLOTS }, (_, i) => achs[i] ?? null);
}

// Cover art the server sent with the stored showcase (for all 12 items, not
// just the ones in slots), so an item that moves into the slot window after a
// save still has its art. The PUT response carries none.
const storedGameArt = new Map<string, NonNullable<LocalShowcaseItem["game"]>>();

/** Cover art for every game the editor already knows about. */
function knownGameMap() {
  const map = new Map(storedGameArt);
  for (const g of allGames.value) {
    map.set(g.id, {
      id: g.id,
      mName: g.mName,
      mIconObjectId: g.mIconObjectId,
      mCoverObjectId: g.mCoverObjectId,
    });
  }
  for (const slot of [...gameSlots.value, ...achievementSlots.value]) {
    if (slot?.game) map.set(slot.game.id, slot.game);
  }
  return map;
}

function removeGameSlot(idx: number) { gameSlots.value[idx] = null; }
function removeAchievementSlot(idx: number) { achievementSlots.value[idx] = null; }

// ── Picker overlay ────────────────────────────────────────────────────────

const pickerOpen = ref(false);
const pickerMode = ref<"game" | "achievement">("game");
const pickerSlotIndex = ref(0);
const pickerSearch = ref("");
const pickerSelectedGameId = ref<string | null>(null);
const allGames = ref<StoreGame[]>([]);
const achievementsLoading = ref(false);

type AchOption = { id: string; title: string; description?: string; iconUrl?: string };
const pickerAchievements = ref<AchOption[]>([]);

const filteredPickerGames = computed(() => {
  const q = pickerSearch.value.toLowerCase();
  if (!q) return allGames.value.slice(0, 30);
  return allGames.value.filter((g) => g.mName.toLowerCase().includes(q)).slice(0, 30);
});

function openGamePicker(idx: number) {
  if (showcaseFull.value) return;
  pickerMode.value = "game";
  pickerSlotIndex.value = idx;
  pickerSelectedGameId.value = null;
  pickerSearch.value = "";
  pickerAchievements.value = [];
  pickerOpen.value = true;
  nextTick(() => {
    focusNav.restrictFocus("picker");
    nextTick(() => focusNav.autoFocusContent("picker"));
  });
}

function openAchievementPicker(idx: number) {
  if (showcaseFull.value) return;
  pickerMode.value = "achievement";
  pickerSlotIndex.value = idx;
  pickerSelectedGameId.value = null;
  pickerSearch.value = "";
  pickerAchievements.value = [];
  pickerOpen.value = true;
  nextTick(() => {
    focusNav.restrictFocus("picker");
    nextTick(() => focusNav.autoFocusContent("picker"));
  });
}

function closePicker() {
  pickerOpen.value = false;
  focusNav.unrestrictFocus("content");
}

function openPickerSearch() {
  // Use on-screen keyboard for picker search
  keyboardField.value = null; // special mode: picker search
  keyboardValue.value = pickerSearch.value;
  keyboardPlaceholder.value = "Search games...";
  keyboardVisible.value = true;
}

function pickerGoBackToGames() {
  pickerSelectedGameId.value = null;
  pickerAchievements.value = [];
}

function onPickerGameSelect(game: StoreGame) {
  if (pickerMode.value === "game") {
    pickerSelectedGameId.value = game.id;
    return;
  }
  // Achievement mode — select game, then load achievements
  pickerSelectedGameId.value = game.id;
  loadGameAchievements(game.id);
}

async function loadGameAchievements(gameId: string) {
  achievementsLoading.value = true;
  try {
    const url = serverUrl(`api/v1/games/${gameId}/achievements`);
    const res = await fetch(url);
    if (res.ok) {
      pickerAchievements.value = await res.json();
    }
  } catch {
    pickerAchievements.value = [];
  } finally {
    achievementsLoading.value = false;
  }
}

function confirmGamePick() {
  if (!pickerSelectedGameId.value) return;
  const game = allGames.value.find((g) => g.id === pickerSelectedGameId.value);
  if (!game) return;

  gameSlots.value[pickerSlotIndex.value] = {
    type: "FavoriteGame",
    gameId: game.id,
    itemId: null,
    title: game.mName,
    data: null,
    game: {
      id: game.id,
      mName: game.mName,
      mIconObjectId: game.mIconObjectId,
      mCoverObjectId: game.mCoverObjectId,
    },
  };
  closePicker();
}

function confirmAchievementPick(ach: AchOption) {
  const game = allGames.value.find((g) => g.id === pickerSelectedGameId.value);
  achievementSlots.value[pickerSlotIndex.value] = {
    type: "Achievement",
    gameId: pickerSelectedGameId.value,
    itemId: ach.id,
    title: ach.title,
    data: { iconUrl: ach.iconUrl, description: ach.description },
    game: game ? {
      id: game.id,
      mName: game.mName,
      mIconObjectId: game.mIconObjectId,
      mCoverObjectId: game.mCoverObjectId,
    } : null,
  };
  closePicker();
}

// ── Avatar picker ────────────────────────────────────────────────────────
// The picker UI itself lives in `components/ProfilePicturePicker.vue` so
// both BPM and the desktop edit page render the same gallery. This page
// only tracks the open flag and reacts to the component's emitted events.
// The component drives the BPM focus-restriction lifecycle internally.

const avatarPickerOpen = ref(false);

function openAvatarPicker() {
  avatarPickerOpen.value = true;
}

function onAvatarPickerClose() {
  avatarPickerOpen.value = false;
}

function onAvatarSelected(newObjectId: string) {
  devLog("state", "[BPM:PROFILE] Avatar upload success, new id:", newObjectId);
  // Update app state so the top bar avatar refreshes immediately.
  if (state.value?.user) {
    state.value.user.profilePictureObjectId = newObjectId;
  }
  if (profile.value) {
    profile.value.profilePictureObjectId = newObjectId;
  }
  // Clear any local preview so the freshly-uploaded server image is what
  // shows. The component itself will close after success.
  avatarPreview.value = null;
}

// ── Banner upload ─────────────────────────────────────────────────────────

function triggerBannerUpload() {
  bannerFileInput.value?.click();
}

async function onBannerSelected(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;

  if (bannerPreview.value) URL.revokeObjectURL(bannerPreview.value);
  bannerPreview.value = URL.createObjectURL(file);
  bannerUploading.value = true;
  clearSaveResult();
  try {
    // Native upload command, the same path the desktop editor uses. A
    // multipart fetch over server:// hard-crashes WebKitGTK on the Deck.
    const { bannerObjectId } = await api.profile.uploadBanner(file);
    if (profile.value) profile.value.bannerObjectId = bannerObjectId;
  } catch (err) {
    console.error("[BPM:PROFILE] Banner upload failed:", err);
    showSaveResult(`Could not upload the banner: ${String(err)}`, true);
  } finally {
    // Show the stored banner (new on success, old on failure), not the local file.
    if (bannerPreview.value) URL.revokeObjectURL(bannerPreview.value);
    bannerPreview.value = null;
    bannerUploading.value = false;
    input.value = "";
  }
}

// ── Save ──────────────────────────────────────────────────────────────────

/** The server's own error text from an h3 error response, if it sent one. */
async function serverErrorText(res: Response): Promise<string> {
  const body = await res.json().catch(() => null);
  return body?.statusMessage || body?.message || `HTTP ${res.status}`;
}

let saveMessageTimer: ReturnType<typeof setTimeout> | null = null;

function clearSaveResult() {
  if (saveMessageTimer) clearTimeout(saveMessageTimer);
  saveMessageTimer = null;
  saveMessage.value = "";
  saveFailed.value = false;
}

function showSaveResult(message: string, failed: boolean) {
  clearSaveResult();
  saveMessage.value = message;
  saveFailed.value = failed;
  // Failures stay until the next attempt so they can't be missed.
  if (!failed) saveMessageTimer = setTimeout(clearSaveResult, 3000);
}

async function saveProfile() {
  // The editor isn't rendered when the load failed, but never save over the
  // stored profile from a state that didn't come from the server.
  if (loadFailed.value || !profile.value) return;
  saving.value = true;
  clearSaveResult();
  let profileSaved = false;
  try {
    const profileRes = await fetch(serverUrl("api/v1/user/profile"), {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        displayName: displayName.value,
        bio: bio.value,
        profileTheme: selectedTheme.value,
      }),
    });
    if (!profileRes.ok) {
      const why = await serverErrorText(profileRes);
      console.error("[BPM:PROFILE] Profile PATCH failed:", profileRes.status, why);
      showSaveResult(`Failed to save profile: ${why}`, true);
      return;
    }
    profileSaved = true;

    const filledGames = gameSlots.value
      .filter((s): s is LocalShowcaseItem => s !== null)
      .map(toEntry);
    const filledAchs = achievementSlots.value
      .filter((s): s is LocalShowcaseItem => s !== null)
      .map(toEntry);
    const items = mergeShowcase(
      storedShowcase.value,
      filledGames,
      filledAchs,
      MAX_GAME_SLOTS,
      MAX_ACH_SLOTS,
    );

    const showcaseRes = await fetch(serverUrl("api/v1/user/showcase"), {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ items }),
    });
    if (!showcaseRes.ok) {
      const why = await serverErrorText(showcaseRes);
      console.error("[BPM:PROFILE] Showcase PUT failed:", showcaseRes.status, why);
      showSaveResult(`Profile saved, but the showcase failed: ${why}`, true);
      return;
    }
    // What the server now holds is the baseline for the next save, and the
    // slots must show the same window of it that the next merge will assume.
    // Otherwise a stored item that just moved into that window (one past the
    // slot limit, after a removal) is invisible and the next save drops it.
    const saved = await showcaseRes.json().catch(() => null);
    applyStoredShowcase(saved?.items ?? items, knownGameMap());

    showSaveResult("Profile saved!", false);
  } catch (err) {
    console.error("[BPM:PROFILE] Save failed:", err);
    showSaveResult(
      profileSaved
        ? "Profile saved, but the showcase failed: could not reach the server"
        : "Failed to save: could not reach the server",
      true,
    );
  } finally {
    saving.value = false;
  }
}

function goBack() {
  router.back();
}

// ── B button = back / close ───────────────────────────────────────────────

const _unsubs: (() => void)[] = [];
_unsubs.push(
  gamepad.onButton(GamepadButton.East, () => {
    if (keyboardVisible.value) {
      closeKeyboard();
    } else if (avatarPickerOpen.value) {
      onAvatarPickerClose();
    } else if (pickerOpen.value) {
      if (pickerMode.value === "achievement" && pickerSelectedGameId.value) {
        pickerGoBackToGames();
      } else {
        closePicker();
      }
    } else {
      goBack();
    }
  }),
);

// ── Data loading ──────────────────────────────────────────────────────────

/** JSON body of a successful GET, or null for any failure. */
async function getJson(url: string): Promise<any | null> {
  try {
    const res = await fetch(url);
    return res.ok ? await res.json() : null;
  } catch {
    return null;
  }
}

async function loadProfile() {
  loading.value = true;
  loadFailed.value = false;
  try {
    const userId = state.value?.user?.id;
    if (!userId) {
      router.replace("/bigpicture");
      return;
    }

    const [profileRes, showcaseRes, gamesRes] = await Promise.all([
      getJson(serverUrl(`api/v1/user/${userId}`)),
      getJson(serverUrl(`api/v1/user/${userId}/showcase`)),
      getJson(serverUrl("api/v1/store?sort=name&order=asc&take=200")),
    ]);

    // Without both of these the editor would start from blanks, and Save
    // would write those blanks over the real profile.
    if (!profileRes || !showcaseRes) {
      loadFailed.value = true;
      return;
    }

    profile.value = profileRes;
    displayName.value = profileRes.displayName ?? "";
    bio.value = profileRes.bio ?? "";
    selectedTheme.value = profileRes.profileTheme ?? "default";

    gamesLoadFailed.value = !gamesRes;
    allGames.value = gamesRes?.results ?? [];

    applyStoredShowcase(showcaseRes.items ?? []);
  } catch (err) {
    console.error("[BPM:PROFILE] Failed to load profile data:", err);
    loadFailed.value = true;
  } finally {
    loading.value = false;
    nextTick(() => focusNav.autoFocusContent("content"));
  }
}

/**
 * "Try again" from the load-failed state. The button that was pressed is about
 * to leave the DOM while still holding focus, and autoFocusContent won't move
 * focus while anything is focused, so let go of it first.
 */
function retryLoad() {
  focusNav.clearFocus();
  loadProfile();
}

onMounted(loadProfile);

onUnmounted(() => {
  if (saveMessageTimer) clearTimeout(saveMessageTimer);
  for (const unsub of _unsubs) unsub();
  if (bannerPreview.value) URL.revokeObjectURL(bannerPreview.value);
});
</script>
