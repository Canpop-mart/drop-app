<template>
  <section
    v-if="event"
    class="mb-8 rounded-2xl bg-gradient-to-br from-amber-900/30 via-zinc-900/60 to-rose-900/20 ring-1 ring-amber-500/20 px-5 py-4"
  >
    <div class="flex items-center gap-2 mb-3">
      <ClockIcon class="size-4 text-amber-300/80" />
      <span
        class="text-[10px] tracking-[0.2em] uppercase text-amber-300/80 font-medium"
      >
        Drop Time Machine
      </span>
    </div>

    <div class="flex items-center gap-4">
      <img
        v-if="event.user.avatarObjectId"
        :src="avatarUrl"
        class="size-10 rounded-full object-cover ring-1 ring-amber-400/30 shrink-0"
      />
      <div
        v-else
        class="size-10 rounded-full bg-zinc-700 flex items-center justify-center ring-1 ring-amber-400/30 shrink-0"
      >
        <span class="text-xs font-bold text-zinc-400">
          {{ event.user.displayName[0]?.toUpperCase() }}
        </span>
      </div>

      <div class="flex-1 min-w-0">
        <p class="text-xs text-amber-300/90 font-medium">
          {{ anniversaryLabel }}
        </p>
        <p class="text-sm text-zinc-100 truncate">
          <span class="font-medium">{{ event.user.displayName }}</span>
          <span class="text-zinc-400"> · {{ event.detail }} · </span>
          <span class="text-amber-200">{{ event.game.name }}</span>
        </p>
      </div>

      <img
        v-if="event.game.coverObjectId"
        :src="coverUrl"
        class="h-14 w-10 rounded object-cover shrink-0"
        loading="lazy"
      />
    </div>
  </section>
</template>

<script setup lang="ts">
import { ClockIcon } from "@heroicons/vue/24/solid";
import type { TimeMachineEvent } from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";

const props = defineProps<{
  event: TimeMachineEvent | null;
}>();

const anniversaryLabel = computed(() => {
  if (!props.event) return "";
  switch (props.event.daysAgo) {
    case 365:
      return "A year ago today";
    case 180:
      return "Six months ago today";
    case 90:
      return "Three months ago today";
    case 30:
      return "One month ago today";
    default:
      return `${props.event.daysAgo} days ago today`;
  }
});

const avatarUrl = computed(() =>
  props.event?.user.avatarObjectId
    ? serverUrl(`api/v1/object/${props.event.user.avatarObjectId}`)
    : "",
);

const coverUrl = computed(() =>
  props.event?.game.coverObjectId
    ? serverUrl(`api/v1/object/${props.event.game.coverObjectId}`)
    : "",
);
</script>
