<template>
  <div
    ref="pageRoot"
    class="flex flex-col h-full"
    :style="{ backgroundColor: 'var(--bpm-bg)', color: 'var(--bpm-text)' }"
  >
    <!-- Header + view tabs (+ sort on the board) -->
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
          v-for="v in views"
          :key="v.value"
          :ref="(el: any) => registerTab(el, { onSelect: () => setView(v.value) })"
          :data-view-tab="v.value"
          class="relative px-3 py-1.5 rounded-lg text-xs font-medium transition-colors"
          :class="
            view === v.value
              ? 'bg-blue-600/20 text-blue-400'
              : 'bg-zinc-800 text-zinc-400 hover:bg-zinc-700'
          "
          @click="setView(v.value)"
        >
          {{ v.label }}
          <span
            v-if="v.value === 'mine' && unreadCount > 0"
            class="ml-1 inline-flex min-w-4 h-4 px-1 items-center justify-center rounded-full bg-blue-500 text-[10px] font-bold text-white"
          >
            {{ unreadCount }}
          </span>
        </button>
        <template v-if="view === 'board'">
          <span class="w-px mx-1 bg-zinc-700" />
          <button
            v-for="s in sorts"
            :key="s.value"
            :ref="(el: any) => registerTab(el, { onSelect: () => (sort = s.value) })"
            class="px-3 py-1.5 rounded-lg text-xs font-medium transition-colors"
            :class="
              sort === s.value
                ? 'bg-zinc-700 text-zinc-100'
                : 'bg-zinc-800 text-zinc-400 hover:bg-zinc-700'
            "
            @click="sort = s.value"
          >
            {{ s.label }}
          </button>
        </template>
      </div>
    </div>

    <!-- ── Board ─────────────────────────────────────────────────── -->
    <template v-if="view === 'board'">
      <div v-if="boardLoading" class="flex-1 overflow-y-auto px-8 py-6 space-y-3">
        <div
          v-for="i in 6"
          :key="i"
          class="h-24 rounded-xl bg-zinc-800/50 animate-pulse"
        />
      </div>

      <div v-else-if="boardError" class="flex-1 px-8 py-6">
        <BigPictureSectionError
          :ref="(el: any) => registerContent(el, { onSelect: fetchBoard })"
          title="Could not load the request board"
          :detail="boardError"
          @retry="fetchBoard"
        />
      </div>

      <div
        v-else-if="sortedRequests.length === 0"
        class="flex-1 flex flex-col items-center justify-center text-center px-8 gap-1"
      >
        <p class="text-base font-medium" :style="{ color: 'var(--bpm-text)' }">
          No requests yet
        </p>
        <p class="text-xs" :style="{ color: 'var(--bpm-muted)' }">
          Use New request at the top to add one.
        </p>
      </div>

      <div v-else class="flex-1 overflow-y-auto px-8 py-6 space-y-3">
        <!-- Keyed by request id so a re-sort after a vote moves the row, and
             the focus ring with it, instead of sliding another row under it. -->
        <div
          v-for="req in sortedRequests"
          :key="req.id"
          :data-board-request="req.id"
          :ref="
            (el: any) =>
              req.status !== 'Pending'
                ? registerContent(el, { onSelect: () => onBoardRowSelect(req.id) })
                : undefined
          "
          class="flex gap-4 p-4 rounded-xl bg-zinc-900/60"
        >
          <!-- Vote column. Only open while the request is pending; each
               arrow is its own focusable button. -->
          <div
            v-if="req.status === 'Pending'"
            class="flex flex-col items-center gap-1 shrink-0 w-12"
          >
            <button
              :ref="(el: any) => registerContent(el, { onSelect: () => toggleVote(req.id, 'Up') })"
              :class="[
                'p-1.5 rounded-md transition-colors',
                req.votes.userVote === 'Up'
                  ? 'text-blue-400 bg-blue-500/10'
                  : 'text-zinc-500 hover:text-zinc-300',
              ]"
              @click.stop="toggleVote(req.id, 'Up')"
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
              :ref="(el: any) => registerContent(el, { onSelect: () => toggleVote(req.id, 'Down') })"
              :class="[
                'p-1.5 rounded-md transition-colors',
                req.votes.userVote === 'Down'
                  ? 'text-red-400 bg-red-500/10'
                  : 'text-zinc-500 hover:text-zinc-300',
              ]"
              @click.stop="toggleVote(req.id, 'Down')"
            >
              <ChevronDownIcon class="size-5" />
            </button>
          </div>
          <div
            v-else
            class="flex flex-col items-center justify-center shrink-0 w-12"
          >
            <span class="text-sm font-bold text-zinc-500">{{ req.votes.up }}</span>
            <span class="text-[10px] text-zinc-600">votes</span>
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
                  statusClasses[req.status],
                ]"
              >
                {{ req.gameId ? "In library" : req.status }}
              </span>
            </div>
            <div
              class="flex items-center gap-3 mt-3 text-xs min-w-0"
              :style="{ color: 'var(--bpm-muted)' }"
            >
              <div class="flex items-center gap-1.5 min-w-0">
                <img
                  v-if="req.requester?.profilePictureObjectId"
                  :src="objectImageUrl(req.requester.profilePictureObjectId)"
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
              <span v-if="req.status !== 'Pending'">· Voting closed</span>
              <span v-if="req.gameId">· Press A to open the game</span>
              <button
                v-if="isMine(req) && req.status === 'Pending'"
                :ref="(el: any) => registerContent(el, { onSelect: () => askWithdraw(req.id, req.title) })"
                class="ml-auto px-3 py-1 rounded-lg bg-zinc-800 text-zinc-300 hover:bg-zinc-700"
                @click="askWithdraw(req.id, req.title)"
              >
                {{ withdrawing[req.id] ? "Withdrawing..." : "Withdraw" }}
              </button>
            </div>
            <p v-if="rowErrors[req.id]" class="mt-2 text-xs text-red-400">
              {{ rowErrors[req.id] }}
            </p>
          </div>
        </div>
      </div>
    </template>

    <!-- ── My requests ───────────────────────────────────────────── -->
    <template v-else-if="view === 'mine'">
      <div v-if="mineLoading" class="flex-1 overflow-y-auto px-8 py-6 space-y-3">
        <div
          v-for="i in 4"
          :key="i"
          class="h-24 rounded-xl bg-zinc-800/50 animate-pulse"
        />
      </div>

      <div v-else-if="mineError" class="flex-1 px-8 py-6">
        <BigPictureSectionError
          :ref="(el: any) => registerContent(el, { onSelect: fetchMine })"
          title="Could not load your requests"
          :detail="mineError"
          @retry="fetchMine"
        />
      </div>

      <div
        v-else-if="mine.length === 0"
        class="flex-1 flex flex-col items-center justify-center text-center px-8 gap-1"
      >
        <p class="text-base font-medium" :style="{ color: 'var(--bpm-text)' }">
          You have not requested any games yet
        </p>
        <p class="text-xs" :style="{ color: 'var(--bpm-muted)' }">
          Use New request at the top to add one.
        </p>
      </div>

      <div v-else class="flex-1 overflow-y-auto px-8 py-6 space-y-3">
        <!-- Every row is focusable so a controller can scroll through them.
             A opens the game once it is in the library, or offers to
             withdraw a pending request; other rows are read-only. -->
        <div
          v-for="req in mine"
          :key="req.id"
          :ref="(el: any) => registerContent(el, { onSelect: () => onMineRowSelect(req.id) })"
          class="p-4 rounded-xl bg-zinc-900/60"
        >
          <div class="flex items-start gap-3">
            <div class="flex-1 min-w-0">
              <h3 class="font-semibold truncate" :style="{ color: 'var(--bpm-text)' }">
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
                statusClasses[req.status],
              ]"
            >
              {{ req.game ? "In library" : req.status }}
            </span>
          </div>
          <p v-if="req.status === 'Denied'" class="mt-2 text-sm text-red-300">
            {{ req.denyReason ? `Denied: ${req.denyReason}` : "Denied. No reason was given." }}
          </p>
          <p
            v-else-if="req.status === 'Approved' && !req.game"
            class="mt-2 text-sm"
            :style="{ color: 'var(--bpm-muted)' }"
          >
            Approved. It will be marked as in the library once it is added.
          </p>
          <div class="flex items-center gap-3 mt-3 text-xs" :style="{ color: 'var(--bpm-muted)' }">
            <span>{{ formatTimeAgo(req.createdAt) }}</span>
            <span>· {{ req.votes.up }} up, {{ req.votes.down }} down</span>
            <span v-if="req.game">· Press A to open the game</span>
            <span v-else-if="req.status === 'Pending'">
              · {{ withdrawing[req.id] ? "Withdrawing..." : "Press A to withdraw" }}
            </span>
          </div>
          <p v-if="rowErrors[req.id]" class="mt-2 text-xs text-red-400">
            {{ rowErrors[req.id] }}
          </p>
        </div>
      </div>
    </template>

    <!-- ── New request ───────────────────────────────────────────── -->
    <!-- Plain focusable rows, no modal. Text comes from the on-screen
         keyboard, which owns its own input lock. -->
    <div v-else class="flex-1 overflow-y-auto px-8 py-6">
      <div class="max-w-3xl space-y-5">
        <div>
          <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">
            Game title
          </p>
          <button
            :ref="(el: any) => registerContent(el, { onSelect: () => openKeyboard('title') })"
            class="w-full flex items-center rounded-xl border border-zinc-700/50 bg-zinc-800/50 px-4 py-3 text-left hover:border-zinc-600"
            @click="openKeyboard('title')"
          >
            <span v-if="newTitle" class="text-sm text-zinc-100">{{ newTitle }}</span>
            <span v-else class="text-sm text-zinc-600">The name of the game</span>
            <PencilIcon class="size-4 text-zinc-500 ml-auto shrink-0" />
          </button>
        </div>

        <div>
          <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">
            Why should it be added? (optional)
          </p>
          <button
            :ref="(el: any) => registerContent(el, { onSelect: () => openKeyboard('note') })"
            class="w-full flex items-start rounded-xl border border-zinc-700/50 bg-zinc-800/50 px-4 py-3 text-left hover:border-zinc-600 min-h-[3.5rem]"
            @click="openKeyboard('note')"
          >
            <span v-if="newNote" class="text-sm text-zinc-100 line-clamp-3 flex-1">{{ newNote }}</span>
            <span v-else class="text-sm text-zinc-600 flex-1">A short note for the admins</span>
            <PencilIcon class="size-4 text-zinc-500 ml-3 mt-0.5 shrink-0" />
          </button>
          <p class="text-xs text-zinc-600 mt-1 text-right">{{ newNote.length }}/{{ NOTE_MAX }}</p>
        </div>

        <!-- Optional provider match -->
        <div>
          <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">
            Match on Steam / IGDB (optional)
          </p>
          <div
            v-if="matched"
            class="flex items-center gap-3 rounded-xl border border-blue-500/30 bg-blue-500/5 p-3"
          >
            <img
              v-if="matched.icon"
              :src="matched.icon"
              class="size-12 rounded object-cover bg-zinc-800 shrink-0"
              alt=""
            />
            <div class="flex-1 min-w-0">
              <p class="text-sm font-medium text-zinc-100 truncate">{{ matched.name }}</p>
              <p class="text-xs text-zinc-400 truncate">
                {{ matched.sourceName }}<span v-if="matched.year"> · {{ matched.year }}</span>
              </p>
            </div>
            <button
              :ref="(el: any) => registerContent(el, { onSelect: clearMatch })"
              class="px-3 py-1.5 rounded-lg text-xs bg-zinc-800 text-zinc-300 hover:bg-zinc-700"
              @click="clearMatch"
            >
              Clear match
            </button>
          </div>
          <template v-else>
            <button
              :ref="(el: any) => registerContent(el, { onSelect: searchProviders })"
              class="px-4 py-2.5 rounded-xl text-sm font-medium bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700"
              :class="{ 'opacity-50': searching }"
              @click="searchProviders"
            >
              {{ searching ? "Searching..." : "Search for the title above" }}
            </button>
            <p v-if="searchHint" class="mt-2 text-xs text-zinc-400">{{ searchHint }}</p>
            <p v-if="searchError" class="mt-2 text-xs text-red-400">
              Search failed: {{ searchError }}
            </p>
            <p v-else-if="failedProviders.length > 0" class="mt-2 text-xs text-yellow-400">
              Some providers could not be searched: {{ failedProviders.join(", ") }}.
              Results may be incomplete.
            </p>
            <p
              v-if="searchedQuery && !searching && !searchError && results.length === 0 && failedProviders.length === 0"
              class="mt-2 text-xs text-zinc-500"
            >
              No matches. You can still submit the request with just a title.
            </p>
            <div v-if="results.length > 0" class="mt-2 grid grid-cols-2 gap-2">
              <button
                v-for="r in results"
                :key="`${r.sourceId}-${r.id}`"
                :ref="(el: any) => registerContent(el, { onSelect: () => pickResult(r) })"
                class="flex items-center gap-3 rounded-xl bg-zinc-800/50 px-3 py-2 text-left hover:bg-zinc-700"
                @click="pickResult(r)"
              >
                <img
                  v-if="r.icon"
                  :src="r.icon"
                  class="size-10 rounded object-cover bg-zinc-900 shrink-0"
                  alt=""
                />
                <div class="flex-1 min-w-0">
                  <p class="text-sm text-zinc-100 truncate">{{ r.name }}</p>
                  <p class="text-xs text-zinc-500 truncate">
                    {{ r.sourceName }}<span v-if="r.year"> · {{ r.year }}</span>
                  </p>
                </div>
              </button>
            </div>
          </template>
        </div>

        <!-- Refused as a duplicate: offer the existing request instead -->
        <div
          v-if="conflict"
          class="rounded-xl bg-yellow-500/5 p-4 ring-1 ring-yellow-500/20 space-y-3"
        >
          <p class="text-sm text-zinc-200">{{ conflictMessage }}</p>
          <!-- The existing request's title is free text, so it gets its own
               line, plus the game it was matched to when that differs. -->
          <p v-if="conflict.request" class="text-sm text-zinc-300">
            Existing request:
            <span class="font-semibold text-zinc-100">{{ conflict.request.title }}</span>
          </p>
          <p v-if="conflictMatchedName" class="text-sm text-zinc-300">
            Matched game:
            <span class="font-semibold text-zinc-100">{{ conflictMatchedName }}</span>
          </p>
          <div class="flex gap-2 flex-wrap">
            <button
              v-if="conflict.reason === 'in-library' && conflict.game"
              :ref="(el: any) => registerContent(el, { onSelect: openConflictGame })"
              class="px-4 py-2 rounded-xl text-sm font-medium bg-blue-600 text-white hover:bg-blue-500"
              @click="openConflictGame"
            >
              Open the game
            </button>
            <button
              v-if="conflict.request && !conflict.request.mine && conflict.request.status === 'Pending'"
              :ref="(el: any) => registerContent(el, { onSelect: voteForConflict })"
              class="px-4 py-2 rounded-xl text-sm font-medium bg-blue-600 text-white hover:bg-blue-500"
              @click="voteForConflict"
            >
              {{ votingForConflict ? "Voting..." : "Vote for it instead" }}
            </button>
            <button
              v-if="conflict.reason === 'similar'"
              :ref="(el: any) => registerContent(el, { onSelect: () => submit(true) })"
              class="px-4 py-2 rounded-xl text-sm bg-zinc-800 text-zinc-200 hover:bg-zinc-700"
              @click="submit(true)"
            >
              Submit anyway
            </button>
          </div>
        </div>

        <p v-if="createError" class="text-sm text-red-400">{{ createError }}</p>

        <button
          :ref="(el: any) => registerContent(el, { onSelect: () => submit(false) })"
          class="px-6 py-3 rounded-xl text-sm font-medium bg-blue-600 text-white hover:bg-blue-500"
          :class="{ 'opacity-50': submitting || !newTitle.trim() }"
          @click="submit(false)"
        >
          {{ submitting ? "Submitting..." : "Submit request" }}
        </button>
      </div>
    </div>

    <BigPictureKeyboard
      :visible="keyboardField !== null"
      :model-value="keyboardField === 'note' ? newNote : newTitle"
      :placeholder="keyboardField === 'note' ? 'A short note for the admins' : 'The name of the game'"
      @update:model-value="onKeyboardInput"
      @close="keyboardField = null"
      @submit="keyboardField = null"
    />

    <BigPictureDialog
      :visible="withdrawTarget !== null"
      title="Withdraw this request?"
      :message="withdrawMessage"
      confirm-label="Withdraw"
      cancel-label="Keep it"
      destructive
      @confirm="confirmWithdraw"
      @cancel="withdrawTarget = null"
    />
  </div>
