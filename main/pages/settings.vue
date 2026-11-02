<template>
  <div class="mx-auto max-w-[1900px] px-8 py-8">
    <div class="mb-7">
      <h3 class="text-2xl font-bold font-display text-zinc-100">Settings</h3>
      <p class="mt-1 text-sm text-zinc-500">Manage Drop on this device.</p>
    </div>
    <div class="flex flex-row items-start gap-8">
      <nav
        class="sticky top-6 flex w-56 shrink-0 flex-col rounded-2xl bg-zinc-900/60 p-2 ring-1 ring-white/5"
        aria-label="Sidebar"
      >
        <ul role="list" class="-mx-2 space-y-1">
          <li v-for="(item, itemIdx) in navigation" :key="item.prefix">
            <NuxtLink
              :href="item.route"
              :class="[
                itemIdx === currentNavigation
                  ? 'bg-zinc-800/50 text-zinc-100'
                  : 'text-zinc-400 hover:bg-zinc-800/30 hover:text-zinc-200',
                'transition group flex gap-x-3 rounded-xl px-3 py-2.5 text-sm font-semibold leading-6',
              ]"
            >
              <component
                :is="item.icon"
                :class="[
                  itemIdx === currentNavigation
                    ? 'text-zinc-100'
                    : 'text-zinc-400 group-hover:text-zinc-200',
                  'transition h-6 w-6 shrink-0',
                ]"
                aria-hidden="true"
              />
              {{ item.label }}
            </NuxtLink>
          </li>
        </ul>
      </nav>
      <div
        class="min-w-0 grow rounded-2xl bg-zinc-900/30 p-7 ring-1 ring-white/5"
      >
        <NuxtPage />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  ArrowDownTrayIcon,
  CloudIcon,
  CodeBracketIcon,
  CubeIcon,
  HomeIcon,
  RectangleGroupIcon,
  TrophyIcon,
  SignalIcon,
} from "@heroicons/vue/16/solid";
import type { Component } from "vue";
import type { NavigationItem } from "~/types";
import { platform } from "@tauri-apps/plugin-os";
import { UserIcon } from "@heroicons/vue/20/solid";

const appState = useAppState();

// Make navigation reactive by wrapping in computed
const navigation = computed(() => [
  {
    label: "Home",
    route: "/settings",
    prefix: "/settings",
    icon: HomeIcon,
  },
  {
    label: "Interface",
    route: "/settings/interface",
    prefix: "/settings/interface",
    icon: RectangleGroupIcon,
  },
  {
    label: "Downloads",
    route: "/settings/downloads",
    prefix: "/settings/downloads",
    icon: ArrowDownTrayIcon,
  },
  ...(appState.value!.umuState !== "NotNeeded"
    ? [
        {
          label: "Proton",
          route: "/settings/compat",
          prefix: "/settings/compat",
          icon: h("img", { src: usePublicUrl("proton-logo.png") }),
        },
      ]
    : []),
  {
    label: "Achievements",
    route: "/settings/achievements",
    prefix: "/settings/achievements",
    icon: TrophyIcon,
  },
  {
    label: "Cloud Saves",
    route: "/settings/cloudsaves",
    prefix: "/settings/cloudsaves",
    icon: CloudIcon,
  },
  // Not dev-gated: this page holds the only setup UI for remote play, and the
  // Play-on-another-device buttons are visible to everyone, so hiding it left
  // people able to start a stream they could never set up.
  {
    label: "Remote Play",
    route: "/settings/streaming",
    prefix: "/settings/streaming",
    icon: SignalIcon,
  },
  {
    label: "Account",
    route: "/settings/account",
    prefix: "/settings/account",
    icon: UserIcon,
  },
  {
    label: "Developer",
    route: "/settings/developer",
    prefix: "/settings/developer",
    icon: CodeBracketIcon,
  },
]);

const currentPlatform = platform();

// Use .value to unwrap the computed ref
const { currentNavigation } = useCurrentNavigationIndex(navigation.value);

// Watch for navigation changes and update currentPageIndex
watch(navigation, (newNav) => {
  currentNavigation.value =
    useCurrentNavigationIndex(newNav).currentNavigation.value;
});
</script>
