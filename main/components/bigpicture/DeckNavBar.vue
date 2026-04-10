<template>
  <nav
    class="flex items-stretch justify-around w-full bg-zinc-950/95 backdrop-blur-md border-t border-zinc-800/40 shrink-0"
    :class="safeAreaPadding"
  >
    <template v-for="item in navItems" :key="item.route">
      <!-- H1 fix: Exit button uses <button> instead of <NuxtLink> -->
      <button
        v-if="item.route === '__exit__'"
        :ref="
          (el: any) => registerNav(el, { onSelect: () => selectNavItem(item.route) })
        "
        class="flex flex-col items-center justify-center gap-0.5 flex-1 py-2 transition-colors relative text-zinc-500 active:text-zinc-300"
        @click="selectNavItem(item.route)"
      >
        <component :is="item.icon" class="size-5" />
        <span class="text-[10px] font-medium leading-tight">{{
          item.label
        }}</span>
      </button>
      <NuxtLink
        v-else
        :ref="
          (el: any) => registerNav(el, { onSelect: () => selectNavItem(item.route) })
        "
        :to="item.route"
        class="flex flex-col items-center justify-center gap-0.5 flex-1 py-2 transition-colors relative"
        :class="[
          isActive(item.route)
            ? 'text-blue-400'
            : 'text-zinc-500 active:text-zinc-300',
        ]"
      >
        <!-- Active pill indicator -->
        <div
          v-if="isActive(item.route)"
          class="absolute top-0 left-1/2 -translate-x-1/2 w-8 h-0.5 bg-blue-500 rounded-full"
        />
        <component :is="item.icon" class="size-5" />
        <span class="text-[10px] font-medium leading-tight">{{
          item.label
        }}</span>
      </NuxtLink>
    </template>
  </nav>
</template>

<script setup lang="ts">
import {
  Square3Stack3DIcon,
  ShoppingBagIcon,
  ChatBubbleLeftRightIcon,
  ArrowDownTrayIcon,
  Cog6ToothIcon,
  ArrowsPointingInIcon,
} from "@heroicons/vue/24/outline";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useBigPictureMode } from "~/composables/big-picture";

const router = useRouter();
const route = useRoute();
const registerNav = useBpFocusableGroup("nav");
const focusNav = useFocusNavigation();
const bigPicture = useBigPictureMode();

onMounted(() => {
  focusNav.setGroupOrder(["nav", "content"]);
});

async function selectNavItem(path: string) {
  // H1 fix: handle exit action
  if (path === "__exit__") {
    bigPicture.exit();
    return;
  }

  await router.push(path);
  await nextTick();
  setTimeout(() => {
    focusNav.focusGroup("content");
  }, 100);
}

const navItems = [
  { route: "/bigpicture/library", icon: Square3Stack3DIcon, label: "Library" },
  { route: "/bigpicture/store", icon: ShoppingBagIcon, label: "Store" },
  {
    route: "/bigpicture/community",
    icon: ChatBubbleLeftRightIcon,
    label: "Community",
  },
  {
    route: "/bigpicture/downloads",
    icon: ArrowDownTrayIcon,
    label: "Downloads",
  },
  { route: "/bigpicture/settings", icon: Cog6ToothIcon, label: "Settings" },
  // H1 fix: Exit BPM button so Deck users can leave fullscreen
  { route: "__exit__", icon: ArrowsPointingInIcon, label: "Exit" },
];

function isActive(navRoute: string): boolean {
  return route.path.startsWith(navRoute);
}

// Bottom safe area padding (for Steam Deck notch / rounded corners)
const safeAreaPadding = "pb-safe-area-inset-bottom";
</script>