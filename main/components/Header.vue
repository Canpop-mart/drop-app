<template>
  <div class="h-16 bg-zinc-950 flex flex-row justify-between">
    <div class="flex flex-row grow items-center pl-5 pr-2 py-3">
      <div class="inline-flex items-center gap-x-6">
        <NuxtLink to="/store">
          <Wordmark class="h-8 mb-0.5" />
        </NuxtLink>
        <!-- Browser-style back / forward arrows. The composable backs
             onto window.history.state.position so the buttons disable
             cleanly at the ends of the session history — clicking
             "back" on the first page is a no-op, not a confusing
             flash. Sits between the wordmark and the nav links for
             the same reading order browser chrome uses. -->
        <div class="flex items-center gap-1">
          <button
            class="rounded-md p-1.5 transition-colors"
            :class="
              navHistory.canGoBack.value
                ? 'text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100'
                : 'text-zinc-700 cursor-not-allowed'
            "
            :disabled="!navHistory.canGoBack.value"
            aria-label="Go back"
            title="Back"
            @click="navHistory.back()"
          >
            <ChevronLeftIcon class="size-5" />
          </button>
          <button
            class="rounded-md p-1.5 transition-colors"
            :class="
              navHistory.canGoForward.value
                ? 'text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100'
                : 'text-zinc-700 cursor-not-allowed'
            "
            :disabled="!navHistory.canGoForward.value"
            aria-label="Go forward"
            title="Forward"
            @click="navHistory.forward()"
          >
            <ChevronRightIcon class="size-5" />
          </button>
        </div>
        <nav class="inline-flex items-center mt-0.5">
          <ol class="inline-flex items-center gap-x-6">
            <NuxtLink
              v-for="(nav, navIdx) in navigation"
              :class="[
                'transition  uppercase font-display font-semibold text-md',
                navIdx === currentNavigation
                  ? 'text-zinc-100'
                  : 'text-zinc-400 hover:text-zinc-200',
              ]"
              :href="nav.route"
            >
              {{ nav.label }}
            </NuxtLink>
          </ol>
        </nav>
      </div>
      <div
        @mousedown="() => window.startDragging()"
        class="flex cursor-pointer grow h-full"
      />
      <div class="inline-flex items-center">
        <ol class="inline-flex gap-3">
          <!-- Big Picture Mode toggle -->
          <li>
            <HeaderWidget @click="enterBigPicture">
              <ArrowsPointingOutIcon class="h-5" />
            </HeaderWidget>
          </li>
          <HeaderProtonSupportWidget />
          <HeaderQueueWidget :object="currentQueueObject" />
          <li v-for="(item, itemIdx) in quickActions">
            <HeaderWidget
              @click="item.action"
              :notifications="item.notifications"
            >
              <component class="h-5" :is="item.icon" />
            </HeaderWidget>
          </li>
          <OfflineHeaderWidget v-if="state?.status === AppStatus.Offline" />
          <HeaderUserWidget />
        </ol>
      </div>
    </div>
    <WindowControl />
  </div>
</template>

<script setup lang="ts">
import { BellIcon, UserGroupIcon, BugAntIcon } from "@heroicons/vue/16/solid";
import {
  ArrowsPointingOutIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
} from "@heroicons/vue/24/outline";
import { AppStatus, type NavigationItem, type QuickActionNav } from "../types";
import HeaderWidget from "./HeaderWidget.vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useBigPictureMode } from "~/composables/big-picture";
import { useNavHistory } from "~/composables/use-nav-history";

const window = getCurrentWindow();
const state = useAppState();
const navHistory = useNavHistory();

const navigation: Array<NavigationItem> = [
  {
    prefix: "/store",
    route: "/store",
    label: "Store",
  },
  {
    prefix: "/library",
    route: "/library",
    label: "Library",
  },
  {
    prefix: "/community",
    route: "/community",
    label: "Community",
  },
  {
    prefix: "/news",
    route: "/news",
    label: "News",
  },
  {
    prefix: "/requests",
    route: "/requests",
    label: "Requests",
  },
];

const { currentNavigation } = useCurrentNavigationIndex(navigation);

const router = useRouter();
const quickActions: Array<QuickActionNav> = [
  {
    icon: UserGroupIcon,
    action: async () => {},
  },
  {
    icon: BellIcon,
    action: async () => {},
  },
  {
    icon: BugAntIcon,
    action: async () => {
      await router.push("/bugreport");
    },
  },
];

const queue = useQueueState();
const currentQueueObject = computed(() => queue.value.queue.at(0));

const bigPicture = useBigPictureMode();
function enterBigPicture() {
  bigPicture.enter();
}
</script>