</template>

<script setup lang="ts">
/**
 * Big Picture Mode request board: the public board with voting, the user's
 * own requests ("My requests": status, deny reason, linked game, withdraw)
 * and a controller-friendly form to request a game. Mirrors the server's
 * web page (`pages/requests.vue` in drop-server), which the desktop layout
 * shows in an iframe.
 */
import {
  ChevronUpIcon,
  ChevronDownIcon,
  PencilIcon,
} from "@heroicons/vue/24/outline";
import { serverUrl } from "~/composables/use-server-fetch";
import { objectImageUrl } from "~/composables/use-object";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useAppState } from "~/composables/app-state";
import {
  markRequestDecisionsRead,
  useRequestNotifications,
} from "~/composables/request-notifications";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";
import BigPictureDialog from "~/components/bigpicture/BigPictureDialog.vue";
import BigPictureSectionError from "~/components/bigpicture/BigPictureSectionError.vue";

definePageMeta({ layout: "bigpicture" });

// Mirrors the server-side limits in drop-server server/internal/requests.
const TITLE_MAX = 120;
const NOTE_MAX = 500;
const ROUTE = "/bigpicture/requests";

const router = useRouter();
const pageRoot = ref<HTMLElement | null>(null);
const focusNav = useFocusNavigation();
const registerTab = useBpFocusableGroup("content");
const registerContent = useBpFocusableGroup("content");
const appState = useAppState();
const { unreadCount } = useRequestNotifications();

