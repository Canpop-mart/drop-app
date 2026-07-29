<template>
  <WrappedDeck
    :cards="cards"
    :window="win"
    :theme="theme"
    :title="title"
    :loading="loading"
    @close="close"
    @update:window="setWindow"
  >
    <template #default="{ card }">
      <!-- OVERVIEW -->
      <div
        v-if="card.key === 'overview' && data"
        class="relative flex min-h-[430px] flex-col overflow-hidden rounded-3xl p-8 text-white"
        :style="{
          background:
            'linear-gradient(160deg, var(--accent-raw), var(--accent-deep) 70%, #0c0710)',
        }"
      >
        <p
          class="mb-auto text-xs font-bold uppercase tracking-[0.14em] text-white/80"
        >
          Your {{ windowLabel }} · wrapped
        </p>
        <div class="text-sm font-semibold opacity-90">
          {{ data.displayName }}
        </div>
        <div
          class="mt-2 font-display font-extrabold leading-none tracking-tight tabular-nums text-[clamp(56px,14vw,110px)]"
        >
          {{ hero.value }}
        </div>
        <div class="mt-2 text-xl font-bold opacity-85">{{ hero.unit }}</div>
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
          Your top games
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
            <span
              class="min-w-0 flex-1 truncate text-sm font-semibold text-zinc-100"
              >{{ g.mName }}</span
            >
            <span
              class="text-sm font-extrabold tabular-nums"
              :style="{ color: 'var(--accent)' }"
              >{{ fmt(g.seconds) }}</span
            >
          </div>
        </div>
      </div>

      <!-- NUMBER ONE -->
      <div
        v-else-if="card.key === 'number1' && data?.topGame"
        class="relative flex min-h-[430px] flex-col overflow-hidden rounded-3xl p-8 text-white"
        :style="{
          background: 'linear-gradient(150deg, var(--accent-deep), #0c0710)',
        }"
      >
        <p
          class="text-xs font-bold uppercase tracking-[0.14em]"
          :style="{ color: 'var(--accent)' }"
        >
          Your number one
        </p>
        <div
          class="mx-auto mb-5 mt-auto h-[184px] w-[138px] overflow-hidden rounded-2xl bg-black/30 shadow-2xl"
        >
          <img
            v-if="data.topGame.mCoverObjectId"
            :src="objectUrl(data.topGame.mCoverObjectId)"
            class="h-full w-full object-cover"
          />
        </div>
        <div class="font-display text-3xl font-extrabold tracking-tight">
          {{ data.topGame.mName }}
        </div>
        <div class="mt-1.5 text-sm opacity-85">
          {{ fmt(data.topGame.seconds) }} · {{ data.topGame.pctOfTotal }}% of
          your playtime
        </div>
      </div>

      <!-- SESSIONS -->
      <div
        v-else-if="card.key === 'sessions' && data"
        class="relative flex min-h-[430px] flex-col overflow-hidden rounded-3xl border border-zinc-800 bg-zinc-900 p-8"
      >
        <p
          class="text-xs font-bold uppercase tracking-[0.14em]"
          :style="{ color: 'var(--accent)' }"
        >
          Time at the desk
        </p>
        <div
          class="mt-3 font-display font-extrabold leading-none tracking-tight tabular-nums text-[clamp(48px,10vw,72px)]"
          :style="{ color: 'var(--accent)' }"
        >
          {{ data.sessionCount.toLocaleString() }}
        </div>
        <div class="mt-2.5 text-sm text-zinc-400">play sessions</div>
        <div class="mt-auto flex gap-3">
          <div class="flex-1 rounded-2xl border border-zinc-800 bg-black/30 p-4">
            <div class="text-2xl font-extrabold tabular-nums text-zinc-100">
              {{ fmt(data.longestSessionSeconds) }}
            </div>
            <div class="mt-1 text-[11px] uppercase tracking-wide text-zinc-500">
              Longest session
            </div>
          </div>
          <div class="flex-1 rounded-2xl border border-zinc-800 bg-black/30 p-4">
            <div class="text-2xl font-extrabold tabular-nums text-zinc-100">
              {{ fmt(data.avgSessionSeconds) }}
            </div>
            <div class="mt-1 text-[11px] uppercase tracking-wide text-zinc-500">
              Average session
            </div>
          </div>
        </div>
      </div>

      <!-- TROPHIES -->
      <div
        v-else-if="card.key === 'trophies' && data"
        class="relative flex min-h-[430px] flex-col overflow-hidden rounded-3xl border border-zinc-800 bg-zinc-900 p-8"
      >
        <p
          class="text-xs font-bold uppercase tracking-[0.14em]"
          :style="{ color: 'var(--accent)' }"
        >
          Trophies
        </p>
        <div
          class="mt-3 font-display font-extrabold leading-none tracking-tight tabular-nums text-[clamp(48px,10vw,72px)]"
          :style="{ color: 'var(--accent)' }"
        >
          {{ data.achievementsUnlocked.toLocaleString() }}
        </div>
        <div class="mt-2.5 text-sm text-zinc-400">
          achievements unlocked<span v-if="data.points > 0">
            · {{ data.points.toLocaleString() }} points</span
          >
        </div>
        <div v-if="data.rarest" class="mt-auto">
          <div class="text-lg font-extrabold text-zinc-100">
            Rarest: {{ data.rarest.title }}
          </div>
          <span
            class="mt-2 inline-block rounded-full px-3 py-1 text-xs font-extrabold"
            :style="{ background: 'var(--accent)', color: 'var(--accent-contrast)' }"
            >Only {{ round1(data.rarest.globalPercent) }}% of players have
            it</span
          >
        </div>
      </div>

      <!-- VIBE -->
      <div
        v-else-if="card.key === 'vibe' && data && data.topTags.length"
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
          Your vibe
        </p>
        <div class="mt-auto">
          <div
            class="font-display font-extrabold leading-none tracking-tight text-[clamp(34px,7vw,52px)]"
            :style="{ color: 'var(--accent)' }"
          >
            {{ data.topTags[0].name }} person
          </div>
          <div class="mt-4 flex flex-wrap gap-2">
            <div
              v-for="t in data.topTags.slice(0, 3)"
              :key="t.name"
              class="rounded-full border border-zinc-800 bg-black/30 px-3 py-1.5 text-sm text-zinc-300"
            >
              {{ t.name }} · {{ fmt(t.seconds) }}
            </div>
          </div>
          <p class="mt-3 text-xs text-zinc-500">
            From this window's game tags — best-effort.
          </p>
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
          That's your Wrapped.
        </div>
        <div class="mt-3 text-lg opacity-85">
          Come back any time — pick any window.
        </div>
      </div>
    </template>
  </WrappedDeck>
