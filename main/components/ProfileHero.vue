<!--
  Profile hero — weaves banner, avatar, identity and headline stats into one
  themed unit (they used to be stacked). Reads the accent CSS custom properties
  set by the page root (ProfileThemeRoot / the page's :style="vars"), so the
  whole thing re-accents when the user changes their theme. `editable` shows
  the Edit-profile action (own profile only).
-->
<template>
  <div class="relative">
    <!-- Banner -->
    <div class="relative h-60">
      <img
        v-if="profile.bannerObjectId"
        :src="objectUrl(profile.bannerObjectId)"
        alt=""
        class="h-full w-full object-cover"
      />
      <div
        v-else
        class="h-full w-full"
        :style="{ background: 'var(--profile-banner)' }"
      />
      <!-- soft accent glow + bottom scrim into the page -->
      <div
        class="absolute inset-0"
        :style="{
          background:
            'radial-gradient(120% 140% at 78% 0%, var(--accent-soft), transparent 55%)',
        }"
      />
      <div
        class="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/40 to-transparent"
      />

      <div class="absolute right-4 top-4 z-10 flex gap-2">
        <NuxtLink
          :to="`/wrapped/${profile.id}`"
          class="inline-flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-semibold shadow-lg transition-transform hover:-translate-y-0.5"
          :style="{
            background: 'var(--accent)',
            color: 'var(--accent-contrast)',
          }"
        >
          <SparklesIcon class="size-4" />
          Wrapped
        </NuxtLink>
        <NuxtLink
          v-if="editable"
          to="/profile/edit"
          class="inline-flex items-center gap-2 rounded-lg bg-zinc-900/70 px-4 py-2 text-sm font-semibold text-zinc-100 ring-1 ring-zinc-700/60 backdrop-blur-sm transition-colors hover:bg-zinc-800"
        >
          <PencilSquareIcon class="size-4" />
          Edit profile
        </NuxtLink>
      </div>
    </div>

    <!-- Identity -->
    <div class="relative z-10 mx-auto max-w-5xl px-8">
      <div class="-mt-[4.5rem] flex items-end gap-5">
        <div class="shrink-0 rounded-full border-4 border-zinc-950 shadow-2xl">
          <Avatar
            :object-id="profile.profilePictureObjectId"
            :name="profile.displayName || profile.username"
            :size="128"
            :presence="!!presence"
          />
        </div>
        <div class="min-w-0 flex-1 pb-2">
          <button
            v-if="presence"
            class="mb-2 inline-flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-[0.16em]"
            :style="{ color: 'var(--accent)' }"
            @click="$emit('go-to-game', presence.gameId)"
          >
            <span class="pulse-dot size-2 rounded-full bg-green-500" />
            Playing {{ presence.gameName }}
          </button>
          <h1
            class="relative inline-block font-display text-3xl font-bold text-zinc-100"
          >
            {{ profile.displayName || profile.username }}
            <span
              class="absolute -bottom-2 left-0 h-1 w-14 rounded-full"
              :style="{ background: 'var(--accent)' }"
            />
          </h1>
          <p class="mt-3.5 text-sm text-zinc-400">@{{ profile.username }}</p>
        </div>
      </div>

      <p v-if="profile.bio" class="mt-5 max-w-2xl text-sm text-zinc-300">
        {{ profile.bio }}
      </p>

      <!-- Headline stats as accent-tinted pills -->
      <div v-if="stats" class="mt-6 flex flex-wrap gap-3">
        <div
          v-for="s in statPills"
          :key="s.label"
          class="flex min-w-[150px] flex-1 items-center gap-3 rounded-2xl p-4"
          :style="{
            background: 'var(--accent-soft)',
            border: '1px solid var(--accent-border)',
          }"
        >
          <span
            class="flex size-10 items-center justify-center rounded-xl bg-zinc-950/40"
            :style="{ color: 'var(--accent)' }"
          >
            <component :is="s.icon" class="size-5" />
          </span>
          <div>
            <div class="text-xl font-bold leading-none tabular-nums text-zinc-100">
              {{ s.value }}
            </div>
            <div class="mt-1 text-[10px] uppercase tracking-wide text-zinc-500">
              {{ s.label }}
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  PencilSquareIcon,
  ClockIcon,
  PlayIcon,
  TrophyIcon,
  SparklesIcon,
} from "@heroicons/vue/24/solid";
import Avatar from "~/components/Avatar.vue";
import type { UserProfile, UserStats } from "~/composables/use-server-api";
import { objectImageUrl } from "~/composables/use-object";
import { formatPlaytime } from "~/composables/use-recent-games";

const props = defineProps<{
  profile: UserProfile;
  stats?: UserStats | null;
  presence?: { gameId: string; gameName: string } | null;
  editable?: boolean;
}>();

defineEmits<{ (e: "go-to-game", gameId: string): void }>();

const statPills = computed(() => {
  const s = props.stats;
  if (!s) return [];
  return [
    {
      label: "Total playtime",
      value: formatPlaytime(s.totalPlaytimeSeconds),
      icon: ClockIcon,
    },
    {
      label: "Games played",
      value: s.gamesPlayed.toLocaleString(),
      icon: PlayIcon,
    },
    {
      label: "Achievements",
      value: s.achievementsUnlocked.toLocaleString(),
      icon: TrophyIcon,
    },
  ];
});

function objectUrl(id: string): string {
  return objectImageUrl(id);
}
</script>

<style scoped>
.pulse-dot {
  animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
}
@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
}
</style>
