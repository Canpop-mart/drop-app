<template>
  <button
    class="group flex w-full text-left transition-transform duration-200"
    :class="
      compact
        ? 'items-center gap-3 rounded-lg hover:bg-zinc-800/40 p-1.5'
        : 'flex-col hover:-translate-y-1'
    "
    @click="$emit('select')"
  >
    <!-- Cover slot. Compact mode renders a small square thumb; the
         default vertical mode keeps the 3:4 box art proportion the
         rest of the app uses. -->
    <div
      class="relative overflow-hidden ring-1 ring-zinc-700/50 group-hover:ring-blue-500/50 transition-colors"
      :class="[
        compact ? 'shrink-0 size-12 rounded-md' : 'aspect-[3/4] rounded-xl',
        coverUrl ? 'bg-zinc-800' : fallbackBgClass,
      ]"
    >
      <img
        v-if="coverUrl"
        :src="coverUrl"
        :alt="name"
        class="w-full h-full object-cover"
        loading="lazy"
      />
      <!-- Styled fallback when there's no cover. Big initial on a
           gradient that's derived from the game name so it's stable
           per-title instead of looking like a broken-image placeholder. -->
      <div
        v-else
        class="w-full h-full flex items-center justify-center"
      >
        <span
          class="font-display font-bold text-zinc-100/90 drop-shadow"
          :class="compact ? 'text-base' : 'text-4xl'"
        >
          {{ fallbackInitial }}
        </span>
      </div>

      <!-- "ROM" marker — top-left, for emulated games. Hidden in compact
           mode where it'd dominate the small thumbnail. -->
      <div
        v-if="rom && !compact"
        class="absolute top-2 left-2 px-1.5 py-0.5 rounded text-[10px] font-bold uppercase bg-purple-500/30 text-purple-200 backdrop-blur-sm"
      >
        ROM
      </div>

      <!-- Badge stack — top-right. "Update" trumps the plain installed dot.
           In compact mode we drop these onto the thumb as a single dot
           since the row already shows status text. -->
      <div
        v-if="!compact"
        class="absolute top-2 right-2 flex flex-col gap-1 items-end"
      >
        <span
          v-if="updateAvailable"
          class="px-1.5 py-0.5 rounded text-[10px] font-bold uppercase bg-blue-500/30 text-blue-200 backdrop-blur-sm"
        >
          Update
        </span>
        <span
          v-else-if="installed"
          class="size-2 rounded-full bg-green-500 shadow-lg shadow-green-500/50"
          title="Installed"
        />
      </div>

      <!-- Hover gradient so the cover doesn't look static under the
           cursor. Skipped in compact mode (the row hover background
           covers it). -->
      <div
        v-if="!compact"
        class="absolute inset-0 bg-gradient-to-t from-zinc-950/80 via-transparent to-transparent opacity-0 group-hover:opacity-100 transition-opacity"
      />

      <!-- Hover "Play / Install" overlay — appears on the cover in
           default mode only when a hover action is requested by the
           parent. Skipped in compact mode. -->
      <div
        v-if="!compact && hoverAction"
        class="absolute inset-0 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
      >
        <span
          class="inline-flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-semibold text-white shadow-lg backdrop-blur-sm"
          :class="
            hoverAction === 'play'
              ? 'bg-blue-600/90'
              : 'bg-zinc-800/90 ring-1 ring-zinc-600/40'
          "
        >
          <svg
            v-if="hoverAction === 'play'"
            class="size-3"
            viewBox="0 0 24 24"
            fill="currentColor"
          >
            <path d="M8 5v14l11-7z" />
          </svg>
          <svg
            v-else
            class="size-3"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M12 5v14M5 12h14" />
          </svg>
          {{ hoverAction === "play" ? "Play" : "Install" }}
        </span>
      </div>
    </div>

    <!-- Name + meta. In compact mode they sit to the right of the thumb;
         in default mode they stack below the cover. -->
    <div
      :class="compact ? 'flex-1 min-w-0' : 'mt-2 w-full min-w-0'"
    >
      <p
        class="text-xs font-medium truncate transition-colors"
        :class="
          compact
            ? 'text-sm text-zinc-100 group-hover:text-blue-300'
            : 'text-zinc-300 group-hover:text-zinc-100'
        "
      >
        {{ name }}
      </p>
      <!-- Meta line: shows last-played hint, install state in compact
           mode, or update badge text. Stays out of the way in default
           mode unless `lastPlayed` is passed in. -->
      <p
        v-if="metaLine"
        class="text-[11px] text-zinc-500 truncate mt-0.5"
      >
        {{ metaLine }}
      </p>
    </div>
  </button>
</template>

