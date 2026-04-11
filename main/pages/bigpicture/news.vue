<template>
  <div class="flex flex-col h-full">
    <!-- Loading -->
    <div v-if="loading" class="flex-1 overflow-y-auto px-8 py-6">
      <div class="space-y-4">
        <div v-for="i in 5" :key="i" class="h-32 rounded-xl bg-zinc-800/50 animate-pulse" />
      </div>
    </div>

    <template v-else>
      <!-- ═══ Article detail view ═══ -->
      <div v-if="selectedArticle" class="flex-1 overflow-y-auto px-8 py-6">
        <button
          :ref="(el: any) => registerContent(el, { onSelect: closeArticle })"
          class="flex items-center gap-2 text-sm text-zinc-400 hover:text-zinc-200 mb-6 transition-colors"
          @click="closeArticle"
        >
          <ArrowLeftIcon class="size-4" />
          Back to news
        </button>

        <div v-if="selectedArticle.imageObjectId" class="rounded-2xl overflow-hidden mb-6 aspect-[21/9]">
          <img :src="objectUrl(selectedArticle.imageObjectId)" :alt="selectedArticle.title" class="w-full h-full object-cover" />
        </div>

        <h1 class="text-3xl font-bold font-display text-zinc-100 mb-3">{{ selectedArticle.title }}</h1>

        <div class="flex items-center gap-3 text-sm text-zinc-500 mb-6">
          <span v-if="selectedArticle.author">{{ selectedArticle.author.displayName }}</span>
          <span>{{ formatDate(selectedArticle.publishedAt) }}</span>
          <div v-if="selectedArticle.tags?.length" class="flex gap-2">
            <span v-for="tag in selectedArticle.tags" :key="tag.id" class="px-2 py-0.5 rounded-full bg-zinc-800/60 text-xs text-zinc-400">
              {{ tag.name }}
            </span>
          </div>
        </div>

        <div class="prose prose-invert prose-zinc max-w-none text-zinc-300 leading-relaxed" v-html="renderedContent" />
      </div>

      <!-- ═══ News list view ═══ -->
      <div v-else class="flex-1 overflow-y-auto px-8 py-6">
        <!-- Featured article -->
        <div
          v-if="articles.length > 0"
          :ref="(el: any) => registerContent(el, { onSelect: () => openArticle(articles[0]) })"
          class="relative rounded-2xl overflow-hidden mb-6 cursor-pointer aspect-[21/9]"
          @click="openArticle(articles[0])"
        >
          <img v-if="articles[0].imageObjectId" :src="objectUrl(articles[0].imageObjectId)" :alt="articles[0].title" class="w-full h-full object-cover" />
          <div v-else class="w-full h-full bg-zinc-800" />
          <div class="absolute inset-0 bg-gradient-to-t from-zinc-950/90 via-zinc-950/30 to-transparent" />
          <div class="absolute bottom-0 inset-x-0 p-6">
            <div class="flex gap-2 mb-2">
              <span v-for="tag in (articles[0].tags || []).slice(0, 3)" :key="tag.id" class="px-2 py-0.5 rounded-full bg-zinc-800/60 text-xs text-zinc-400">{{ tag.name }}</span>
            </div>
            <h2 class="text-2xl font-bold font-display text-white mb-1">{{ articles[0].title }}</h2>
            <p class="text-sm text-zinc-300 line-clamp-2 max-w-2xl">{{ articles[0].description }}</p>
            <p class="text-xs text-zinc-500 mt-2">{{ formatDate(articles[0].publishedAt) }}</p>
          </div>
        </div>

        <!-- Article cards -->
        <div class="grid gap-4 grid-cols-2 lg:grid-cols-3">
          <div
            v-for="article in articles.slice(1)"
            :key="article.id"
            :ref="(el: any) => registerContent(el, { onSelect: () => openArticle(article) })"
            class="bg-zinc-900/50 rounded-xl overflow-hidden cursor-pointer hover:bg-zinc-900/70 transition-colors"
            @click="openArticle(article)"
          >
            <div v-if="article.imageObjectId" class="aspect-[16/9]">
              <img :src="objectUrl(article.imageObjectId)" :alt="article.title" class="w-full h-full object-cover" loading="lazy" />
            </div>
            <div class="p-4">
              <h3 class="text-sm font-semibold text-zinc-200 mb-1 line-clamp-2">{{ article.title }}</h3>
              <p class="text-xs text-zinc-500 line-clamp-2 mb-2">{{ article.description }}</p>
              <p class="text-xs text-zinc-600">{{ formatDate(article.publishedAt) }}</p>
            </div>
          </div>
        </div>

        <div v-if="articles.length >= 10" class="flex justify-center py-6">
          <button
            :ref="(el: any) => registerContent(el, { onSelect: loadMore })"
            class="px-6 py-2 rounded-lg bg-zinc-800 text-zinc-300 text-sm font-medium hover:bg-zinc-700 transition-colors"
            @click="loadMore"
          >
            Load More
          </button>
        </div>

        <div v-if="articles.length === 0" class="flex items-center justify-center py-24">
          <div class="text-center">
            <NewspaperIcon class="size-16 mx-auto mb-4 text-zinc-600" />
            <h3 class="text-2xl font-semibold text-zinc-400 mb-2">No news yet</h3>
            <p class="text-zinc-600">Articles will appear here when published</p>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ArrowLeftIcon, NewspaperIcon } from "@heroicons/vue/24/outline";
import { useServerApi, type NewsArticle } from "~/composables/use-server-api";
import { serverUrl, rewriteDescriptionImages } from "~/composables/use-server-fetch";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";

definePageMeta({ layout: "bigpicture" });

const api = useServerApi();
const focusNav = useFocusNavigation();
const registerContent = useBpFocusableGroup("content");

const loading = ref(true);
const articles = ref<NewsArticle[]>([]);
const selectedArticle = ref<NewsArticle | null>(null);

const renderedContent = computed(() => {
  if (!selectedArticle.value) return "";
  return rewriteDescriptionImages(selectedArticle.value.content);
});

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

function formatDate(dateStr: string): string {
  return new Date(dateStr).toLocaleDateString("en-US", { year: "numeric", month: "short", day: "numeric" });
}

function openArticle(article: NewsArticle) {
  selectedArticle.value = article;
  nextTick(() => focusNav.autoFocusContent("content"));
}

function closeArticle() {
  selectedArticle.value = null;
  nextTick(() => focusNav.autoFocusContent("content"));
}

async function loadMore() {
  const more = await api.news.list({ skip: articles.value.length, limit: 10 });
  articles.value.push(...more);
}

onMounted(async () => {
  try {
    articles.value = await api.news.list({ limit: 10 });
  } catch (e) {
    console.error("Failed to load news:", e);
  } finally {
    loading.value = false;
    nextTick(() => focusNav.autoFocusContent("content"));
  }
});
</script>
