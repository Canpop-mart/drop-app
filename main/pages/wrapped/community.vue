<template>
  <WrappedDeck
    :cards="cards"
    :window="win"
    :theme="theme"
    title="The whole server"
    :loading="loading"
    @close="close"
    @update:window="setWindow"
  >
    <template #default="{ card }">
      <!-- TOGETHER -->
      <div
        v-if="card.key === 'together' && data"
        class="relative flex min-h-[430px] flex-col overflow-hidden rounded-3xl p-8 text-white"
        :style="{
          background:
            'linear-gradient(160deg, var(--accent-raw), var(--accent-deep) 70%, #0c0710)',
        }"
      >
        <p
          class="mb-auto text-xs font-bold uppercase tracking-[0.14em] text-white/80"
        >
          {{ windowLabel }} · on the server
        </p>
        <div class="text-sm font-semibold opacity-90">
          {{ data.playerCount }}
          {{ data.playerCount === 1 ? "player" : "players" }}
        </div>
        <div
          class="mt-2 font-display font-extrabold leading-none tracking-tight tabular-nums text-[clamp(52px,13vw,104px)]"
        >
          {{ hero.value }}
        </div>
        <div class="mt-2 text-xl font-bold opacity-85">
          {{ hero.unit }} together
        </div>
      </div>

      <!-- TOP GAMES -->
      <div
        v-else-if="card.key === 'topgames' && data"
        class="relative flex min-h-[430px] flex-col overflow-hidden rounded-3xl p-8"
        :style="{
          background: 'var(--accent-soft)',
          border: '1px solid var(--accent-border)',
        }"
      >
        <p
          class="text-xs font-bold uppercase tracking-[0.14em]"
          :style="{ color: 'var(--accent)' }"
        >
          What everyone played
        </p>
        <div class="mt-auto flex flex-col gap-3">
          <div
            v-for="(g, i) in data.topGames"
            :key="g.id"
            class="flex items-center gap-3"
          >
            <span
              class="w-6 font-display text-2xl font-extrabold tabular-nums"
              :style="{ color: 'var(--accent)' }"
              >{{ i + 1 }}</span
            >
            <div
              class="h-[60px] w-[46px] shrink-0 overflow-hidden rounded-lg bg-zinc-800"
            >
              <img
                v-if="g.mCoverObjectId"
                :src="objectUrl(g.mCoverObjectId)"
                class="h-full w-full object-cover"
              />
            </div>
            <div class="min-w-0 flex-1">
              <div class="truncate text-sm font-semibold text-zinc-100">
                {{ g.mName }}
              </div>
              <div class="text-[11px] text-zinc-500">
                {{ g.players }} {{ g.players === 1 ? "player" : "players" }}
              </div>
            </div>
            <span
              class="text-sm font-extrabold tabular-nums"
              :style="{ color: 'var(--accent)' }"
              >{{ fmt(g.seconds) }}</span
            >
          </div>
        </div>
      </div>

      <!-- MVP -->
      <div
        v-else-if="card.key === 'mvp' && data?.topPlayer"
        class="relative flex min-h-[430px] flex-col overflow-hidden rounded-3xl border border-zinc-800 bg-zinc-900 p-8"
      >
        <p
          class="text-xs font-bold uppercase tracking-[0.14em]"
          :style="{ color: 'var(--accent)' }"
        >
          Top player
        </p>
        <div
          class="mt-3 font-display text-4xl font-extrabold tracking-tight text-zinc-100"
        >
          {{ data.topPlayer.displayName }}
        </div>
        <div class="mt-1.5 text-sm text-zinc-400">
          {{ fmt(data.topPlayer.seconds) }} played
        </div>
        <div class="mt-auto flex gap-3">
          <div class="flex-1 rounded-2xl border border-zinc-800 bg-black/30 p-4">
            <div class="text-2xl font-extrabold tabular-nums text-zinc-100">
              {{ data.achievementsUnlocked.toLocaleString() }}
            </div>
            <div class="mt-1 text-[11px] uppercase tracking-wide text-zinc-500">
              Achievements unlocked
            </div>
          </div>
          <div class="flex-1 rounded-2xl border border-zinc-800 bg-black/30 p-4">
            <div class="text-2xl font-extrabold tabular-nums text-zinc-100">
              {{ data.newPlayers > 0 ? data.newPlayers : data.playerCount }}
            </div>
            <div class="mt-1 text-[11px] uppercase tracking-wide text-zinc-500">
              {{ data.newPlayers > 0 ? "New players" : "Players" }}
            </div>
          </div>
        </div>
      </div>

      <!-- RAREST -->
      <div
        v-else-if="card.key === 'rarest' && data?.rarest"
        class="relative flex min-h-[430px] flex-col overflow-hidden rounded-3xl p-8"
        :style="{
          background: 'var(--accent-soft)',
          border: '1px solid var(--accent-border)',
        }"
      >
        <p
          class="text-xs font-bold uppercase tracking-[0.14em]"
          :style="{ color: 'var(--accent)' }"
        >
          Rarest feat on the server
        </p>
        <div class="mt-auto">
          <div class="font-display text-3xl font-extrabold tracking-tight text-zinc-100">
            {{ data.rarest.title }}
          </div>
          <div class="mt-2 text-sm text-zinc-400">
            <span v-if="data.rarest.userDisplayName">
              Only
              <b :style="{ color: 'var(--accent)' }">{{
                data.rarest.userDisplayName
              }}</b>
              has it
            </span>
            <span v-else>Rarest unlock this window</span>
            · {{ round1(data.rarest.globalPercent) }}% of players
          </div>
        </div>
      </div>

      <!-- FINALE -->
      <div
        v-else-if="card.key === 'finale'"
        class="relative flex min-h-[430px] flex-col items-center justify-center overflow-hidden rounded-3xl p-8 text-center text-white"
        :style="{
          background:
            'linear-gradient(160deg, var(--accent-raw), var(--accent-deep) 70%, #0c0710)',
        }"
      >
        <div class="font-display text-4xl font-extrabold tracking-tight">
          The server's Wrapped.
        </div>
        <div class="mt-3 text-lg opacity-85">Everyone's stretch, in one place.</div>
      </div>
    </template>
  </WrappedDeck>