type Status = "Pending" | "Approved" | "Denied" | "Withdrawn";
const statusClasses: Record<Status, string> = {
  Pending: "bg-yellow-500/10 text-yellow-400",
  Approved: "bg-green-500/10 text-green-400",
  Denied: "bg-red-500/10 text-red-400",
  Withdrawn: "bg-zinc-500/10 text-zinc-400",
};

/** Reads the server's error body: `{ statusMessage, message, data }`. */
async function readError(resp: Response): Promise<{
  message: string;
  data: unknown;
}> {
  try {
    const body = (await resp.json()) as {
      statusMessage?: string;
      message?: string;
      data?: unknown;
    };
    return {
      message: body.statusMessage || body.message || `HTTP ${resp.status}`,
      data: body.data,
    };
  } catch {
    return { message: `HTTP ${resp.status}`, data: undefined };
  }
}

function describe(e: unknown) {
  return e instanceof Error ? e.message : String(e);
}

// ── View ──────────────────────────────────────────────────────────────
type View = "board" | "mine" | "new";
const views = [
  { value: "board", label: "Board" },
  { value: "mine", label: "My requests" },
  { value: "new", label: "New request" },
] as const;
// Remembered across a trip to a game page and back.
const view = ref<View>(focusNav.getRouteState<View>("view", ROUTE) ?? "board");

