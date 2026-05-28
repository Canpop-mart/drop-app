<template>
  <div
    class="w-full h-full flex items-center justify-center"
    :class="bgClass"
  >
    <span
      class="font-display font-bold text-zinc-100/90 drop-shadow-lg select-none"
      :class="textSize"
    >
      {{ initial }}
    </span>
  </div>
</template>

<script setup lang="ts">
/**
 * Styled placeholder for the banner slot when a game has no
 * `mBannerObjectId` (or has one pointing to a tiny icon that would
 * otherwise stretch ridiculously across a 16:9 / 21:9 hero).
 *
 * Mirrors the styled fallback we already ship for cover tiles
 * (`GameTile.vue`): a deterministic gradient background derived from
 * the game name hash + the game's first letter rendered in display
 * type.  The intent is for missing-banner games to look like a
 * deliberate placeholder, not a broken stretched icon.
 *
 * Use this whenever you render a banner image — wrap it in your
 * existing `aspect-[X/Y]` / `rounded-*` parent and render this
 * component inside an `<template v-else>` when the banner ID is null.
 */

const props = withDefaults(
  defineProps<{
    /** The game's display name — first letter is rendered in the
     *  middle, and the whole name hashes into the gradient choice. */
    name: string;
    /** Tailwind text-size for the initial. Defaults to `text-5xl` for
     *  the typical 16:9 card banner; bump it for larger heroes. */
    textSize?: string;
  }>(),
  { textSize: "text-5xl" },
);

// Same gradient palette as GameTile so the per-game look is
// consistent across covers and banners — Super Smash Rex's
// placeholder cover and its placeholder banner pick from the same
// gradient bucket.
const fallbackBgClasses = [
  "bg-gradient-to-br from-blue-600/40 to-purple-700/40",
  "bg-gradient-to-br from-emerald-600/40 to-teal-700/40",
  "bg-gradient-to-br from-amber-600/40 to-orange-700/40",
  "bg-gradient-to-br from-rose-600/40 to-pink-700/40",
  "bg-gradient-to-br from-indigo-600/40 to-violet-700/40",
  "bg-gradient-to-br from-cyan-600/40 to-sky-700/40",
  "bg-gradient-to-br from-fuchsia-600/40 to-purple-700/40",
  "bg-gradient-to-br from-lime-600/40 to-emerald-700/40",
];

function hashStringToIndex(s: string, modulo: number): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) {
    h = (h * 31 + s.charCodeAt(i)) | 0;
  }
  return Math.abs(h) % modulo;
}

const bgClass = computed(
  () => fallbackBgClasses[hashStringToIndex(props.name, fallbackBgClasses.length)],
);

const initial = computed(() => {
  const trimmed = props.name.trim();
  if (trimmed.length === 0) return "?";
  const firstChar = trimmed.charAt(0).toUpperCase();
  // Treat "A Game" as starting with G so we don't render an army of A's
  if (trimmed.length > 1 && firstChar === "A" && trimmed.charAt(1) === " ") {
    const next = trimmed.slice(2).trimStart().charAt(0);
    return next ? next.toUpperCase() : firstChar;
  }
  return firstChar;
});
</script>