</template>

<script setup lang="ts">
import WrappedDeck from "~/components/WrappedDeck.vue";
import {
  useServerApi,
  type CommunityWrapped,
} from "~/composables/use-server-api";
import { objectImageUrl } from "~/composables/use-object";
import { formatPlaytime } from "~/composables/use-recent-games";

useHead({ title: "Community Wrapped" });

const router = useRouter();
const api = useServerApi();

const win = ref("all");
const data = ref<CommunityWrapped | null>(null);
const theme = ref<string | undefined>(undefined);
const loading = ref(true);

const windowLabel = computed(() => {
  const m: Record<string, string> = {
    all: "all-time",
    year: "this year",
    month: "this month",
    week: "this week",
  };
  return m[win.value] ?? "all-time";
});

const hero = computed(() => {
  const s = data.value?.totalSeconds ?? 0;
  if (s >= 3600)
    return { value: Math.round(s / 3600).toString(), unit: "hours played" };
  return {
    value: Math.max(1, Math.round(s / 60)).toString(),
    unit: "minutes played",
  };
});

const cards = computed(() => {
  const d = data.value;
  if (!d || d.sessionCount === 0) return [];
  const list: { key: string; title: string }[] = [
    { key: "together", title: "Together" },
  ];
  if (d.topGames.length) list.push({ key: "topgames", title: "Top games" });
  if (d.topPlayer) list.push({ key: "mvp", title: "Top player" });
  if (d.rarest) list.push({ key: "rarest", title: "Rarest feat" });
  list.push({ key: "finale", title: "Finale" });
  return list;
});

function objectUrl(o: string): string {
  return objectImageUrl(o);
}
function fmt(s: number): string {
  return formatPlaytime(s);
}
function round1(n: number): number {
  return Math.round(n * 10) / 10;
}

function close() {
  router.push("/community");
}

async function load() {
  loading.value = true;
  try {
    data.value = await api.community.wrapped(win.value);
  } catch {
    data.value = null;
  } finally {
    loading.value = false;
  }
}

function setWindow(w: string) {
  win.value = w;
  load();
}

onMounted(() => {
  // Theme the server wrapped with the viewer's own accent.
  api.profile
    .me()
    .then((me) => (theme.value = me.profileTheme))
    .catch(() => {});
  load();
});
</script>
