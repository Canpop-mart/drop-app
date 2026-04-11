<template>
  <nav
    class="flex flex-col items-center w-20 bg-zinc-950 border-r border-zinc-800/50 py-6 gap-2 shrink-0"
  >
    <!-- Logo -->
    <div class="mb-6">
      <Logo class="size-10" />
    </div>

    <!-- Nav items -->
    <NuxtLink
      v-for="item in navItems"
      :key="item.route"
      :ref="
        (el: any) => registerNav(el, { onSelect: () => selectNavItem(item.route) })
      "
      :to="item.route"
      class="group relative flex items-center justify-center w-14 h-14 rounded-xl transition-all duration-200"
      :class="[
        isActive(item.route)
          ? 'bg-blue-600/20 text-blue-400 shadow-lg shadow-blue-500/10'
          : 'text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800/50',
      ]"
    >
      <component :is="item.icon" class="size-6" />

      <!-- Active indicator -->
      <div
        v-if="isActive(item.route)"
        class="absolute left-0 top-1/2 -translate-y-1/2 -translate-x-1 w-1 h-8 bg-blue-500 rounded-r-full"
      />

      <!-- Tooltip -->
      <div
        class="absolute left-full ml-3 px-2.5 py-1 bg-zinc-800 text-zinc-200 text-xs font-medium rounded-md opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-50"
      >
        {{ item.label }}
      </div>
    </NuxtLink>

    <div class="flex-1" />

    <!-- Exit Big Picture -->
    <button
      :ref="(el: any) => registerNav(el, { onSelect: () => exitBigPicture() })"
      class="flex items-center justify-center w-14 h-14 rounded-xl text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800/50 transition-all duration-200"
      @click="exitBigPicture"
    >
      <ArrowsPointingInIcon class="size-6" />
    </button>
  </nav>
</template>

<script setup lang="ts">
import {
  HomeIcon,
  Square3Stack3DIcon,
  ShoppingBagIcon,
  ChatBubbleLeftRightIcon,
  NewspaperIcon,
  Cog6ToothIcon,
  ArrowDownTrayIcon,
  ArrowsPointingInIcon,
} from "@heroicons/vue/24/outline";
import { useBigPictureMode } from "~/composables/big-picture";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";

const router = useRouter();
const route = useRoute();
const bigPicture = useBigPictureMode();
const registerNav = useBpFocusableGroup("nav");
const focusNav = useFocusNavigation();

// Set group order: nav rail first, then content
onMounted(() => {
  focusNav.setGroupOrder(["nav", "content"]);
});

/**
 * Navigate to a page and attempt to move focus into the content group.
 * We wait for the next tick (so the page component mounts and registers
 * its focusable elements) and then try to focus the content group.
 */
async function selectNavItem(path: string) {
  await router.push(path);

  // Wait for the new page to mount and register focus elements
  await nextTick();
  // Small extra delay for async component setup
  setTimeout(() => {
    focusNav.focusGroup("content");
  }, 100);
}

const navItems = [
  {
    route: "/bigpicture/library",
    icon: Square3Stack3DIcon,
    label: "Library",
  },
  { route: "/bigpicture/store", icon: ShoppingBagIcon, label: "Store" },
  {
    route: "/bigpicture/community",
    icon: ChatBubbleLeftRightIcon,
    label: "Community",
  },
  {
    route: "/bigpicture/news",
    icon: NewspaperIcon,
    label: "News",
  },
  {
    route: "/bigpicture/downloads",
    icon: ArrowDownTrayIcon,
    label: "Downloads",
  },
  {
    route: "/bigpicture/settings",
    icon: Cog6ToothIcon,
    label: "Settings",
  },
];

function isActive(navRoute: string): boolean {
  if (navRoute === "/bigpicture") {
    return route.path === "/bigpicture";
  }
  return route.path.startsWith(navRoute);
}

function exitBigPicture() {
  bigPicture.exit();
}
</script>