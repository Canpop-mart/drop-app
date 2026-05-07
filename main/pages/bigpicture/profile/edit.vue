<template>
  <div class="flex flex-col h-full overflow-y-auto">
    <!-- Loading state -->
    <div v-if="loading" class="flex-1 flex items-center justify-center">
      <div class="size-12 border-4 border-blue-500/30 border-t-blue-500 rounded-full animate-spin" />
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
          <span v-if="saveMessage" class="text-sm" :class="saveMessage.includes('Failed') ? 'text-red-400' : 'text-green-400'">{{ saveMessage }}</span>
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
              <p v-if="filteredPickerGames.length === 0" class="text-sm text-zinc-500 p-3 text-center">
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

    <!-- Avatar Picker Overlay -->
    <Teleport to="body">
      <Transition
        enter-active-class="transition-opacity duration-200"
        leave-active-class="transition-opacity duration-200"
        enter-from-class="opacity-0"
        leave-to-class="opacity-0"
      >
        <div
          v-if="avatarPickerOpen"
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
        >
          <div class="bg-zinc-900 border border-zinc-700/50 rounded-2xl shadow-2xl p-6 max-w-3xl w-full mx-4 max-h-[85vh] flex flex-col">
            <h2 class="text-lg font-semibold font-display text-zinc-100 mb-4">Choose Your Avatar</h2>

            <!-- Avatar grid -->
            <div class="flex-1 overflow-y-auto min-h-0">
              <div class="grid grid-cols-6 sm:grid-cols-8 md:grid-cols-10 gap-2 p-1">
                <button
                  v-for="pic in profilePics"
                  :key="pic"
                  :ref="(el: any) => registerAvatarPicker(el, { onSelect: () => selectAvatar(pic) })"
                  class="relative aspect-square rounded-xl overflow-hidden ring-2 transition-all hover:scale-105"
                  :class="selectedAvatarPic === pic
                    ? 'ring-blue-500 shadow-lg shadow-blue-500/20'
                    : 'ring-transparent hover:ring-zinc-600'"
                  @click="selectAvatar(pic)"
                >
                  <img
                    :src="avatarPicUrl(pic)"
                    :alt="pic"
                    class="size-full object-cover"
                    loading="lazy"
                  />
                </button>
              </div>
            </div>

            <!-- Actions -->
            <div class="flex justify-end gap-2 mt-4 pt-3 border-t border-zinc-800">
              <button
                :ref="(el: any) => registerAvatarPicker(el, { onSelect: closeAvatarPicker })"
                class="px-4 py-2.5 rounded-xl text-sm text-zinc-300 hover:text-zinc-100 bg-zinc-800/50 hover:bg-zinc-700 transition-colors"
                @click="closeAvatarPicker"
              >
                Cancel
              </button>
              <button
                v-if="selectedAvatarPic"
                :ref="(el: any) => registerAvatarPicker(el, { onSelect: confirmAvatarPick })"
                class="px-4 py-2.5 rounded-xl text-sm font-medium bg-blue-600 text-white hover:bg-blue-500 transition-colors"
                :class="{ 'opacity-50 pointer-events-none': avatarUploading }"
                @click="confirmAvatarPick"
              >
                {{ avatarUploading ? "Saving..." : "Select Avatar" }}
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
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
import { serverUrl } from "~/composables/use-server-fetch";
import type { UserProfile, StoreGame } from "~/composables/use-server-api";
import { useAppState } from "~/composables/app-state";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { devLog } from "~/composables/dev-mode";

definePageMeta({ layout: "bigpicture" });

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
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
const registerAvatarPicker = useBpFocusableGroup("avatar-picker");

// ── State ─────────────────────────────────────────────────────────────────

const loading = ref(true);
const saving = ref(false);
const saveMessage = ref("");
const profile = ref<UserProfile | null>(null);

const displayName = ref("");
const bio = ref("");
const selectedTheme = ref("default");
const avatarPreview = ref<string | null>(null);
const bannerPreview = ref<string | null>(null);
const avatarUploading = ref(false);
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

// ── Avatar picker (Xbox profile pics) ─────────────────────────────────────

