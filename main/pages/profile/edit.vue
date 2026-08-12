<template>
  <div class="mx-auto max-w-3xl px-8 py-8">
    <div class="flex items-center justify-between mb-6">
      <div>
        <h1 class="text-2xl font-display font-bold text-zinc-100">
          Edit profile
        </h1>
        <p class="mt-1 text-sm text-zinc-400">
          Public details visible on your profile page.
        </p>
      </div>
      <NuxtLink
        to="/profile"
        class="text-sm font-medium text-zinc-400 hover:text-zinc-200 transition-colors"
      >
        Cancel
      </NuxtLink>
    </div>

    <div
      v-if="loading"
      class="flex items-center justify-center min-h-[40vh] text-zinc-500 text-sm"
    >
      Loading...
    </div>

    <template v-else-if="profile">
      <!-- Banner preview + change -->
      <div class="mb-6">
        <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">
          Banner image
        </p>
        <div class="relative h-32 rounded-xl overflow-hidden bg-zinc-800">
          <img
            v-if="bannerPreview || profile.bannerObjectId"
            :src="bannerPreview || objectUrl(profile.bannerObjectId!)"
            class="w-full h-full object-cover"
          />
          <div
            v-else
            class="w-full h-full flex items-center justify-center text-zinc-600 text-sm"
          >
            No banner set
          </div>
        </div>
        <div class="mt-2 flex items-center gap-3">
          <label
            class="inline-flex items-center gap-x-2 rounded-md bg-zinc-800/50 px-3 py-1.5 text-sm font-medium text-zinc-200 cursor-pointer hover:bg-zinc-800 transition-colors"
            :class="{ 'opacity-50 pointer-events-none': bannerUploading }"
          >
            <PhotoIcon class="size-4" />
            {{ bannerUploading ? "Uploading..." : "Change banner" }}
            <input
              type="file"
              accept="image/*"
              class="hidden"
              @change="onBannerSelect"
            />
          </label>
          <p class="text-xs text-zinc-500">
            Recommended 1500×500. Auto-cropped on the profile page.
          </p>
        </div>
      </div>

      <!-- Avatar — uses the canonical preset gallery picker shared with BPM -->
      <div class="mb-6">
        <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">
          Avatar
        </p>
        <div class="flex items-center gap-4">
          <img
            v-if="profile.profilePictureObjectId"
            :src="objectUrl(profile.profilePictureObjectId!)"
            class="size-20 rounded-full object-cover ring-2 ring-zinc-700"
          />
          <div
            v-else
            class="size-20 rounded-full bg-zinc-800 flex items-center justify-center ring-2 ring-zinc-700"
          >
            <UserIcon class="size-10 text-zinc-500" />
          </div>
          <button
            type="button"
            class="inline-flex items-center gap-x-2 rounded-md bg-zinc-800/50 px-3 py-1.5 text-sm font-medium text-zinc-200 hover:bg-zinc-800 transition-colors"
            @click="avatarPickerOpen = true"
          >
            <PhotoIcon class="size-4" />
            Choose avatar
          </button>
        </div>
        <p class="text-xs text-zinc-500 mt-2">
          Pick from the preset gallery — choices match Big Picture mode.
        </p>
      </div>

      <ProfilePicturePicker
        :open="avatarPickerOpen"
        @close="avatarPickerOpen = false"
        @selected="onAvatarSelected"
        @error="onAvatarError"
      />

      <!-- Display name -->
      <div class="mb-6">
        <label
          for="displayName"
          class="block text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2"
        >
          Display name
        </label>
        <input
          id="displayName"
          v-model="displayName"
          type="text"
          maxlength="64"
          class="w-full rounded-md border border-zinc-700 bg-zinc-800/50 px-3 py-2 text-sm text-zinc-100 placeholder:text-zinc-500 focus:bg-zinc-800 focus:border-blue-500 focus:ring-2 focus:ring-blue-500/30 outline-none transition-colors"
          placeholder="Your display name"
        />
        <p class="text-xs text-zinc-500 mt-1">
          Shown publicly. Your @username
          (<span class="font-mono text-zinc-400">{{ profile.username }}</span>)
          is fixed.
        </p>
      </div>

      <!-- Bio -->
      <div class="mb-6">
        <label
          for="bio"
          class="block text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2"
        >
          Bio
        </label>
        <textarea
          id="bio"
          v-model="bio"
          maxlength="500"
          rows="4"
          class="w-full rounded-md border border-zinc-700 bg-zinc-800/50 px-3 py-2 text-sm text-zinc-100 placeholder:text-zinc-500 focus:bg-zinc-800 focus:border-blue-500 focus:ring-2 focus:ring-blue-500/30 outline-none transition-colors resize-none"
          placeholder="Tell people about yourself..."
        />
        <p class="text-xs text-zinc-500 mt-1 text-right">
          {{ bio.length }} / 500
        </p>
      </div>

      <!-- Profile theme — an accent that threads through the whole profile.
           Presets + a custom colour, with a live preview of the result. -->
      <div class="mb-8" :style="previewVars">
        <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">
          Accent colour
        </p>
        <div class="flex flex-wrap items-center gap-3">
          <button
            v-for="p in presetList"
            :key="p.id"
            type="button"
            class="size-9 rounded-full transition-transform hover:scale-110"
            :class="
              selectedTheme === p.id
                ? 'ring-2 ring-white ring-offset-2 ring-offset-zinc-950'
                : ''
            "
            :style="{ background: p.accent }"
            :title="p.label"
            @click="selectPreset(p.id)"
          />
          <label
            class="relative block size-9 cursor-pointer overflow-hidden rounded-full ring-1 ring-zinc-600"
            :class="
              isCustom
                ? 'ring-2 ring-white ring-offset-2 ring-offset-zinc-950'
                : ''
            "
            :style="{
              background: isCustom
                ? customColor
                : 'conic-gradient(from 0deg, #ef4444, #f59e0b, #22c55e, #06b6d4, #3b82f6, #a855f7, #ef4444)',
            }"
            title="Custom colour"
          >
            <input
              v-model="customColor"
              type="color"
              class="absolute inset-0 h-full w-full cursor-pointer opacity-0"
              @input="onColorInput"
            />
          </label>
          <input
            v-if="isCustom"
            :value="customColor"
            maxlength="7"
            spellcheck="false"
            class="w-24 rounded-md border border-zinc-700 bg-zinc-800/50 px-2 py-1.5 font-mono text-xs text-zinc-100 focus:border-blue-500 focus:outline-none"
            @input="onHexInput"
          />
        </div>

        <!-- Live preview -->
        <div class="mt-3 overflow-hidden rounded-lg ring-1 ring-zinc-700/40">
          <div class="h-10" :style="{ background: 'var(--profile-banner)' }" />
          <div class="flex items-center gap-2 bg-zinc-900 px-3 py-2">
            <span class="text-sm font-semibold text-[color:var(--accent)]">
              Accent preview
            </span>
            <span
              class="ml-auto rounded-md px-2.5 py-1 text-xs font-semibold"
              :style="{
                background: 'var(--accent)',
                color: 'var(--accent-contrast)',
              }"
            >
              Button
            </span>
          </div>
        </div>

        <p class="text-xs text-zinc-500 mt-2">
          Threads through your whole profile — name, stats, shelves and buttons,
          plus the banner when you haven't uploaded one.
        </p>
      </div>

      <!-- Content editors — favourites + showcase live on their own pages. -->
      <div class="mb-8">
        <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">
          Profile content
        </p>
        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <NuxtLink
            to="/profile/favorites"
            class="flex items-center gap-3 rounded-xl bg-zinc-800/40 p-4 ring-1 ring-zinc-700/50 transition-colors hover:bg-zinc-800"
          >
            <StarIcon class="size-5 shrink-0 text-zinc-400" />
            <div class="min-w-0">
              <p class="text-sm font-medium text-zinc-100">Favourite games</p>
              <p class="text-xs text-zinc-500">Pin and reorder your top games</p>
            </div>
            <ChevronRightIcon class="ml-auto size-4 shrink-0 text-zinc-600" />
          </NuxtLink>
          <NuxtLink
            to="/profile/showcase"
            class="flex items-center gap-3 rounded-xl bg-zinc-800/40 p-4 ring-1 ring-zinc-700/50 transition-colors hover:bg-zinc-800"
          >
            <Squares2X2Icon class="size-5 shrink-0 text-zinc-400" />
            <div class="min-w-0">
              <p class="text-sm font-medium text-zinc-100">Showcase</p>
              <p class="text-xs text-zinc-500">
                Feature games, achievements or a note
              </p>
            </div>
            <ChevronRightIcon class="ml-auto size-4 shrink-0 text-zinc-600" />
          </NuxtLink>
        </div>
      </div>

      <!-- Save / cancel -->
      <div
        class="sticky bottom-0 -mx-8 px-8 py-4 bg-zinc-950/90 backdrop-blur-sm border-t border-zinc-800 flex items-center justify-end gap-3"
      >
        <p v-if="saveError" class="text-sm text-red-400 mr-auto">
          {{ saveError }}
        </p>
        <p v-else-if="saveOk" class="text-sm text-green-400 mr-auto">
          Saved!
        </p>
        <NuxtLink
          to="/profile"
          class="rounded-md px-4 py-2 text-sm font-medium text-zinc-400 hover:text-zinc-200 transition-colors"
        >
          Cancel
        </NuxtLink>
        <button
          class="rounded-md bg-blue-600 px-5 py-2 text-sm font-semibold text-white shadow-sm hover:bg-blue-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          :disabled="!hasChanges || saving"
          @click="save"
        >
          {{ saving ? "Saving..." : "Save changes" }}
        </button>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import {
  PhotoIcon,
  UserIcon,
  StarIcon,
  Squares2X2Icon,
  ChevronRightIcon,
} from "@heroicons/vue/24/solid";
import {
  useServerApi,
  type UserProfile,
} from "~/composables/use-server-api";
import { objectImageUrl } from "~/composables/use-object";
import ProfilePicturePicker from "~/components/ProfilePicturePicker.vue";
import {
  PROFILE_THEME_PRESETS,
  resolveAccentHex,
  useProfileTheme,
} from "~/composables/use-profile-theme";

