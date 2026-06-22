<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition-opacity duration-200"
      leave-active-class="transition-opacity duration-200"
      enter-from-class="opacity-0"
      leave-to-class="opacity-0"
    >
      <div
        v-if="open"
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
        @click.self="onCancel"
      >
        <div
          class="bg-zinc-900 border border-zinc-700/50 rounded-2xl shadow-2xl p-6 max-w-3xl w-full mx-4 max-h-[85vh] flex flex-col"
        >
          <h2 class="text-lg font-semibold font-display text-zinc-100 mb-4">
            Choose Your Avatar
          </h2>

          <!-- Avatar grid -->
          <div class="flex-1 overflow-y-auto min-h-0">
            <div
              class="grid grid-cols-6 sm:grid-cols-8 md:grid-cols-10 gap-2 p-1"
            >
              <button
                v-for="pic in profilePics"
                :key="pic"
                :ref="(el: any) => onTileRef(el, pic)"
                type="button"
                class="relative aspect-square rounded-xl overflow-hidden ring-2 transition-all hover:scale-105"
                :class="
                  selectedPic === pic
                    ? 'ring-blue-500 shadow-lg shadow-blue-500/20'
                    : 'ring-transparent hover:ring-zinc-600'
                "
                @click="selectPic(pic)"
              >
                <img
                  :src="avatarPicUrl(pic)"
                  :alt="pic"
                  class="size-full object-cover"
                  loading="lazy"
                  @error="onTileImgError"
                />
              </button>
            </div>
          </div>

          <!-- Error message -->
          <p v-if="errorMessage" class="text-xs text-red-400 mt-3 px-1">
            {{ errorMessage }}
          </p>

          <!-- Actions -->
          <div
            class="flex justify-end gap-2 mt-4 pt-3 border-t border-zinc-800"
          >
            <button
              :ref="(el: any) => onCancelRef(el)"
              type="button"
              class="px-4 py-2.5 rounded-xl text-sm text-zinc-300 hover:text-zinc-100 bg-zinc-800/50 hover:bg-zinc-700 transition-colors"
              :class="{ 'opacity-50 pointer-events-none': uploading }"
              @click="onCancel"
            >
              Cancel
            </button>
            <button
              v-if="selectedPic"
              :ref="(el: any) => onConfirmRef(el)"
              type="button"
              class="px-4 py-2.5 rounded-xl text-sm font-medium bg-blue-600 text-white hover:bg-blue-500 transition-colors"
              :class="{ 'opacity-50 pointer-events-none': uploading }"
              @click="confirm"
            >
              {{ uploading ? "Saving..." : "Select Avatar" }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { serverUrl } from "~/composables/use-server-fetch";
import { useServerApi } from "~/composables/use-server-api";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { devLog } from "~/composables/dev-mode";

/**
 * Canonical "preset gallery" profile picture picker.
 *
 * Used by:
 *   - desktop profile edit (`pages/profile/edit.vue`)
 *   - Big Picture profile edit (`pages/bigpicture/profile/edit.vue`)
 *
 * Both surfaces fetch the chosen preset as a blob and POST it to the
 * existing avatar upload endpoint, so the server side stays untouched.
 *
 * In BPM mode (`bpmMode = true`) the component drives `useFocusNavigation`
 * directly so D-pad navigation is scoped to the picker overlay, and
 * restores focus to the previous group when the picker closes. In
 * desktop mode it's a plain modal — the focus calls become no-ops.
 */
const props = withDefaults(
  defineProps<{
    /** Open / close. v-model'd by parent. */
    open: boolean;
    /**
     * When true, register tiles with the BPM focus system and
     * restrict focus to the "avatar-picker" group while open.
     */
    bpmMode?: boolean;
    /**
     * Group to restore focus to when the picker closes (BPM only).
     */
    returnFocusGroup?: string;
  }>(),
  {
    bpmMode: false,
    returnFocusGroup: "content",
  },
);

const emit = defineEmits<{
  /** Picker dismissed (cancel button, backdrop click, or B button). */
  (e: "close"): void;
  /** Avatar successfully uploaded — payload is the new object id. */
  (e: "selected", profilePictureObjectId: string): void;
  /** Upload failed — payload is a human-readable message. */
  (e: "error", message: string): void;
}>();

// ── Preset gallery (Xbox 360-style picks) ─────────────────────────────────
// Sourced from `public/img/boxart/profilepic/`. Adding new presets only
// requires dropping files in that directory and extending this list.
const profilePics = [
  "20000.png", "20000(1).png", "20000(2).png", "20001.png", "20001(1).png",
  "20002.png", "20002(1).png", "20003.png", "20003(1).png", "20004.png",
  "20005.png", "20006.png", "20007.png",
  "20008.png", "20009.png", "2000a.png", "2000b.png", "2000c.png",
  "2000d.png", "2000d(1).png", "2007b.png",
  "20400.png", "20402.png", "20405.png", "20406.png", "2041c.png", "20429.png", "20431.png",
  "21000.png", "21001.png", "21002.png", "21003.png", "21004.png", "21005.png",
  "21006.png", "21007.png", "21008.png", "21009.png", "21010.png", "21011.png",
  "21012.png", "21013.png", "21014.png", "21015.png", "21016.png", "21017.png",
  "21018.png", "21019.png", "21020.png", "21021.png", "21022.png", "21023.png",
  "21024.png", "21025.png", "21026.png", "21027.png", "21029.png", "21030.png",
  "21031.png", "21032.png", "21033.png", "21035.png", "21036.png", "21037.png",
  "21038.png", "21039.png", "21040.png", "21041.png", "21042.png", "21043.png",
  "21044.png", "21045.png", "21046.png", "21047.png", "21048.png", "21049.png",
  "21050.png", "21051.png", "21052.png", "21053.png", "21054.png", "21055.png",
  "21056.png", "21057.png", "21058.png", "21059.png", "21060.png", "21061.png",
  "21062.png", "21063.png", "21064.png", "21065.png", "21066.png", "21067.png",
  "21068.png", "21069.png",
  "28000.png", "28006.png", "28008.png", "28017.png", "2801a.png", "28026.png", "2802e.png", "2808e.png",
];

// Resolve preset path against Nuxt's baseURL so it works in both the
// Tauri WebView (baseURL "/main") and a plain web build.
const _runtimeBase = (useRuntimeConfig().app.baseURL ?? "/").replace(
  /\/$/,
  "",
);
const _profilePicBase = `${_runtimeBase}/img/boxart/profilepic`;

function avatarPicUrl(filename: string): string {
  // Encode filename so parens / unicode in preset names don't break the
  // request URL. The trailing extension already URL-safe.
  return `${_profilePicBase}/${encodeURIComponent(filename)}`;
}

// ── State ────────────────────────────────────────────────────────────────

const selectedPic = ref<string | null>(null);
const uploading = ref(false);
const errorMessage = ref<string | null>(null);
const api = useServerApi();

// Reset state whenever the picker re-opens so reopening after a cancel
// doesn't show stale selection / error UI.
watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      selectedPic.value = null;
      errorMessage.value = null;
      uploading.value = false;
      if (props.bpmMode) {
        // Activate BPM focus restriction once the overlay's children have
        // had a chance to register via the :ref callbacks.
        nextTick(() => {
          focusNav.restrictFocus(FOCUS_GROUP);
          nextTick(() => focusNav.autoFocusContent(FOCUS_GROUP));
        });
      }
    } else if (props.bpmMode) {
      // Release the restriction whenever the picker closes for any reason.
      focusNav.unrestrictFocus(props.returnFocusGroup);
    }
  },
);