const profilePics = [
  "20000.png","20000(1).png","20000(2).png","20001.png","20001(1).png",
  "20002.png","20002(1).png","20003.png","20003(1).png","20004.png",
  "20005.png","20006.png","20007.png",
  "20008.png","20009.png","2000a.png","2000b.png","2000c.png",
  "2000d.png","2000d(1).png","2007b.png",
  "20400.png","20402.png","20405.png","20406.png","2041c.png","20429.png","20431.png",
  "21000.png","21001.png","21002.png","21003.png","21004.png","21005.png",
  "21006.png","21007.png","21008.png","21009.png","21010.png","21011.png",
  "21012.png","21013.png","21014.png","21015.png","21016.png","21017.png",
  "21018.png","21019.png","21020.png","21021.png","21022.png","21023.png",
  "21024.png","21025.png","21026.png","21027.png","21029.png","21030.png",
  "21031.png","21032.png","21033.png","21035.png","21036.png","21037.png",
  "21038.png","21039.png","21040.png","21041.png","21042.png","21043.png",
  "21044.png","21045.png","21046.png","21047.png","21048.png","21049.png",
  "21050.png","21051.png","21052.png","21053.png","21054.png","21055.png",
  "21056.png","21057.png","21058.png","21059.png","21060.png","21061.png",
  "21062.png","21063.png","21064.png","21065.png","21066.png","21067.png",
  "21068.png","21069.png",
  "28000.png","28006.png","28008.png","28017.png","2801a.png","28026.png","2802e.png","2808e.png",
];

const avatarPickerOpen = ref(false);
const selectedAvatarPic = ref<string | null>(null);

// Resolve profile pic base path with Nuxt baseURL
const _runtimeBase = (useRuntimeConfig().app.baseURL ?? "/").replace(/\/$/, "");
const _profilePicBase = `${_runtimeBase}/img/boxart/profilepic`;

function avatarPicUrl(filename: string): string {
  return `${_profilePicBase}/${filename}`;
}

function openAvatarPicker() {
  selectedAvatarPic.value = null;
  avatarPickerOpen.value = true;
  nextTick(() => {
    focusNav.restrictFocus("avatar-picker");
    nextTick(() => focusNav.autoFocusContent("avatar-picker"));
  });
}

function closeAvatarPicker() {
  avatarPickerOpen.value = false;
  focusNav.unrestrictFocus("content");
}

function selectAvatar(pic: string) {
  selectedAvatarPic.value = pic;
}

async function confirmAvatarPick() {
  devLog("state","[BPM:PROFILE] confirmAvatarPick called, selectedAvatarPic:", selectedAvatarPic.value);
  if (!selectedAvatarPic.value) return;
  avatarUploading.value = true;
  try {
    // Fetch the preset image as a blob and upload it to the avatar endpoint
    const imgUrl = avatarPicUrl(selectedAvatarPic.value);
    devLog("state","[BPM:PROFILE] Fetching preset image from:", imgUrl);
    const res = await fetch(imgUrl);
    devLog("state","[BPM:PROFILE] Image fetch status:", res.status, res.ok);
    if (!res.ok) {
      console.error("[BPM:PROFILE] Failed to fetch preset image:", res.status, res.statusText);
      return;
    }
    const blob = await res.blob();
    devLog("state","[BPM:PROFILE] Got blob:", blob.size, "bytes, type:", blob.type);
    const file = new File([blob], selectedAvatarPic.value, { type: blob.type || "image/png" });

    const form = new FormData();
    form.append("file", file);
    const url = serverUrl("api/v1/user/avatar");
    devLog("state","[BPM:PROFILE] Uploading avatar to:", url);
    const uploadRes = await fetch(url, { method: "POST", body: form });
    devLog("state","[BPM:PROFILE] Upload response:", uploadRes.status, uploadRes.ok);

    // Show preview
    avatarPreview.value = imgUrl;

    // Update app state so the top bar avatar refreshes immediately
    if (uploadRes.ok) {
      try {
        const data = await uploadRes.json();
        devLog("state","[BPM:PROFILE] Avatar response data:", JSON.stringify(data));
        // Server returns { profilePictureObjectId: "..." }
        const newId = data?.profilePictureObjectId;
        if (newId && state.value?.user) {
          state.value.user.profilePictureObjectId = newId;
          // Also update our local profile ref
          if (profile.value) {
            profile.value.profilePictureObjectId = newId;
          }
        }
      } catch (e) {
        console.error("[BPM:PROFILE] Failed to parse avatar response:", e);
      }
    } else {
      const errText = await uploadRes.text().catch(() => "");
      console.error("[BPM:PROFILE] Avatar upload failed with status:", uploadRes.status, errText);
    }

    closeAvatarPicker();
  } catch (err) {
    console.error("[BPM:PROFILE] Avatar upload error:", err);
  } finally {
    avatarUploading.value = false;
  }
}

// ── Banner upload ─────────────────────────────────────────────────────────

function triggerBannerUpload() {
  bannerFileInput.value?.click();
}

async function onBannerSelected(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;

  bannerPreview.value = URL.createObjectURL(file);
  bannerUploading.value = true;
  try {
    const form = new FormData();
    form.append("file", file);
    const url = serverUrl("api/v1/user/banner");
    await fetch(url, { method: "POST", body: form });
  } catch (err) {
    console.error("[BPM:PROFILE] Banner upload failed:", err);
  } finally {
    bannerUploading.value = false;
  }
}

