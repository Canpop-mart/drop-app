<template>
  <div class="space-y-5">
    <!-- Summary strip. Reads at TV distance and saves counting cards. -->
    <div class="flex items-center gap-3 px-1">
      <h3 class="text-sm font-semibold" style="color: var(--bpm-muted)">MODS</h3>
      <span class="text-sm" style="color: var(--bpm-text)">
        {{ installedCount }} of {{ mods.cards.value.length }} installed
      </span>
      <span
        v-if="mods.installingModId.value"
        class="text-sm text-blue-300"
      >
        Installing…
      </span>
    </div>

    <p
      v-if="mods.modError.value"
      class="rounded-xl px-4 py-3 text-sm text-red-300"
      style="background-color: rgba(239, 68, 68, 0.1)"
    >
      {{ mods.modError.value }}
    </p>

    <!-- Card grid. Deliberately a grid, not a list: two axes of movement give
         the d-pad somewhere to go and keep the mod art large enough to read
         across a room. -->
    <div class="grid grid-cols-1 gap-4 md:grid-cols-2 2xl:grid-cols-3">
      <div
        v-for="mod in mods.cards.value"
        :key="mod.id"
        :ref="(el: any) => registerAction(el, { onSelect: () => activate(mod.id) })"
        class="bp-focus-delegate cursor-pointer"
        @click="activate(mod.id)"
      >
        <div
          class="bp-focus-ring flex h-full flex-col gap-3 rounded-xl p-5 transition-transform hover:scale-[1.02]"
          style="background-color: var(--bpm-surface)"
        >
          <div class="flex items-start gap-4">
            <img
              v-if="mod.iconObjectId"
              :src="objectUrl(mod.iconObjectId)"
              class="size-14 flex-shrink-0 rounded-lg object-cover"
              loading="lazy"
            />
            <div
              v-else
              class="flex size-14 flex-shrink-0 items-center justify-center rounded-lg text-xl font-bold"
              style="background-color: var(--bpm-bg); color: var(--bpm-muted)"
            >
              {{ mod.name[0]?.toUpperCase() ?? "?" }}
            </div>

            <div class="min-w-0 flex-1">
              <p
                class="truncate font-display text-base font-semibold"
                style="color: var(--bpm-text)"
              >
                {{ mod.name }}
              </p>
              <p
                v-if="mod.description"
                class="mt-0.5 line-clamp-2 text-sm"
                style="color: var(--bpm-muted)"
              >
                {{ mod.description }}
              </p>
              <p
                v-else-if="mod.unlisted"
                class="mt-0.5 text-sm"
                style="color: var(--bpm-muted)"
              >
                Installed on this game, but the server no longer lists it.
              </p>
            </div>

            <span
              v-if="mod.installed"
              class="flex-shrink-0 rounded-full px-2.5 py-1 text-xs font-semibold text-emerald-300"
              style="background-color: rgba(16, 185, 129, 0.15)"
            >
              Installed
            </span>
            <span
              v-else-if="queuedIds.includes(mod.id)"
              class="flex-shrink-0 rounded-full px-2.5 py-1 text-xs font-semibold text-blue-300"
              style="background-color: rgba(59, 130, 246, 0.15)"
            >
              Queued
            </span>
          </div>

          <p v-if="mod.requires.length > 0" class="text-xs text-amber-300/90">
            Needs {{ mod.requires.join(", ") }}
          </p>

          <div class="mt-auto flex items-center justify-between pt-1">
            <span class="text-xs" style="color: var(--bpm-muted)">
              <template v-if="mod.fileCount !== null">
                {{ mod.fileCount }} {{ mod.fileCount === 1 ? "file" : "files" }}
              </template>
            </span>
            <!-- No prompt on a queued mod: A would queue the same download a
                 second time, so the card deliberately has no action left. -->
            <BigPictureButtonPrompt
              v-if="actionLabel(mod)"
              button="A"
              :label="actionLabel(mod)"
              size="sm"
            />
            <span v-else class="text-xs" style="color: var(--bpm-muted)">
              Download in progress
            </span>
          </div>
        </div>
      </div>
    </div>

    <p
      v-if="mods.loaded.value && mods.cards.value.length === 0"
      class="px-1 text-sm"
      style="color: var(--bpm-muted)"
    >
      No mods are available for this game yet.
    </p>

    <!-- Prerequisite confirmation. Installing one mod can pull in several
         others, so it asks before queueing them. -->
    <BigPictureDialog
      :visible="mods.prereqOpen.value"
      title="Install required mods?"
      :message="prereqMessage"
      confirm-label="Install all"
      cancel-label="Cancel"
      @confirm="mods.answerPrereqs(true)"
      @cancel="mods.answerPrereqs(false)"
    />

    <!-- Uninstall confirmation. A single A press must never delete files. -->
    <BigPictureDialog
      :visible="mods.modToUninstall.value !== null"
      title="Uninstall mod"
      :message="uninstallMessage"
      confirm-label="Uninstall"
      cancel-label="Cancel"
      :destructive="true"
      @confirm="mods.confirmUninstall()"
      @cancel="mods.cancelUninstall()"
    >
      <p
        v-if="mods.uninstallDependents.value.length > 0"
        class="rounded-lg px-4 py-3 text-sm text-amber-300"
        style="background-color: rgba(245, 158, 11, 0.1)"
      >
        {{ mods.uninstallDependents.value.join(", ") }}
        {{ mods.uninstallDependents.value.length === 1 ? "needs" : "need" }}
        this mod and may stop working.
      </p>
    </BigPictureDialog>
  </div>