useHead({ title: "Edit profile" });

const router = useRouter();
const api = useServerApi();

const loading = ref(true);
const profile = ref<UserProfile | null>(null);

const displayName = ref("");
const bio = ref("");
const selectedTheme = ref<string>("default");
const initial = ref({ displayName: "", bio: "", theme: "default" });

// Avatar picker open flag — the picker component owns the upload itself
// and emits the new object id back when it succeeds.
const avatarPickerOpen = ref(false);

const bannerUploading = ref(false);
const bannerPreview = ref<string | null>(null);

const saving = ref(false);
const saveError = ref<string | null>(null);
const saveOk = ref(false);

const presetList = Object.entries(PROFILE_THEME_PRESETS).map(([id, p]) => ({
  id,
  label: p.label,
  accent: p.accent,
}));

// Accent state — `selectedTheme` holds either a preset key or a #hex string.
const customColor = ref("#3b82f6");
const isCustom = computed(() => /^#[0-9a-f]{6}$/i.test(selectedTheme.value));
const { vars: previewVars } = useProfileTheme(() => selectedTheme.value);

function selectPreset(id: string) {
  selectedTheme.value = id;
}
function onColorInput() {
  selectedTheme.value = customColor.value.toLowerCase();
}
function onHexInput(e: Event) {
  let v = (e.target as HTMLInputElement).value.trim();
  if (v && !v.startsWith("#")) v = `#${v}`;
  customColor.value = v;
  if (/^#[0-9a-f]{6}$/i.test(v)) selectedTheme.value = v.toLowerCase();
}

const hasChanges = computed(
  () =>
    displayName.value !== initial.value.displayName ||
    bio.value !== initial.value.bio ||
    selectedTheme.value !== initial.value.theme,
);

function objectUrl(id: string): string {
  return objectImageUrl(id);
}

function onAvatarSelected(newObjectId: string) {
  if (profile.value) {
    profile.value.profilePictureObjectId = newObjectId;
  }
  saveError.value = null;
}

function onAvatarError(message: string) {
  saveError.value = `Avatar upload failed: ${message}`;
}

async function onBannerSelect(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;
  bannerUploading.value = true;
  try {
    const reader = new FileReader();
    reader.onload = () => {
      bannerPreview.value = reader.result as string;
    };
    reader.readAsDataURL(file);
    const result = await api.profile.uploadBanner(file);
    if (profile.value) {
      profile.value.bannerObjectId = result.bannerObjectId;
    }
    bannerPreview.value = null;
  } catch (e) {
    saveError.value =
      "Banner upload failed: " + (e instanceof Error ? e.message : String(e));
    bannerPreview.value = null;
  } finally {
    bannerUploading.value = false;
  }
}

async function save() {
  saveError.value = null;
  saveOk.value = false;
  saving.value = true;
  try {
    await api.profile.update({
      displayName: displayName.value || undefined,
      bio: bio.value || undefined,
      profileTheme: selectedTheme.value,
    });
    initial.value = {
      displayName: displayName.value,
      bio: bio.value,
      theme: selectedTheme.value,
    };
    saveOk.value = true;
    // Brief flash, then return to the profile page so the user sees the
    // updated state immediately. 800ms is enough to read "Saved!".
    setTimeout(() => {
      router.push("/profile");
    }, 800);
  } catch (e) {
    saveError.value =
      "Save failed: " + (e instanceof Error ? e.message : String(e));
  } finally {
    saving.value = false;
  }
}

onMounted(async () => {
  try {
    const me = await api.profile.me();
    profile.value = me;
    displayName.value = me.displayName || "";
    bio.value = me.bio || "";
    selectedTheme.value = me.profileTheme || "default";
    customColor.value = resolveAccentHex(me.profileTheme);
    initial.value = {
      displayName: displayName.value,
      bio: bio.value,
      theme: selectedTheme.value,
    };
  } catch (e) {
    saveError.value =
      "Couldn't load your profile. " +
      (e instanceof Error ? e.message : String(e));
  } finally {
    loading.value = false;
  }
});
</script>
