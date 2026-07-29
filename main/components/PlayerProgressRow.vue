<!--
  A person + what they're up to. Generalises the inline leaderboard row so the
  same shape serves the Top Players list, the "playing now" presence band, and
  the per-game "who plays this" list.

  Subtitle priority: a live `nowPlaying` game name (green dot, clickable) wins;
  otherwise a plain `subtitle` (e.g. "12h · 8/20 achievements"). `stat` is the
  right-aligned figure (e.g. "100h"). `rank` shows a leading position number.
-->
<template>
  <div class="flex items-center gap-3 px-3 py-2.5">
    <span
      v-if="rank != null"
      class="w-4 shrink-0 text-center text-xs font-bold tabular-nums text-zinc-500"
      >{{ rank }}</span
    >
    <button
      class="shrink-0 rounded-full transition-transform hover:scale-110"
      @click="$emit('select-user')"
    >
      <Avatar
        :object-id="avatarObjectId"
        :name="name"
        :size="28"
        :presence="!!nowPlaying"
      />
    </button>
    <div class="min-w-0 flex-1">
      <div class="flex min-w-0 items-center gap-1 text-sm">
        <button
          class="truncate text-left font-medium text-zinc-100 transition-colors hover:text-blue-400"
          @click="$emit('select-user')"
        >
          {{ name }}
        </button>
        <span
          v-if="crown"
          :title="crownTitle"
          class="shrink-0 cursor-help text-amber-300"
          aria-label="Today's MVP"
          >👑</span
        >
      </div>
      <button
        v-if="nowPlaying"
        class="mt-0.5 flex min-w-0 max-w-full items-center gap-1.5 text-[11px] text-zinc-400 transition-colors hover:text-zinc-200"
        @click="$emit('select-game')"
      >
        <span class="pulse-dot size-1.5 shrink-0 rounded-full bg-green-500" />
        <span class="truncate text-left">{{ nowPlaying }}</span>
      </button>
      <div
        v-else-if="subtitle"
        class="mt-0.5 truncate text-[11px] text-zinc-500"
      >
        {{ subtitle }}
      </div>
    </div>
    <span
      v-if="stat"
      class="shrink-0 text-xs font-medium tabular-nums text-zinc-400"
      >{{ stat }}</span
    >
  </div>
</template>

<script setup lang="ts">
import Avatar from "./Avatar.vue";

defineProps<{
  name: string;
  avatarObjectId?: string | null;
  rank?: number | null;
  crown?: boolean;
  crownTitle?: string;
  /** Game name when the user has a live session — renders the green line. */
  nowPlaying?: string | null;
  /** Fallback subtitle when not live (e.g. progress text). */
  subtitle?: string | null;
  /** Right-aligned figure (e.g. "100h"). */
  stat?: string | null;
}>();

defineEmits<{ (e: "select-user"): void; (e: "select-game"): void }>();
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