// ── Save ──────────────────────────────────────────────────────────────────

async function saveProfile() {
  devLog("state","[BPM:PROFILE] saveProfile called");
  saving.value = true;
  saveMessage.value = "";
  try {
    // Save profile fields
    const profileUrl = serverUrl("api/v1/user/profile");
    devLog("state","[BPM:PROFILE] PATCH profile to:", profileUrl, { displayName: displayName.value, bio: bio.value, profileTheme: selectedTheme.value });
    const profileRes = await fetch(profileUrl, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        displayName: displayName.value,
        bio: bio.value,
        profileTheme: selectedTheme.value,
      }),
    });
    devLog("state","[BPM:PROFILE] Profile PATCH response:", profileRes.status, profileRes.ok);
    if (!profileRes.ok) {
      const errText = await profileRes.text().catch(() => "");
      console.error("[BPM:PROFILE] Profile PATCH failed:", profileRes.status, errText);
    }

    // Save showcase
    const gameItems = gameSlots.value
      .filter((s): s is LocalShowcaseItem => s !== null)
      .map((s) => ({ type: s.type, gameId: s.gameId, itemId: s.itemId, title: s.title, data: s.data }));
    const achItems = achievementSlots.value
      .filter((s): s is LocalShowcaseItem => s !== null)
      .map((s) => ({ type: s.type, gameId: s.gameId, itemId: s.itemId, title: s.title, data: s.data }));

    const showcaseUrl = serverUrl("api/v1/user/showcase");
    devLog("state","[BPM:PROFILE] PUT showcase to:", showcaseUrl, "items:", gameItems.length + achItems.length);
    const showcaseRes = await fetch(showcaseUrl, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ items: [...gameItems, ...achItems] }),
    });
    devLog("state","[BPM:PROFILE] Showcase PUT response:", showcaseRes.status, showcaseRes.ok);
    if (!showcaseRes.ok) {
      const errText = await showcaseRes.text().catch(() => "");
      console.error("[BPM:PROFILE] Showcase PUT failed:", showcaseRes.status, errText);
    }

    saveMessage.value = "Profile saved!";
    setTimeout(() => { saveMessage.value = ""; }, 3000);
  } catch (err) {
    console.error("[BPM:PROFILE] Save failed:", err);
    saveMessage.value = "Failed to save";
    setTimeout(() => { saveMessage.value = ""; }, 3000);
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
      closeAvatarPicker();
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

onMounted(async () => {
  try {
    const userId = state.value?.user?.id;
    if (!userId) {
      router.replace("/bigpicture");
      return;
    }

    // Fetch profile, showcase, and games list in parallel
    const profileUrl = serverUrl(`api/v1/user/${userId}`);
    const showcaseUrl = serverUrl(`api/v1/user/${userId}/showcase`);
    const gamesUrl = serverUrl("api/v1/store?sort=name&order=asc&take=200");

    const [profileRes, showcaseRes, gamesRes] = await Promise.all([
      fetch(profileUrl).then((r) => r.ok ? r.json() : null).catch(() => null),
      fetch(showcaseUrl).then((r) => r.ok ? r.json() : null).catch(() => null),
      fetch(gamesUrl).then((r) => r.ok ? r.json() : null).catch(() => null),
    ]);

    if (profileRes) {
      profile.value = profileRes;
      displayName.value = profileRes.displayName ?? "";
      bio.value = profileRes.bio ?? "";
      selectedTheme.value = profileRes.profileTheme ?? "default";
    }

    allGames.value = gamesRes?.results ?? [];

    if (showcaseRes) {
      const existingGames = (showcaseRes.items ?? []).filter(
        (i: any) => i.type === "FavoriteGame",
      );
      const existingAchs = (showcaseRes.items ?? []).filter(
        (i: any) => i.type === "Achievement",
      );
      gameSlots.value = Array.from({ length: MAX_GAME_SLOTS }, (_, i) =>
        existingGames[i] ? { ...existingGames[i] } : null,
      );
      achievementSlots.value = Array.from({ length: MAX_ACH_SLOTS }, (_, i) =>
        existingAchs[i] ? { ...existingAchs[i] } : null,
      );
    }
  } catch (err) {
    console.error("[BPM:PROFILE] Failed to load profile data:", err);
  } finally {
    loading.value = false;
    nextTick(() => focusNav.autoFocusContent("content"));
  }
});

onUnmounted(() => {
  for (const unsub of _unsubs) unsub();
  if (bannerPreview.value) URL.revokeObjectURL(bannerPreview.value);
});
</script>