function setView(v: View) {
  view.value = v;
  focusNav.setRouteState("view", v, ROUTE);
  if (v === "mine") {
    void fetchMine();
    void markRequestDecisionsRead();
  }
}

function openGame(gameId: string) {
  const target = `/bigpicture/library/${gameId}`;
  focusNav.setRouteState("backTo", ROUTE, target);
  router.push(target).catch((e) => {
    console.error("[BPM:REQUESTS] navigation to game failed:", e);
  });
}

// Per-row action errors and in-flight flags, keyed by request id.
const rowErrors = ref<Record<string, string>>({});
const withdrawing = ref<Record<string, boolean>>({});

// ── Board ─────────────────────────────────────────────────────────────
type RequestItem = {
  id: string;
  title: string;
  description: string;
  status: "Pending" | "Approved";
  gameId: string | null;
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
const boardLoading = ref(true);
const boardError = ref<string | null>(null);
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

function isMine(req: RequestItem) {
  const me = appState.value?.user?.id;
  return !!me && req.requester?.id === me;
}

async function fetchBoard() {
  boardLoading.value = true;
  boardError.value = null;
  try {
    const resp = await fetch(serverUrl("api/v1/community/requests"));
    if (!resp.ok) {
      boardError.value = (await readError(resp)).message;
      console.error("[BPM:REQUESTS] board fetch failed:", resp.status);
      return;
    }
    requests.value = (await resp.json()) as RequestItem[];
  } catch (e) {
    boardError.value = describe(e);
    console.error("[BPM:REQUESTS] board fetch error:", e);
  } finally {
    boardLoading.value = false;
    focusNav.autoFocusContent("content");
  }
}

/**
 * Toggle the user's vote, same semantics as the web board: the active arrow
 * clears the vote (DELETE), the other one sets it (POST). A failure is shown
 * on the row.
 */
async function toggleVote(id: string, v: "Up" | "Down") {
  const req = requests.value.find((r) => r.id === id);
  if (!req) return;
  const isClear = req.votes.userVote === v;
  const url = serverUrl(`api/v1/store/requests/${req.id}/vote`);
  rowErrors.value[req.id] = "";
  try {
    const resp = isClear
      ? await fetch(url, { method: "DELETE" })
      : await fetch(url, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ vote: v }),
        });
    if (!resp.ok) {
      rowErrors.value[req.id] = `Vote failed: ${(await readError(resp)).message}`;
      return;
    }
    const result = (await resp.json()) as { up: number; down: number };
    req.votes = {
      ...req.votes,
      up: result.up,
      down: result.down,
      total: result.up + result.down,
      userVote: isClear ? null : v,
    };
  } catch (e) {
    rowErrors.value[req.id] = `Vote failed: ${describe(e)}`;
  }
}