</template>

<script setup lang="ts">
import BigPictureDialog from "~/components/bigpicture/BigPictureDialog.vue";
import BigPictureButtonPrompt from "~/components/bigpicture/BigPictureButtonPrompt.vue";
import { objectImageUrl } from "~/composables/use-object";
import type { useBpmMods } from "~/composables/bigpicture/use-bpm-mods";
import type { ModCard } from "~/composables/game-detail/mods-tab";

const props = defineProps<{
  /** The object returned by the page's `useBpmMods()` call. */
  mods: ReturnType<typeof useBpmMods>;
  /** The name of the base game, for the uninstall confirmation copy. */
  gameName: string;
  /** The page's focus-nav action registrar (the "content" group). */
  registerAction: (
    el: any,
    opts: { onSelect: () => void },
  ) => void;
}>();

function objectUrl(id: string): string {
  return objectImageUrl(id);
}

/**
 * Mods queued this visit. The download finishes through the queue long after
 * `installMod` resolves, so without this the card snaps straight back to
 * "Install" and the A press looks like it did nothing. `installed` wins over
 * this, so it clears itself once the mod actually lands.
 */
const queuedIds = ref<string[]>([]);

/** The action A performs on this card, or "" when it has none. */
function actionLabel(mod: ModCard): string {
  if (props.mods.uninstallingModId.value === mod.id) return "Uninstalling…";
  if (props.mods.installingModId.value === mod.id) return "Installing…";
  if (mod.installed) return "Uninstall";
  if (queuedIds.value.includes(mod.id)) return "";
  return "Install";
}

/**
 * A on a card: install it, or open the uninstall confirmation.
 *
 * Takes an id, not the card object: the focus system registers `onSelect` once
 * per DOM element and keeps that first closure for the element's whole life,
 * so a captured card would still say "not installed" after the install landed.
 * Busy cards ignore the press rather than queueing the same work twice.
 */
async function activate(modId: string) {
  if (props.mods.installingModId.value !== null) return;
  if (props.mods.uninstallingModId.value !== null) return;
  const mod = props.mods.cards.value.find((c) => c.id === modId);
  if (!mod) return;
  if (!mod.installed && queuedIds.value.includes(mod.id)) return;
  if (mod.installed) {
    props.mods.askUninstall(mod.id);
    return;
  }
  const queued = await props.mods.installMod(mod.id);
  if (queued > 0 && !queuedIds.value.includes(mod.id)) {
    queuedIds.value = [...queuedIds.value, mod.id];
  }
}

const installedCount = computed(
  () => props.mods.cards.value.filter((m) => m.installed).length,
);

const prereqMessage = computed(() => {
  const names = props.mods.prereqNames.value;
  const single = names.length === 1;
  return (
    `This mod also needs ${single ? "another mod" : "other mods"}: ` +
    `${names.join(", ")}. Drop will install ${single ? "it" : "them"} ` +
    `onto this game too.`
  );
});

const uninstallMessage = computed(
  () =>
    `Remove ${props.mods.uninstallName.value} from ${props.gameName || "this game"}? ` +
    `Only this mod's files are removed. The base game is left alone.`,
);

/**
 * Both dialogs live in this tab, but the state behind them lives on the page,
 * which outlives it. `installMod` does several awaits before it asks about
 * prerequisites and holds no input lock while it does, so the user can leave
 * for another tab in that window — and then the question is asked with nothing
 * mounted to answer it. `installMod` waits on that promise forever, keeping
 * `installingModId` set, which makes every card refuse the next A press.
 *
 * Answering "no" on the way out resolves it, so the install returns 0 and its
 * `finally` clears the id. The uninstall confirmation has the same shape, and
 * the same cure.
 */
onUnmounted(() => {
  props.mods.answerPrereqs(false);
  props.mods.cancelUninstall();
});
</script>

<style scoped>
/* Tailwind's line-clamp plugin isn't enabled in this project — the same rule
   is hand-rolled in GameCommunityTab.vue. */
.line-clamp-2 {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
</style>
