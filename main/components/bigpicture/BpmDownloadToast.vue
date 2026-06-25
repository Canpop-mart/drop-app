<template>
  <Teleport to="body">
    <Transition name="dl-toast">
      <div
        v-if="current"
        class="fixed bottom-20 right-6 z-[60] pointer-events-auto"
      >
        <button
          type="button"
          class="flex items-center gap-3 px-4 py-3 rounded-xl shadow-2xl bg-zinc-900/95 border border-green-500/40 text-zinc-100 min-w-[18rem] max-w-[22rem] transition-colors hover:bg-zinc-800"
          @click="goToLibrary"
        >
          <div class="shrink-0 size-9 rounded-lg bg-green-500/15 border border-green-500/40 flex items-center justify-center">
            <svg class="size-5 text-green-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M20 6 9 17l-5-5" />
            </svg>
          </div>
          <div class="flex-1 min-w-0 text-left">
            <div class="text-xs font-semibold text-green-400 uppercase tracking-wide">
              Installed
            </div>
            <div class="text-sm font-medium text-zinc-100 truncate">
              {{ current.name }}
            </div>
          </div>
          <div class="shrink-0 text-[10px] text-zinc-500 uppercase tracking-wide">
            A: Open
          </div>
        </button>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { useCompletedDownloads, useQueueState } from "~/composables/downloads";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { invoke } from "@tauri-apps/api/core";

interface ToastEntry {
  gameId: string;
  name: string;
  at: number;
}

const completed = useCompletedDownloads();
const queue = useQueueState();
const gamepad = useGamepad();
const router = useRouter();
const route = useRoute();
const focusNav = useFocusNavigation();

const toastQueue = ref<ToastEntry[]>([]);
const current = ref<ToastEntry | null>(null);
let dismissTimer: ReturnType<typeof setTimeout> | null = null;
let lastSeenAt = 0;
const unsub: (() => void)[] = [];

const TOAST_DURATION_MS = 5000;

onMounted(() => {
  // Initialize the "seen" high-water mark so existing entries don't
  // trigger toasts on first mount.
  lastSeenAt = completed.value.at(0)?.completedAt ?? Date.now();

  unsub.push(
    gamepad.onButton(GamepadButton.South, () => {
      if (current.value) {
        goToLibrary();
      }
    }),
  );
  unsub.push(
    gamepad.onButton(GamepadButton.East, () => {
      if (current.value) {
        dismissCurrent();
      }
    }),
  );
});

onUnmounted(() => {
  if (dismissTimer) clearTimeout(dismissTimer);
  for (const u of unsub) u();
});

// Detect new completed entries by watching for items whose completedAt is
// newer than lastSeenAt.
watch(
  completed,
  async (entries) => {
    const fresh = entries.filter((e) => e.completedAt > lastSeenAt);
    if (!fresh.length) return;
    for (const entry of fresh) {
      if (entry.completedAt > lastSeenAt) lastSeenAt = entry.completedAt;
      const name = await resolveGameName(entry.gameId);
      toastQueue.value.push({ gameId: entry.gameId, name, at: entry.completedAt });
    }
    if (!current.value) showNext();
  },
  { deep: true },
);

async function resolveGameName(gameId: string): Promise<string> {
  try {
    const result: any = await invoke("fetch_game", { gameId });
    return result?.game?.mName ?? result?.mName ?? gameId;
  } catch {
    return gameId;
  }
}

function showNext() {
  if (dismissTimer) {
    clearTimeout(dismissTimer);
    dismissTimer = null;
  }
  const next = toastQueue.value.shift();
  if (!next) {
    current.value = null;
    return;
  }
  current.value = next;
  dismissTimer = setTimeout(() => {
    showNext();
  }, TOAST_DURATION_MS);
}

function dismissCurrent() {
  if (!current.value) return;
  showNext();
}

function goToLibrary() {
  const id = current.value?.gameId;
  const origin = route.path;
  dismissCurrent();
  if (id) {
    const target = `/bigpicture/library/${id}`;
    // Record where the user was so pressing B returns them there instead
    // of bouncing to /bigpicture/library.
    focusNav.setRouteState("backTo", origin, target);
    router.push(target);
  }
}

// Also dismiss if the user navigates away so the toast doesn't linger on
// an unrelated page forever.
watch(
  () => queue.value.queue.length,
  () => {
    // no-op — kept so downloads state changes are reactive here
  },
);
</script>

<style scoped>
.dl-toast-enter-active,
.dl-toast-leave-active {
  transition: transform 0.25s cubic-bezier(0.34, 1.56, 0.64, 1), opacity 0.2s ease;
}
.dl-toast-enter-from,
.dl-toast-leave-to {
  opacity: 0;
  transform: translateY(20px);
}
</style>