// ── My requests ───────────────────────────────────────────────────────
type MyRequest = {
  id: string;
  title: string;
  description: string;
  status: Status;
  createdAt: string;
  denyReason: string | null;
  game: { id: string; name: string } | null;
  votes: { up: number; down: number };
};
const mine = ref<MyRequest[]>([]);
const mineLoading = ref(false);
const mineError = ref<string | null>(null);

async function fetchMine() {
  // Only the first load shows the skeleton; a refresh keeps the rows (and
  // the focus on them) in place.
  mineLoading.value = mine.value.length === 0;
  mineError.value = null;
  try {
    const resp = await fetch(serverUrl("api/v1/store/requests/list"));
    if (!resp.ok) {
      mineError.value = (await readError(resp)).message;
      console.error("[BPM:REQUESTS] my requests fetch failed:", resp.status);
      return;
    }
    mine.value = (await resp.json()) as MyRequest[];
  } catch (e) {
    mineError.value = describe(e);
    console.error("[BPM:REQUESTS] my requests fetch error:", e);
  } finally {
    mineLoading.value = false;
    focusNav.autoFocusContent("content");
  }
}

// Focus registrations keep the first options they were given, and a row's
// element outlives a refresh, so row actions look the request up by id at
// press time rather than closing over a row object that may be stale.
function onMineRowSelect(id: string) {
  const req = mine.value.find((r) => r.id === id);
  if (!req) return;
  if (req.game) openGame(req.game.id);
  else if (req.status === "Pending") askWithdraw(req.id, req.title);
}

