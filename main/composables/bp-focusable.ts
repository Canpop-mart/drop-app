/**
 * Composable to register an element as focusable in Big Picture mode.
 *
 * Usage in a component's onMounted:
 *
 *   const el = ref<HTMLElement | null>(null);
 *   useBpFocusable(el, "nav", { onSelect: () => router.push("/bigpicture/library") });
 *
 * Or in v-for with a function ref:
 *
 *   const register = useBpFocusableGroup("content");
 *   // In template: :ref="(el) => register(el, { onSelect: () => ... })"
 */

import type { ComponentPublicInstance } from "vue";
import { useFocusNavigation } from "./focus-navigation";

/**
 * Register a single element ref as focusable. Automatically unregisters on unmount.
 */
export function useBpFocusable(
  elRef: Ref<HTMLElement | null>,
  group: string,
  options?: {
    onSelect?: () => void;
    onContext?: () => void;
  },
) {
  const focusNav = useFocusNavigation();
  let unregister: (() => void) | null = null;

  onMounted(() => {
    if (elRef.value) {
      unregister = focusNav.registerElement(elRef.value, group, options);
    }
  });

  onUnmounted(() => {
    unregister?.();
  });
}

/**
 * Returns a registration function for use in v-for template refs.
 * Call the returned function with each element and its options.
 * All registrations are cleaned up on unmount.
 */
export function useBpFocusableGroup(group: string) {
  const focusNav = useFocusNavigation();
  const cleanups: (() => void)[] = [];

  onUnmounted(() => {
    for (const cleanup of cleanups) cleanup();
    cleanups.length = 0;
  });

  return function register(
    el: Element | ComponentPublicInstance | null,
    options?: {
      onSelect?: () => void;
      onContext?: () => void;
      onFocus?: () => void;
    },
  ) {
    if (!el) return;
    const htmlEl = (el as any).$el ?? el;
    if (!(htmlEl instanceof HTMLElement)) return;
    const unregister = focusNav.registerElement(htmlEl, group, options);
    cleanups.push(unregister);
  };
}

/**
 * Like useBpFocusableGroup, but also registers the group as a grid.
 * Grid groups use index-aligned (column-sticky) navigation instead of
 * spatial cone search, which produces predictable movement in tile grids.
 *
 * Usage:
 *   const registerTile = useBpFocusableGrid("content");
 *   // In template: :ref="(el) => registerTile(el, { onSelect: ... })"
 */
export function useBpFocusableGrid(group: string) {
  const focusNav = useFocusNavigation();
  const cleanups: (() => void)[] = [];

  // Register this group as a grid on mount
  onMounted(() => {
    focusNav.registerGrid(group);
  });

  onUnmounted(() => {
    focusNav.unregisterGrid(group);
    for (const cleanup of cleanups) cleanup();
    cleanups.length = 0;
  });

  return function register(
    el: Element | ComponentPublicInstance | null,
    options?: {
      onSelect?: () => void;
      onContext?: () => void;
      onFocus?: () => void;
    },
  ) {
    if (!el) return;
    const htmlEl = (el as any).$el ?? el;
    if (!(htmlEl instanceof HTMLElement)) return;
    const unregister = focusNav.registerElement(htmlEl, group, options);
    cleanups.push(unregister);
  };
}