// ── BPM focus registration ───────────────────────────────────────────────

const FOCUS_GROUP = "avatar-picker";

// Always create the helpers — they no-op outside BPM (no calls happen
// because we gate the :ref handlers below). This keeps the composable
// lifecycle hooks (`onUnmounted`) registered in script-setup, which
// matters when `bpmMode` flips at runtime.
const focusNav = useFocusNavigation();
const registerPickerTile = useBpFocusableGroup(FOCUS_GROUP);

function onTileRef(el: any, pic: string) {
  if (!props.bpmMode) return;
  registerPickerTile(el, { onSelect: () => selectPic(pic) });
}

function onCancelRef(el: any) {
  if (!props.bpmMode) return;
  registerPickerTile(el, { onSelect: onCancel });
}

function onConfirmRef(el: any) {
  if (!props.bpmMode) return;
  registerPickerTile(el, { onSelect: confirm });
}

// ── Tile interactions ────────────────────────────────────────────────────

function selectPic(pic: string) {
  selectedPic.value = pic;
  errorMessage.value = null;
}

function onTileImgError(e: Event) {
  // Don't crash if a preset image 404s — just leave it blank.
  const target = e.target as HTMLImageElement | null;
  if (target) target.style.opacity = "0.3";
}

// ── Confirm / cancel ─────────────────────────────────────────────────────

async function confirm() {
  if (!selectedPic.value || uploading.value) return;
  uploading.value = true;
  errorMessage.value = null;
  try {
    const imgUrl = avatarPicUrl(selectedPic.value);
    devLog("state", "[ProfilePicturePicker] fetching preset:", imgUrl);
    const res = await fetch(imgUrl);
    if (!res.ok) {
      throw new Error(`Couldn't load preset image (${res.status})`);
    }
    const blob = await res.blob();
    devLog(
      "state",
      "[ProfilePicturePicker] blob size:",
      blob.size,
      "type:",
      blob.type,
    );
    const file = new File([blob], selectedPic.value, {
      type: blob.type || "image/png",
    });
    const result = await api.profile.uploadAvatar(file);
    devLog(
      "state",
      "[ProfilePicturePicker] upload ok, new id:",
      result.profilePictureObjectId,
    );
    emit("selected", result.profilePictureObjectId);
    // Parent decides what `open` becomes; we still proactively emit close
    // so a parent that only listens to "close" tears down the overlay.
    emit("close");
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error("[ProfilePicturePicker] upload failed:", err);
    errorMessage.value = `Couldn't save avatar: ${msg}`;
    emit("error", msg);
  } finally {
    uploading.value = false;
  }
}

function onCancel() {
  if (uploading.value) return;
  emit("close");
}

// Make sure we don't leak the focus restriction if the picker is
// destroyed mid-flight (e.g. parent unmounts while open).
onBeforeUnmount(() => {
  if (props.bpmMode && props.open) {
    focusNav.unrestrictFocus(props.returnFocusGroup);
  }
});

defineExpose({
  /** Imperative cancel — useful when wiring a B-button outside the component. */
  cancel: onCancel,
});
</script>