function onBoardRowSelect(id: string) {
  const req = requests.value.find((r) => r.id === id);
  if (req?.gameId) openGame(req.gameId);
}

// ── Withdraw (confirmed, never optimistic) ────────────────────────────
const withdrawTarget = ref<{ id: string; title: string } | null>(null);
const withdrawMessage = computed(() =>
  withdrawTarget.value
    ? `"${withdrawTarget.value.title}" will be taken off the board.`
    : "",
);

function askWithdraw(id: string, title: string) {
  if (withdrawing.value[id]) return;
  withdrawTarget.value = { id, title };
}

async function confirmWithdraw() {
  const target = withdrawTarget.value;
  withdrawTarget.value = null;
  if (!target) return;
  withdrawing.value[target.id] = true;
  rowErrors.value[target.id] = "";
  try {
    const resp = await fetch(serverUrl(`api/v1/store/requests/${target.id}`), {
      method: "DELETE",
    });
    if (!resp.ok) {
      rowErrors.value[target.id] =
        `Could not withdraw: ${(await readError(resp)).message}`;
      return;
    }
    moveFocusOffBoardRow(target.id);
    requests.value = requests.value.filter((r) => r.id !== target.id);
    const own = mine.value.find((r) => r.id === target.id);
    if (own) own.status = "Withdrawn";
  } catch (e) {
    rowErrors.value[target.id] = `Could not withdraw: ${describe(e)}`;
  } finally {
    withdrawing.value[target.id] = false;
  }
}

