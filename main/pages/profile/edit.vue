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

      <!-- Avatar -->
      <div class="mb-6">
        <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">
          Avatar
        </p>
        <div class="flex items-center gap-4">
          <img
            v-if="avatarPreview || profile.profilePictureObjectId"
            :src="
              avatarPreview ||
              objectUrl(profile.profilePictureObjectId!)
            "
            class="size-20 rounded-full object-cover ring-2 ring-zinc-700"
          />
          <div
            v-else
            class="size-20 rounded-full bg-zinc-800 flex items-center justify-center ring-2 ring-zinc-700"
          >
            <UserIcon class="size-10 text-zinc-500" />
          </div>
          <label
            class="inline-flex items-center gap-x-2 rounded-md bg-zinc-800/50 px-3 py-1.5 text-sm font-medium text-zinc-200 cursor-pointer hover:bg-zinc-800 transition-colors"
            :class="{ 'opacity-50 pointer-events-none': avatarUploading }"
          >
            <PhotoIcon class="size-4" />
            {{ avatarUploading ? "Uploading..." : "Change avatar" }}
            <input
              type="file"
              accept="image/*"
              class="hidden"
              @change="onAvatarSelect"
            />
          </label>
        </div>
      </div>

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

      <!-- Profile theme -->
      <div class="mb-8">
        <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">
          Profile theme
        </p>
        <div class="grid grid-cols-3 sm:grid-cols-6 gap-3">
          <button
            v-for="theme in profileThemes"
            :key="theme.id"
            class="flex flex-col items-center gap-2 p-3 rounded-xl border-2 transition-all"
            :class="
              selectedTheme === theme.id
                ? 'border-blue-500 bg-zinc-800/80'
                : 'border-transparent bg-zinc-800/30 hover:border-zinc-600'
            "
            @click="selectedTheme = theme.id"
          >
            <div
              class="w-full h-7 rounded-md"
              :style="{
                background: `linear-gradient(135deg, ${theme.from}, ${theme.to})`,
              }"
            />
            <span
              class="text-[10px] font-medium"
              :class="
                selectedTheme === theme.id ? 'text-blue-400' : 'text-zinc-500'
              "
            >
              {{ theme.label }}
            </span>
          </button>
        </div>
        <p class="text-xs text-zinc-500 mt-2">
          Sets the banner fallback colour when you haven't uploaded one.
        </p>
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
import { PhotoIcon, UserIcon } from "@heroicons/vue/24/solid";
import {
  useServerApi,
  type UserProfile,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";

useHead({ title: "Edit profile" });

const router = useRouter();
const api = useServerApi();

const loading = ref(true);
const profile = ref<UserProfile | null>(null);

const displayName = ref("");
const bio = ref("");
const selectedTheme = ref<string>("default");
const initial = ref({ displayName: "", bio: "", theme: "default" });

const avatarUploading = ref(false);
const avatarPreview = ref<string | null>(null);

const bannerUploading = ref(false);
const bannerPreview = ref<string | null>(null);

const saving = ref(false);
const saveError = ref<string | null>(null);
const saveOk = ref(false);

const profileThemes = [
  { id: "default", label: "Blue", from: "#3b82f6", to: "#1e3a8a" },
  { id: "ocean", label: "Ocean", from: "#0ea5e9", to: "#1e3a8a" },
  { id: "sunset", label: "Sunset", from: "#f97316", to: "#7c2d12" },
  { id: "forest", label: "Forest", from: "#22c55e", to: "#14532d" },
  { id: "purple", label: "Purple", from: "#a855f7", to: "#581c87" },
  { id: "rose", label: "Rose", from: "#f43f5e", to: "#881337" },
] as const;

const hasChanges = computed(
  () =>
    displayName.value !== initial.value.displayName ||
    bio.value !== initial.value.bio ||
    selectedTheme.value !== initial.value.theme,
);

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

async function onAvatarSelect(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;
  avatarUploading.value = true;
  try {
    // Local preview first so the UI feels responsive — upload runs in
    // parallel and the preview is replaced by the server URL once
    // profile state refreshes.
    const reader = new FileReader();
    reader.onload = () => {
      avatarPreview.value = reader.result as string;
    };
    reader.readAsDataURL(file);
    const result = await api.profile.uploadAvatar(file);
    if (profile.value) {
      profile.value.profilePictureObjectId = result.profilePictureObjectId;
    }
    avatarPreview.value = null;
  } catch (e) {
    saveError.value =
      "Avatar upload failed: " + (e instanceof Error ? e.message : String(e));
    avatarPreview.value = null;
  } finally {
    avatarUploading.value = false;
  }
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
    initial.value = {
      displayName: displayName.value,
      bio: bio.value,
      theme: selectedTheme.value,
    };
  } catch (e) {
    saveError.value =
      "Couldn't load your profile — " +
      (e instanceof Error ? e.message : String(e));
  } finally {
    loading.value = false;
  }
});
</script>
