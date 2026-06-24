<template>
  <div class="h-full flex flex-col overflow-y-auto">
    <!-- Console header — a calm themed banner: name, maker, blurb, count. -->
    <div
      class="relative overflow-hidden border-b border-zinc-800/40 bg-gradient-to-br from-zinc-900 via-zinc-950 to-zinc-900 px-8 xl:px-12 pt-8 pb-7"
    >
      <NuxtLink
        to="/library"
        class="inline-flex items-center gap-1.5 text-sm text-zinc-400 transition-colors hover:text-zinc-200"
      >
        <ChevronLeftIcon class="size-4" />
        Library
      </NuxtLink>
      <div class="mt-4 flex flex-wrap items-end justify-between gap-4">
        <div class="min-w-0">
          <p
            v-if="meta?.maker"
            class="mb-1 text-[11px] font-semibold uppercase tracking-[0.25em] text-blue-300"
          >
            {{ meta.maker }}
          </p>
          <h1
            class="font-display text-4xl font-bold leading-none text-white drop-shadow-lg"
          >
            {{ meta?.name ?? "Console" }}
          </h1>
          <p v-if="meta?.blurb" class="mt-3 max-w-xl text-sm text-zinc-400">
            {{ meta.blurb }}
          </p>
        </div>
        <p class="text-sm text-zinc-400">
          {{ consoleEntries.length }} game{{
            consoleEntries.length === 1 ? "" : "s"
          }}
          <span v-if="installedCount > 0">
            ·
            <span class="text-green-500">{{ installedCount }} installed</span>
          </span>
        </p>
      </div>
    </div>

    <div class="flex-1 px-8 xl:px-12 py-8 pb-16">
      <div
        v-if="loading"
        class="flex items-center justify-center py-20 text-sm text-zinc-500"
      >
        Loading…
      </div>
      <div
        v-else-if="consoleEntries.length === 0"
        class="flex flex-col items-center justify-center py-20 text-center"
      >
        <p class="text-sm text-zinc-400">
          No games here yet for this console.
        </p>
        <NuxtLink
          to="/library"
          class="mt-3 text-sm text-blue-400 hover:text-blue-300"
        >
          Back to library
        </NuxtLink>
      </div>
      <LibraryGrid
        v-else
        :entries="consoleEntries"
        :last-played-map="lastPlayedMap"
        show-hover-action
        @select="goToGame"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ChevronLeftIcon } from "@heroicons/vue/24/outline";
import { invoke } from "@tauri-apps/api/core";
import { useGame } from "~/composables/game";
import {
  useServerApi,
  type ConsoleGroup,
} from "~/composables/use-server-api";
import type { Game, GameStatus } from "~/types";
import { InstalledType } from "~/types";
import LibraryGrid from "~/components/LibraryGrid.vue";

interface LibraryEntry {
  game: Game;
  status: GameStatus | null;
  installed: boolean;
  updateAvailable: boolean;
}

type FetchLibraryResponse = {
  library: Game[];
  collections: Array<{ entries: Array<{ game: Game }> }>;
  other: Game[];
  missing: Game[];
};

const route = useRoute();
const router = useRouter();
const api = useServerApi();

const consoleId = computed(() => String(route.params.id));
const meta = ref<Omit<ConsoleGroup, "gameIds"> | null>(null);
const consoleEntries = ref<LibraryEntry[]>([]);
const lastPlayedMap = ref<Map<string, string>>(new Map());
const loading = ref(true);

const installedCount = computed(
  () => consoleEntries.value.filter((e) => e.installed).length,
);

function goToGame(gameId: string) {
  invoke("fetch_game", { gameId }).catch(() => {});
  router.push(`/library/${gameId}`);
}

async function load() {
  loading.value = true;
  try {
    const [{ consoles }, lib] = await Promise.all([
      api.emulation.consoles(),
      invoke<FetchLibraryResponse>("fetch_library", { hardRefresh: false }),
    ]);

    const group = consoles.find((c) => c.id === consoleId.value);
    if (!group) {
      meta.value = null;
      consoleEntries.value = [];
      return;
    }
    const { gameIds, ...rest } = group;
    meta.value = rest;

    const wanted = new Set(gameIds);
    const games = [
      ...lib.library,
      ...lib.collections.flatMap((c) => c.entries.map((e) => e.game)),
      ...lib.other,
    ]
      .filter((g, i, a) => a.findIndex((x) => x.id === g.id) === i)
      .filter((g) => wanted.has(g.id));

    const built: LibraryEntry[] = [];
    const batchSize = 5;
    for (let i = 0; i < games.length; i += batchSize) {
      const batch = games.slice(i, i + batchSize);
      const results = await Promise.all(
        batch.map((g) => useGame(g.id).catch(() => null)),
      );
      for (let j = 0; j < batch.length; j++) {
        const r = results[j];
        const game = batch[j];
        if (!r) {
          built.push({
            game,
            status: null,
            installed: false,
            updateAvailable: false,
          });
          continue;
        }
        const status = r.status.value;
        const installed =
          status.type === "Installed" &&
          status.install_type.type === InstalledType.Installed;
        const updateAvailable =
          status.type === "Installed" ? status.update_available : false;
        built.push({ game, status, installed, updateAvailable });
      }
    }
    built.sort((a, b) => a.game.mName.localeCompare(b.game.mName));
    consoleEntries.value = built;
  } catch (e) {
    console.warn("[console] load failed:", e);
    consoleEntries.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(load);
</script>