/**
 * A withdrawn request leaves the board, and with it the Withdraw button that
 * usually holds focus. Before the row goes, focus moves to the nearest
 * remaining row (next one down, else the one above), found by request id,
 * or to the Board tab when the board is about to be empty. Focus that is
 * elsewhere by then is left alone.
 */
function moveFocusOffBoardRow(id: string) {
  const root = pageRoot.value;
  // `currentFocused` is exposed deep-readonly; the element itself is a plain
  // DOM node.
  const focused = focusNav.currentFocused.value?.el as HTMLElement | undefined;
  if (!root || !focused) return;
  const rowFor = (rid: string) =>
    root.querySelector<HTMLElement>(
      `[data-board-request="${CSS.escape(rid)}"]`,
    );
  if (!rowFor(id)?.contains(focused)) return;

  const order = sortedRequests.value.map((r) => r.id);
  const at = order.indexOf(id);
  const candidates =
    at === -1
      ? []
      : [...order.slice(at + 1), ...order.slice(0, at).reverse()];
  for (const rid of candidates) {
    const row = rowFor(rid);
    if (!row) continue;
    // A decided row is itself focusable; a pending one through its buttons.
    const target = row.hasAttribute("data-focusable")
      ? row
      : row.querySelector<HTMLElement>("[data-focusable]");
    if (focusNav.focusElement(target)) return;
  }
  focusNav.focusElement(
    root.querySelector<HTMLElement>('[data-view-tab="board"]'),
  );
}

// ── New request ───────────────────────────────────────────────────────
type MetadataResult = {
  id: string;
  name: string;
  icon: string;
  year: number;
  sourceId: string;
  sourceName: string;
};
type Conflict = {
  reason: "duplicate" | "similar" | "in-library";
  request?: {
    id: string;
    title: string;
    status: string;
    mine: boolean;
    // Absent from servers older than this client.
    matchedName?: string | null;
  };
  game?: { id: string; name: string };
};

const newTitle = ref("");
const newNote = ref("");
const keyboardField = ref<"title" | "note" | null>(null);
const matched = ref<MetadataResult | null>(null);
const results = ref<MetadataResult[]>([]);
const failedProviders = ref<string[]>([]);
const searchError = ref<string | null>(null);
const searchHint = ref<string | null>(null);
const searchedQuery = ref("");
const searching = ref(false);
const submitting = ref(false);
const createError = ref<string | null>(null);
const conflict = ref<Conflict | null>(null);
const votingForConflict = ref(false);

const conflictMessage = computed(() => {
  const c = conflict.value;
  if (!c) return "";
  if (c.reason === "in-library")
    return `${c.game?.name ?? "This game"} is already in the library.`;
  if (c.request?.mine) return "You already requested this.";
  return c.reason === "duplicate"
    ? "This game has already been requested."
    : "A request with the same title already exists.";
});

// The matched game's name, unless it just repeats the request's title.
const conflictMatchedName = computed(() => {
  const r = conflict.value?.request;
  const name = r?.matchedName?.trim();
  if (!r || !name) return null;
  return name.toLowerCase() === r.title.trim().toLowerCase() ? null : name;
});