<script setup lang="ts">
/**
 * Single store/library game tile — the one canonical "box-art card".
 *
 * Two render modes:
 *   - default        (vertical) 3:4 cover above the title; what the store
 *                    and library grids use.
 *   - compact        (horizontal) small square thumb + title + meta;
 *                    higher-density list view for big libraries.
 *
 * Deliberately presentational: takes an already-resolved `coverUrl`
 * rather than an object ID, because callers resolve through two
 * different protocols — `useObject()` for native `object://` IDs, and
 * `serverUrl("api/v1/object/...")` for store metadata.
 */
const props = defineProps<{
  /** Fully-resolved cover image URL, or empty/undefined for the fallback. */
  coverUrl?: string | null;
  /** Display name — also the fallback text when there's no cover. */
  name: string;
  /** Show the green "installed" dot. */
  installed?: boolean;
  /** Show the blue "Update" pill (takes precedence over the dot). */
  updateAvailable?: boolean;
  /** Show the purple "ROM" pill for emulated games. */
  rom?: boolean;
  /** Corner radius — `xl` (default) for the main grids, `lg` for denser ones. */
  rounded?: "lg" | "xl";
  /**
   * Compact list-style layout — a horizontal row with a small thumb,
   * title, and meta line. Used by the library's "density toggle" and
   * the collections-management page.
   */
  compact?: boolean;
  /**
   * Hover overlay action — shows a Play / Install pill on hover so the
   * user can launch without navigating into the detail page. The parent
   * decides which action to surface; the tile only emits `select` on
   * click and lets the parent intercept.
   */
  hoverAction?: "play" | "install" | null;
  /**
   * Optional "last played" ISO timestamp — when present, renders a
   * relative-time meta line under the title ("3 hours ago", "yesterday").
   */
  lastPlayed?: string | null;
}>();

defineEmits<{
  (e: "select"): void;
}>();

// Stable hash → background gradient class for the cover fallback. The
// goal is "this looks intentional per-game" rather than "this looks
// broken." Cycling through eight gradients keeps the palette consistent
// with the rest of the dark UI.
const fallbackBgClasses = [
  "bg-gradient-to-br from-blue-600/30 to-purple-700/30",
  "bg-gradient-to-br from-emerald-600/30 to-teal-700/30",
  "bg-gradient-to-br from-amber-600/30 to-orange-700/30",
  "bg-gradient-to-br from-rose-600/30 to-pink-700/30",
  "bg-gradient-to-br from-indigo-600/30 to-violet-700/30",
  "bg-gradient-to-br from-cyan-600/30 to-sky-700/30",
  "bg-gradient-to-br from-fuchsia-600/30 to-purple-700/30",
  "bg-gradient-to-br from-lime-600/30 to-emerald-700/30",
];

function hashStringToIndex(s: string, modulo: number): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) {
    h = (h * 31 + s.charCodeAt(i)) | 0;
  }
  return Math.abs(h) % modulo;
}

const fallbackBgClass = computed(
  () => fallbackBgClasses[hashStringToIndex(props.name, fallbackBgClasses.length)],
);

const fallbackInitial = computed(() => {
  const trimmed = props.name.trim();
  if (trimmed.length === 0) return "?";
  // Use the first non-space character; if the first word is a single
  // letter article ("A Game"), grab the second word's first letter so
  // the badge isn't just "A" everywhere.
  const firstChar = trimmed.charAt(0).toUpperCase();
  if (trimmed.length > 1 && firstChar === "A" && trimmed.charAt(1) === " ") {
    const next = trimmed.slice(2).trimStart().charAt(0);
    return next ? next.toUpperCase() : firstChar;
  }
  return firstChar;
});

function formatRelativeTime(iso: string): string {
  const then = new Date(iso).getTime();
  if (!Number.isFinite(then)) return "";
  const diffMs = Date.now() - then;
  const seconds = Math.max(0, Math.floor(diffMs / 1000));
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  if (days >= 30) {
    const months = Math.floor(days / 30);
    return `${months} month${months === 1 ? "" : "s"} ago`;
  }
  if (days >= 2) return `${days} days ago`;
  if (days === 1) return "yesterday";
  if (hours >= 1) return `${hours} hour${hours === 1 ? "" : "s"} ago`;
  if (minutes >= 1) return `${minutes} min ago`;
  return "just now";
}

// Compose the meta line. Compact mode is the chatty surface: it shows
// install state alongside the last-played hint so each row stands on
// its own. Default mode is quieter — only render the line when
// last-played is supplied, since the grid is otherwise visually busy.
const metaLine = computed(() => {
  const lastPlayedText = props.lastPlayed
    ? `Played ${formatRelativeTime(props.lastPlayed)}`
    : "";
  if (props.compact) {
    const parts: string[] = [];
    if (props.updateAvailable) parts.push("Update available");
    else if (props.installed) parts.push("Installed");
    else parts.push("Not installed");
    if (lastPlayedText) parts.push(lastPlayedText);
    return parts.join(" · ");
  }
  return lastPlayedText;
});
</script>
