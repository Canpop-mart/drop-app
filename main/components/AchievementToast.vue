<template>
  <Teleport to="body">
    <TransitionGroup
      tag="div"
      class="fixed bottom-6 right-6 z-[9999] flex flex-col gap-3 pointer-events-none"
      enter-active-class="transition-all duration-500 ease-out"
      enter-from-class="translate-x-full opacity-0"
      enter-to-class="translate-x-0 opacity-100"
      leave-active-class="transition-all duration-300 ease-in"
      leave-from-class="translate-x-0 opacity-100"
      leave-to-class="translate-x-full opacity-0"
    >
      <div
        v-for="toast in toasts"
        :key="toast.id"
        class="pointer-events-auto flex items-center gap-3 px-4 py-3 bg-zinc-900 ring-1 ring-yellow-500/30 rounded-xl shadow-2xl shadow-yellow-500/10 max-w-sm"
      >
        <img
          v-if="toast.iconUrl"
          :src="toast.iconUrl"
          class="size-12 rounded-lg shrink-0"
        />
        <div
          v-else
          class="size-12 rounded-lg shrink-0 bg-yellow-500/10 flex items-center justify-center"
        >
          <svg
            class="size-6 text-yellow-400"
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 24 24"
            fill="currentColor"
          >
            <path
              fill-rule="evenodd"
              d="M5.166 2.621v.858c-1.035.148-2.059.33-3.071.543a.75.75 0 0 0-.584.859 6.753 6.753 0 0 0 6.138 5.6 6.73 6.73 0 0 0 2.743 1.346A6.707 6.707 0 0 1 9.279 15H8.54c-1.036 0-1.875.84-1.875 1.875V19.5h-.75a2.25 2.25 0 0 0-2.25 2.25c0 .414.336.75.75.75h15.19a.75.75 0 0 0 .75-.75 2.25 2.25 0 0 0-2.25-2.25h-.75v-2.625c0-1.036-.84-1.875-1.875-1.875h-.739a6.707 6.707 0 0 1-1.112-3.173 6.73 6.73 0 0 0 2.743-1.347 6.753 6.753 0 0 0 6.139-5.6.75.75 0 0 0-.585-.858 47.077 47.077 0 0 0-3.07-.543V2.62a.75.75 0 0 0-.658-.744 49.22 49.22 0 0 0-6.093-.377c-2.063 0-4.096.128-6.093.377a.75.75 0 0 0-.657.744Zm0 2.629c0 1.196.312 2.32.857 3.294A5.266 5.266 0 0 1 3.16 5.337a45.6 45.6 0 0 1 2.006-.343v.256Zm13.5 0v-.256c.674.1 1.343.214 2.006.343a5.265 5.265 0 0 1-2.863 3.207 6.72 6.72 0 0 0 .857-3.294Z"
              clip-rule="evenodd"
            />
          </svg>
        </div>
        <div class="flex-1 min-w-0">
          <p
            class="text-xs font-medium text-yellow-400 uppercase tracking-wide"
          >
            Achievement Unlocked
          </p>
          <p class="text-sm font-semibold text-zinc-100 truncate">
            {{ toast.title }}
          </p>
          <p v-if="toast.description" class="text-xs text-zinc-400 truncate">
            {{ toast.description }}
          </p>
        </div>
      </div>
    </TransitionGroup>
  </Teleport>
</template>

<script setup lang="ts">
import { listen } from "@tauri-apps/api/event";

interface AchievementToastItem {
  id: string;
  title: string;
  description?: string;
  iconUrl?: string;
}

interface AchievementPayload {
  id: string;
  title: string;
  description: string;
  iconUrl: string;
}

const toasts = ref<AchievementToastItem[]>([]);

// Listen for achievement_unlocked events from the Rust backend
const unlisten = listen<AchievementPayload>("achievement_unlocked", (event) => {
  const data = event.payload;
  const toast: AchievementToastItem = {
    id: `${data.id}-${Date.now()}`,
    title: data.title,
    description: data.description || undefined,
    iconUrl: data.iconUrl || undefined,
  };

  toasts.value.push(toast);

  // Auto-dismiss after 6 seconds
  setTimeout(() => {
    toasts.value = toasts.value.filter((t) => t.id !== toast.id);
  }, 6000);
});

onUnmounted(async () => {
  (await unlisten)();
});
</script>