function openKeyboard(field: "title" | "note") {
  keyboardField.value = field;
}

function onKeyboardInput(value: string) {
  if (keyboardField.value === "title") {
    newTitle.value = value.slice(0, TITLE_MAX);
    conflict.value = null;
  } else if (keyboardField.value === "note") {
    newNote.value = value.slice(0, NOTE_MAX);
  }
}

async function searchProviders() {
  if (searching.value) return;
  const q = newTitle.value.trim();
  searchHint.value = null;
  if (q.length < 2) {
    searchHint.value = "Enter at least two letters of the title first.";
    return;
  }
  searching.value = true;
  searchError.value = null;
  try {
    const resp = await fetch(
      serverUrl(`api/v1/store/requests/metadata-search?q=${encodeURIComponent(q)}`),
    );
    if (!resp.ok) {
      searchError.value = (await readError(resp)).message;
      results.value = [];
      failedProviders.value = [];
      return;
    }
    const body = (await resp.json()) as {
      results: MetadataResult[];
      failedProviders: Array<{ name: string }>;
    };
    // A short grid, not a long scrolling list.
    results.value = body.results.slice(0, 6);
    failedProviders.value = body.failedProviders.map((f) => f.name);
  } catch (e) {
    searchError.value = describe(e);
    results.value = [];
    failedProviders.value = [];
  } finally {
    searchedQuery.value = q;
    searching.value = false;
  }
}

function resetSearch() {
  results.value = [];
  failedProviders.value = [];
  searchError.value = null;
  searchHint.value = null;
  searchedQuery.value = "";
}

function pickResult(r: MetadataResult) {
  matched.value = r;
  conflict.value = null;
  if (!newTitle.value.trim()) newTitle.value = r.name.slice(0, TITLE_MAX);
  resetSearch();
}

function clearMatch() {
  matched.value = null;
  conflict.value = null;
}

async function submit(allowSimilar: boolean) {
  if (submitting.value) return;
  createError.value = null;
  if (!newTitle.value.trim()) {
    createError.value = "Enter the game title first.";
    return;
  }
  submitting.value = true;
  conflict.value = null;
  try {
    const m = matched.value;
    const resp = await fetch(serverUrl("api/v1/store/requests/create"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        title: newTitle.value.trim(),
        description: newNote.value.trim(),
        steamUrl:
          m?.sourceId === "Steam"
            ? `https://store.steampowered.com/app/${m.id}`
            : undefined,
        metadata: m ? { sourceId: m.sourceId, id: m.id, name: m.name } : undefined,
        allowSimilar: allowSimilar || undefined,
      }),
    });
    if (!resp.ok) {
      const err = await readError(resp);
      const data = err.data as Conflict | undefined;
      if (resp.status === 409 && data?.reason) conflict.value = data;
      else createError.value = `Could not create the request: ${err.message}`;
      return;
    }
    newTitle.value = "";
    newNote.value = "";
    matched.value = null;
    resetSearch();
    mine.value = [];
    setView("mine");
    void fetchBoard();
  } catch (e) {
    createError.value = `Could not create the request: ${describe(e)}`;
  } finally {
    submitting.value = false;
  }
}

function openConflictGame() {
  const gameId = conflict.value?.game?.id;
  if (gameId) openGame(gameId);
}

async function voteForConflict() {
  const target = conflict.value?.request;
  if (!target || votingForConflict.value) return;
  votingForConflict.value = true;
  createError.value = null;
  try {
    const resp = await fetch(
      serverUrl(`api/v1/store/requests/${target.id}/vote`),
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ vote: "Up" }),
      },
    );
    if (!resp.ok) {
      createError.value = `Vote failed: ${(await readError(resp)).message}`;
      return;
    }
    conflict.value = null;
    newTitle.value = "";
    newNote.value = "";
    matched.value = null;
    setView("board");
    void fetchBoard();
  } catch (e) {
    createError.value = `Vote failed: ${describe(e)}`;
  } finally {
    votingForConflict.value = false;
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

onMounted(() => {
  void fetchBoard();
  if (view.value === "mine") {
    void fetchMine();
    void markRequestDecisionsRead();
  }
});
</script>
