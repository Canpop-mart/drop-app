<template>
  <div class="flex h-full flex-col overflow-y-auto bg-zinc-950">
    <!-- Compact platform identity. Emulator management and games are separate
         below, so the header only needs to orient the player. -->
    <div
      class="relative overflow-hidden border-b border-zinc-800/50 bg-gradient-to-r from-zinc-900/80 to-zinc-950 px-8 py-5 xl:px-12"
    >
      <NuxtLink
        to="/library"
        class="inline-flex items-center gap-1.5 text-sm text-zinc-400 transition-colors hover:text-zinc-200"
      >
        <ChevronLeftIcon class="size-4" />
        Library
      </NuxtLink>
      <div class="mt-3 flex flex-wrap items-center justify-between gap-4">
        <div class="flex min-w-0 items-center gap-5">
          <img
            v-if="art"
            :src="art.render"
            :alt="art.name"
            class="h-16 w-auto max-w-[8rem] shrink-0 object-contain drop-shadow-lg"
            :style="art.pixel ? 'image-rendering: pixelated' : ''"
          />
          <div class="min-w-0">
            <img
              v-if="art?.logo"
              :src="art.logo"
              :alt="art.name"
              class="w-auto max-w-[22rem] object-contain"
              :class="art.big ? 'h-12' : 'h-9'"
              :style="
                art.whiten
                  ? 'filter: brightness(0) invert(1) drop-shadow(0 2px 3px rgba(0,0,0,0.4))'
                  : 'filter: drop-shadow(0 2px 4px rgba(0,0,0,0.5))'
              "
            />
            <h1
              v-else
              class="font-display text-3xl font-bold leading-none text-white drop-shadow-lg"
            >
              {{ displayName }}
            </h1>
            <p v-if="meta?.blurb" class="mt-2 max-w-2xl text-sm text-zinc-400">
              {{ meta.blurb }}
            </p>
          </div>
        </div>
        <p class="text-sm text-zinc-400">
          {{ gameEntries.length }} game{{
            gameEntries.length === 1 ? "" : "s"
          }}
          <span v-if="installedGameCount > 0">
            ·
            <span class="text-green-500">{{ installedGameCount }} installed</span>
          </span>
        </p>
      </div>
    </div>

    <div class="flex-1 px-8 py-6 pb-16 xl:px-12">
      <div
        v-if="loading"
        class="flex items-center justify-center py-20 text-sm text-zinc-500"
      >
        Loading…
      </div>
      <SectionError
        v-else-if="loadError"
        title="Couldn't load this console"
        :detail="loadError"
        @retry="load"
      />
      <div
        v-else-if="gameEntries.length === 0 && emulatorEntries.length === 0"
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
      <template v-else>
        <section v-if="emulatorEntries.length > 0" class="mb-8">
          <div class="mb-3 flex items-baseline gap-3">
            <h2 class="font-display text-lg font-semibold text-zinc-100">Emulator</h2>
            <span class="text-xs text-zinc-500">Runs this platform</span>
          </div>
          <div class="flex flex-wrap gap-3">
            <div
              v-for="entry in emulatorEntries"
              :key="entry.game.id"
              class="flex min-w-[22rem] items-center gap-4 rounded-xl border border-zinc-800 bg-zinc-900/45 p-3"
            >
              <img
                v-if="entry.game.mCoverObjectId"
                :src="useObject(entry.game.mCoverObjectId)"
                :alt="entry.game.mName"
                class="size-14 rounded-lg object-cover"
              />
              <div class="min-w-0 flex-1">
                <p class="truncate font-semibold text-zinc-100">{{ entry.game.mName }}</p>
                <p class="mt-0.5 text-xs" :class="entry.installed ? 'text-green-400' : 'text-zinc-500'">
                  {{ entry.installed ? "Installed" : "Not installed" }}
                </p>
              </div>
              <NuxtLink
                :to="entry.installed ? `/library/emulator/${entry.game.id}` : `/library/${entry.game.id}`"
                class="rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-200 hover:bg-zinc-700"
              >
                {{ entry.installed ? "Manage" : "View" }}
              </NuxtLink>
            </div>
          </div>
        </section>

        <section v-if="gameEntries.length > 0">
          <div class="mb-4 flex items-baseline gap-3">
            <h2 class="font-display text-xl font-semibold text-zinc-100">Games</h2>
            <span class="text-xs tabular-nums text-zinc-500">{{ gameEntries.length }}</span>
          </div>
          <LibraryGrid
            :entries="gameEntries"
            :last-played-map="lastPlayedMap"
            :boxart-theme="art?.boxartTheme ?? null"
            dense
            show-hover-action
            @select="goToGame"
          />
        </section>
      </template>
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
import { consoleArt } from "~/composables/console-art";
import { withEmulatorsFirst } from "~/composables/console-emulators";

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

// Console render + canonical display name, resolved from the library name.
const art = computed(() => (meta.value ? consoleArt(meta.value.name) : null));
const displayName = computed(
  () => art.value?.name ?? meta.value?.name ?? "Console",
);
const consoleEntries = ref<LibraryEntry[]>([]);
const emulatorGameIds = ref<Set<string>>(new Set());
const lastPlayedMap = ref<Map<string, string>>(new Map());
const loading = ref(true);
// Set when the console list or the library couldn't be read, so a failure
// isn't shown as "No games here yet".
const loadError = ref<string | null>(null);

const emulatorEntries = computed(() =>
  consoleEntries.value.filter((e) => emulatorGameIds.value.has(e.game.id)),
);
const gameEntries = computed(() =>
  consoleEntries.value.filter((e) => !emulatorGameIds.value.has(e.game.id)),
);
const installedGameCount = computed(
  () => gameEntries.value.filter((e) => e.installed).length,
);

function goToGame(gameId: string) {
  invoke("fetch_game", { gameId }).catch(() => {});
  router.push(`/library/${gameId}`);
}

async function load() {
  loading.value = true;
  loadError.value = null;
  try {
    const [{ consoles }, lib] = await Promise.all([
      api.emulation.consoles(),
      invoke<FetchLibraryResponse>("fetch_library", { hardRefresh: false }),
    ]);

    const group = consoles.find((c) => c.id === consoleId.value);
    if (!group) {
      meta.value = null;
      consoleEntries.value = [];
      emulatorGameIds.value = new Set();
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

    // The console's emulator goes first, ahead of its ROMs. It usually isn't
    // in the player's library, so it's fetched on its own; one that fails to
    // load is simply left out.
    const emulators = new Map<string, LibraryEntry>();
    emulatorGameIds.value = new Set(group.emulatorIds ?? []);
    for (const id of group.emulatorIds ?? []) {
      const r = await useGame(id).catch(() => null);
      if (!r) continue;
      const status = r.status.value;
      emulators.set(id, {
        game: r.game,
        status,
        installed:
          status.type === "Installed" &&
          status.install_type.type === InstalledType.Installed,
        updateAvailable:
          status.type === "Installed" ? status.update_available : false,
      });
    }
    consoleEntries.value = withEmulatorsFirst(
      built,
      group.emulatorIds,
      emulators,
    );
  } catch (e) {
    console.warn("[console] load failed:", e);
    consoleEntries.value = [];
    emulatorGameIds.value = new Set();
    loadError.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
}

onMounted(load);
</script>