</template>

<script setup lang="ts">
import WrappedDeck from "~/components/WrappedDeck.vue";
import {
  useServerApi,
  type PersonalWrapped,
  type UserProfile,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import { formatPlaytime } from "~/composables/use-recent-games";

useHead({ title: "Wrapped" });

const route = useRoute();
const router = useRouter();
const api = useServerApi();
const id = route.params.id as string;

const win = ref("all");
const data = ref<PersonalWrapped | null>(null);
const profile = ref<UserProfile | null>(null);
const loading = ref(true);

const theme = computed(() => profile.value?.profileTheme);
const title = computed(
  () => data.value?.displayName || profile.value?.displayName || "Wrapped",
);

const windowLabel = computed(() => {
  const m: Record<string, string> = {
    all: "all-time",
    year: "year",
    month: "month",
    week: "week",
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
    { key: "overview", title: "Overview" },
  ];
  if (d.topGames.length) list.push({ key: "topgames", title: "Top games" });
  if (d.topGame) list.push({ key: "number1", title: "Number one" });
  list.push({ key: "sessions", title: "Sessions" });
  if (d.achievementsUnlocked > 0)
    list.push({ key: "trophies", title: "Trophies" });
  if (d.topTags.length) list.push({ key: "vibe", title: "Your vibe" });
  list.push({ key: "finale", title: "Finale" });
  return list;
});

function objectUrl(o: string): string {
  return serverUrl(`api/v1/object/${o}`);
}
function fmt(s: number): string {
  return formatPlaytime(s);
}
function round1(n: number): number {
  return Math.round(n * 10) / 10;
}

function close() {
  router.push(`/profile/${id}`);
}

async function load() {
  loading.value = true;
  try {
    data.value = await api.profile.wrapped(id, win.value);
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
  api.profile
    .get(id)
    .then((p) => (profile.value = p))
    .catch(() => {});
  load();
});
</script>
