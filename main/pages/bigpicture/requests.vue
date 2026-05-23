<template>
  <div
    class="flex flex-col h-full"
    :style="{ backgroundColor: 'var(--bpm-bg)', color: 'var(--bpm-text)' }"
  >
    <!-- Header + sort toggle -->
    <div
      class="flex items-center justify-between gap-4 px-8 py-4 border-b"
      :style="{ borderColor: 'var(--bpm-border)' }"
    >
      <div class="min-w-0">
        <h1
          class="text-2xl font-bold font-display truncate"
          :style="{ color: 'var(--bpm-text)' }"
        >
          Request Board
        </h1>
        <p class="text-xs mt-0.5" :style="{ color: 'var(--bpm-muted)' }">
          Vote on games the community wants added to Drop
        </p>
      </div>
      <div class="flex gap-2 shrink-0">
        <button
          v-for="s in sorts"
          :key="s.value"
          :ref="(el: any) => registerTab(el, { onSelect: () => (sort = s.value) })"
          class="px-3 py-1.5 rounded-lg text-xs font-medium transition-colors"
          :class="
            sort === s.value
              ? 'bg-blue-600/20 text-blue-400'
              : 'bg-zinc-800 text-zinc-400 hover:bg-zinc-700'
          "
          @click="sort = s.value"
        >
          {{ s.label }}
        </button>
      </div>
    </div>

    <!-- Loading skeleton -->
    <div v-if="loading" class="flex-1 overflow-y-auto px-8 py-6 space-y-3">
      <div
        v-for="i in 6"
        :key="i"
        class="h-24 rounded-xl bg-zinc-800/50 animate-pulse"
      />
    </div>

    <!-- Empty state — Big Picture is read + vote only; creation lives on desktop. -->
    <div
      v-else-if="sortedRequests.length === 0"
      class="flex-1 flex flex-col items-center justify-center text-center px-8 gap-1"
    >
      <p class="text-base font-medium" :style="{ color: 'var(--bpm-text)' }">
        No requests yet
      </p>
      <p class="text-xs" :style="{ color: 'var(--bpm-muted)' }">
        Submit one from Drop on desktop — Big Picture is read + vote only.
      </p>
    </div>

    <!-- List -->
    <div v-else class="flex-1 overflow-y-auto px-8 py-6 space-y-3">
      <div
        v-for="req in sortedRequests"
        :key="req.id"
        class="flex gap-4 p-4 rounded-xl bg-zinc-900/60"
      >
        <!-- Vote column. Each arrow is its own focusable button so a
             gamepad user can navigate up/down through the list and
             tab L/R between the arrows on a row. -->
        <div class="flex flex-col items-center gap-1 shrink-0 w-12">
          <button
            :ref="(el: any) => registerContent(el, { onSelect: () => toggleVote(req, 'Up') })"
            :class="[
              'p-1.5 rounded-md transition-colors',
              req.votes.userVote === 'Up'
                ? 'text-blue-400 bg-blue-500/10'
                : 'text-zinc-500 hover:text-zinc-300',
            ]"
            @click.stop="toggleVote(req, 'Up')"
          >
            <ChevronUpIcon class="size-5" />
          </button>
          <span
            class="text-sm font-bold"
            :class="req.votes.up > 0 ? 'text-blue-400' : 'text-zinc-500'"
          >
            {{ req.votes.up }}
          </span>
          <button
            :ref="(el: any) => registerContent(el, { onSelect: () => toggleVote(req, 'Down') })"
            :class="[
              'p-1.5 rounded-md transition-colors',
              req.votes.userVote === 'Down'
                ? 'text-red-400 bg-red-500/10'
                : 'text-zinc-500 hover:text-zinc-300',
            ]"
            @click.stop="toggleVote(req, 'Down')"
          >
            <ChevronDownIcon class="size-5" />
          </button>
        </div>

        <!-- Body -->
        <div class="flex-1 min-w-0">
          <div class="flex items-start gap-3">
            <div class="flex-1 min-w-0">
              <h3
                class="font-semibold truncate"
                :style="{ color: 'var(--bpm-text)' }"
              >
                {{ req.title }}
              </h3>
              <p
                v-if="req.description"
                class="text-sm mt-1 line-clamp-2"
                :style="{ color: 'var(--bpm-muted)' }"
              >
                {{ req.description }}
              </p>
            </div>
            <span
              :class="[
                'shrink-0 inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium',
                req.status === 'Pending'
                  ? 'bg-yellow-500/10 text-yellow-400'
                  : 'bg-green-500/10 text-green-400',
              ]"
            >
              {{ req.status }}
            </span>
          </div>
          <div
            class="flex items-center gap-3 mt-3 text-xs min-w-0"
            :style="{ color: 'var(--bpm-muted)' }"
          >
            <div class="flex items-center gap-1.5 min-w-0">
              <img
                v-if="req.requester?.profilePictureObjectId"
                :src="
                  serverUrl(
                    `api/v1/object/${req.requester.profilePictureObjectId}`,
                  )
                "
                class="size-5 rounded-full shrink-0"
              />
              <span class="truncate">
                {{
                  req.requester?.displayName ||
                  req.requester?.username ||
                  "Unknown"
                }}
              </span>
            </div>
            <span>·</span>
            <span>{{ formatTimeAgo(req.createdAt) }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Big Picture Mode view of the Request Board. Read + vote only — creating
 * and withdrawing requests stay on the desktop page (`pages/requests.vue`
 * in drop-server), which already has the create dialog wired up. This
 * mirror exists so a gamepad user can still discover and upvote what the
 * community wants added.
 */
import { ChevronUpIcon, ChevronDownIcon } from "@heroicons/vue/24/outline";
import { serverUrl } from "~/composables/use-server-fetch";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";

definePageMeta({ layout: "bigpicture" });

const focusNav = useFocusNavigation();
const registerTab = useBpFocusableGroup("content");
const registerContent = useBpFocusableGroup("content");

type RequestItem = {
  id: string;
  title: string;
  description: string;
  igdbUrl: string | null;
  steamUrl: string | null;
  status: "Pending" | "Approved";
  createdAt: string;
  requester: {
    id: string;
    username: string;
    displayName: string;
    profilePictureObjectId: string;
  } | null;
  votes: {
    up: number;
    down: number;
    total: number;
    userVote: "Up" | "Down" | null;
  };
};

const sorts = [
  { value: "votes", label: "Top voted" },
  { value: "newest", label: "Newest" },
] as const;

const sort = ref<"votes" | "newest">("votes");
const loading = ref(true);
const requests = ref<RequestItem[]>([]);

const sortedRequests = computed(() => {
  const list = [...requests.value];
  if (sort.value === "votes") {
    list.sort((a, b) => b.votes.up - a.votes.up);
  } else {
    list.sort(
      (a, b) =>
        new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime(),
    );
  }
  return list;
});

async function fetchRequests() {
  loading.value = true;
  try {
    const resp = await fetch(serverUrl("api/v1/community/requests"));
    if (!resp.ok) {
      console.error("[BPM:REQUESTS] fetch failed:", resp.status);
      requests.value = [];
      return;
    }
    requests.value = (await resp.json()) as RequestItem[];
  } catch (e) {
    console.error("[BPM:REQUESTS] fetch error:", e);
    requests.value = [];
  } finally {
    loading.value = false;
    nextTick(() => focusNav.autoFocusContent("content"));
  }
}

/**
 * Toggle the user's vote on a request — same semantics as the desktop
 * request board: clicking the currently-active arrow clears the vote
 * (DELETE), clicking the other arrow flips it (POST).
 */
async function toggleVote(req: RequestItem, v: "Up" | "Down") {
  const isClear = req.votes.userVote === v;
  const url = serverUrl(`api/v1/store/requests/${req.id}/vote`);
  try {
    const resp = isClear
      ? await fetch(url, { method: "DELETE" })
      : await fetch(url, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ vote: v }),
        });
    if (!resp.ok) {
      console.warn("[BPM:REQUESTS] vote failed:", resp.status);
      return;
    }
    const result = (await resp.json()) as Partial<RequestItem["votes"]>;
    req.votes = {
      ...req.votes,
      ...result,
      userVote: isClear ? null : v,
    };
  } catch (e) {
    console.error("[BPM:REQUESTS] vote error:", e);
  }
}

function formatTimeAgo(timestamp: string): string {
  const diff = Date.now() - new Date(timestamp).getTime();
  const minutes = Math.floor(diff / 60000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return `${Math.floor(days / 7)}w ago`;
}

onMounted(fetchRequests);
</script>
