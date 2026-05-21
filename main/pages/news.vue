<template>
  <div class="mx-auto max-w-4xl px-8 py-6">
    <div class="mb-6">
      <h1 class="text-2xl font-display font-bold text-zinc-100">News</h1>
      <p class="mt-1 text-sm text-zinc-400">
        Announcements and updates from your Drop server.
      </p>
    </div>

    <!-- Search -->
    <div class="mb-6">
      <div class="relative">
        <MagnifyingGlassIcon
          class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-zinc-500 pointer-events-none"
        />
        <input
          v-model="searchInput"
          type="text"
          placeholder="Search articles..."
          class="w-full rounded-lg border border-zinc-700 bg-zinc-800/50 pl-9 pr-3 py-2 text-sm text-zinc-100 placeholder:text-zinc-500 focus:bg-zinc-800 focus:ring-2 focus:ring-blue-500 outline-none transition-colors"
          @input="debouncedSearch"
        />
      </div>
    </div>

    <!-- Article detail view -->
    <article
      v-if="selectedArticle"
      class="rounded-xl bg-zinc-800/50 backdrop-blur-sm ring-1 ring-zinc-700/40 overflow-hidden"
    >
      <button
        class="px-5 py-3 inline-flex items-center gap-x-2 text-sm font-medium text-zinc-400 hover:text-zinc-200 transition-colors"
        @click="selectedArticle = null"
      >
        <ArrowLeftIcon class="size-4" />
        Back to all articles
      </button>
      <img
        v-if="selectedArticle.imageObjectId"
        :src="objectUrl(selectedArticle.imageObjectId)"
        class="w-full h-64 object-cover"
      />
      <div class="px-8 py-6">
        <div
          v-if="selectedArticle.tags?.length"
          class="flex gap-2 mb-3"
        >
          <span
            v-for="tag in selectedArticle.tags"
            :key="tag.id"
            class="px-2 py-0.5 rounded-full text-[10px] font-bold uppercase bg-blue-500/20 text-blue-300"
          >
            {{ tag.name }}
          </span>
        </div>
        <h2 class="text-3xl font-display font-bold text-zinc-100 mb-2">
          {{ selectedArticle.title }}
        </h2>
        <p class="text-sm text-zinc-500 mb-6">
          {{ formatDate(selectedArticle.publishedAt) }}
          <template v-if="selectedArticle.author?.displayName">
            · by {{ selectedArticle.author.displayName }}
          </template>
        </p>
        <div
          class="prose prose-invert prose-blue max-w-none"
          v-html="renderedContent"
        />
      </div>
    </article>

    <!-- List view -->
    <template v-else>
      <div
        v-if="loading"
        class="text-sm text-zinc-500 py-10 text-center"
      >
        Loading articles...
      </div>
      <div
        v-else-if="articles.length === 0"
        class="text-sm text-zinc-500 py-20 text-center"
      >
        {{
          searchQuery
            ? "No articles match that search."
            : "No news articles yet."
        }}
      </div>
      <div v-else class="space-y-3">
        <button
          v-for="article in articles"
          :key="article.id"
          class="w-full flex items-start gap-x-4 rounded-xl bg-zinc-800/50 backdrop-blur-sm p-4 ring-1 ring-zinc-700/40 hover:ring-blue-500/40 transition-colors text-left"
          @click="openArticle(article)"
        >
          <img
            v-if="article.imageObjectId"
            :src="objectUrl(article.imageObjectId)"
            class="w-28 h-20 rounded-lg object-cover shrink-0 hidden sm:block"
          />
          <div class="flex-1 min-w-0">
            <div
              v-if="article.tags?.length"
              class="flex gap-2 mb-1"
            >
              <span
                v-for="tag in article.tags.slice(0, 3)"
                :key="tag.id"
                class="px-1.5 py-0.5 rounded-full text-[9px] font-bold uppercase bg-blue-500/20 text-blue-300"
              >
                {{ tag.name }}
              </span>
            </div>
            <h3 class="text-base font-display font-semibold text-zinc-100">
              {{ article.title }}
            </h3>
            <p
              class="text-sm text-zinc-400 mt-1 line-clamp-2"
            >
              {{ article.description }}
            </p>
            <p class="text-xs text-zinc-500 mt-2">
              {{ formatDate(article.publishedAt) }}
              <template v-if="article.author?.displayName">
                · {{ article.author.displayName }}
              </template>
            </p>
          </div>
        </button>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import {
  MagnifyingGlassIcon,
  ArrowLeftIcon,
} from "@heroicons/vue/24/outline";
import { micromark } from "micromark";
import {
  useServerApi,
  type NewsArticle,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";

useHead({ title: "News" });

const api = useServerApi();

const articles = ref<NewsArticle[]>([]);
const selectedArticle = ref<NewsArticle | null>(null);
const loading = ref(true);
const searchInput = ref("");
const searchQuery = ref("");

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

function formatDate(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleDateString(undefined, {
      month: "long",
      day: "numeric",
      year: "numeric",
    });
  } catch {
    return iso;
  }
}

const renderedContent = computed(() => {
  // Articles are markdown — same renderer as game descriptions so the
  // typography is consistent across the app. Guard against the server
  // returning null/undefined content (drafts, malformed records) so the
  // page renders empty body instead of throwing inside micromark.
  const raw = selectedArticle.value?.content;
  if (!raw || typeof raw !== "string") return "";
  try {
    return micromark(raw);
  } catch (e) {
    console.warn("[news] markdown parse failed:", e);
    return `<p class="text-red-400 text-sm">Couldn't render this article's content.</p>`;
  }
});

let searchDebounce: ReturnType<typeof setTimeout> | null = null;
function debouncedSearch() {
  if (searchDebounce) clearTimeout(searchDebounce);
  searchDebounce = setTimeout(() => {
    searchQuery.value = searchInput.value.trim();
    load();
  }, 300);
}

function openArticle(article: NewsArticle) {
  selectedArticle.value = article;
  // Scroll to top so the article opens at its title, not wherever the
  // list was scrolled.
  window.scrollTo({ top: 0, behavior: "smooth" });
}

async function load() {
  loading.value = true;
  try {
    const data = await api.news.list({
      limit: 30,
      order: "desc",
      search: searchQuery.value || undefined,
    });
    articles.value = data;
  } catch (e) {
    console.warn("[news] list failed:", e);
    articles.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(load);
</script>
