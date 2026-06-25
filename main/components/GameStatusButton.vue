<template>
  <!-- Single primary action button.  The chevron dropdown that used
       to sit next to this button (with "Options" + "Uninstall") was
       consolidated into the top-left gear menu (OptionsMenu) so all
       per-game knobs live in one discoverable place.

       Hidden capability: **Shift+click on the Play / Setup button
       triggers an incognito launch** (no playtime, achievement, or
       presence reporting).  No visible UI hint — pure stealth.  The
       handler short-circuits to `launch-incognito` only when the
       current status is a launchable one; any other status falls
       through to the normal action.

       Do not add scale animations to this: https://stackoverflow.com/a/35683068 -->
  <button
    type="button"
    class="inline-flex uppercase font-display items-center gap-x-2 px-4 py-3 text-md font-semibold shadow-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 rounded-md disabled:opacity-50 disabled:cursor-not-allowed"
    :class="fetchStatusStyleData($props.status).style"
    :disabled="disabled"
    @click="onClick"
  >
    <component
      :is="fetchStatusStyleData($props.status).icon"
      class="-mr-0.5 size-5"
      aria-hidden="true"
    />
    {{ fetchStatusStyleData($props.status).buttonName }}
  </button>
</template>

<script setup lang="ts">
import {
  ArrowDownTrayIcon,
  PlayIcon,
  QueueListIcon,
  ServerIcon,
  StopIcon,
  WrenchIcon,
} from "@heroicons/vue/20/solid";

import type { Component } from "vue";
import {
  type EmptyGameStatusEnum,
  InstalledType,
  type GameStatus,
} from "~/types.js";
import { TrashIcon } from "@heroicons/vue/24/outline";

const props = defineProps<{
  status: GameStatus;
  // Optional, defaults to off — disables the action while a launch is in
  // flight so a slow prefix-prep step can't be re-triggered mid-launch.
  // Existing callers that don't pass it are unaffected.
  disabled?: boolean;
}>();
const emit = defineEmits<{
  (e: "install"): void;
  (e: "launch"): void;
  (e: "launch-incognito"): void;
  (e: "queue"): void;
  (e: "kill"): void;
  (e: "resume"): void;
}>();

/**
 * Click handler with the hidden Shift+click → incognito branch.
 * Only short-circuits when the current status would actually start
 * a session (Installed + Installed install_type — i.e. the Play
 * button).  Setup / Resume / Install paths fall through to their
 * normal actions even with Shift held, because there's no notion of
 * "private install" to suppress there.
 */
function onClick(evt: MouseEvent) {
  const isLaunchable =
    props.status.type === "Installed" &&
    props.status.install_type.type === InstalledType.Installed;
  if (evt.shiftKey && isLaunchable) {
    emit("launch-incognito");
    return;
  }
  fetchStatusStyleData(props.status).action();
}

interface StatusStyleData {
  style: string;
  buttonName: string;
  icon: Component;
  action: () => void;
}

function fetchStatusStyleData(status: GameStatus): StatusStyleData {
  if (status.type === "Installed") {
    if (status.install_type.type === InstalledType.Installed) {
      return {
        style:
          "bg-green-600 text-white hover:bg-green-500 focus-visible:outline-green-600 hover:bg-green-500",
        buttonName: "Play",
        icon: PlayIcon,
        action: () => emit("launch"),
      };
    }
    if (status.install_type.type === InstalledType.SetupRequired) {
      return {
        style:
          "bg-yellow-600 text-white hover:bg-yellow-500 focus-visible:outline-yellow-600 hover:bg-yellow-500",
        buttonName: "Setup",
        icon: WrenchIcon,
        action: () => emit("launch"),
      };
    }
    if (status.install_type.type === InstalledType.PartiallyInstalled) {
      return {
        style:
          "bg-blue-600 text-white hover:bg-blue-500 focus-visible:outline-blue-600 hover:bg-blue-500",
        buttonName: "Resume",
        icon: ArrowDownTrayIcon,
        action: () => emit("resume"),
      };
    }
    throw "Non-exhaustive install type: " + JSON.stringify(status.install_type);
  }
  return {
    style: styles[status.type],
    buttonName: buttonNames[status.type],
    icon: buttonIcons[status.type],
    action: buttonActions[status.type],
  };
}

const styles: { [key in EmptyGameStatusEnum]: string } = {
  Remote:
    "bg-blue-600 text-white hover:bg-blue-500 focus-visible:outline-blue-600 hover:bg-blue-500",
  Queued:
    "bg-zinc-800 text-white hover:bg-zinc-700 focus-visible:outline-zinc-700 hover:bg-zinc-700",
  Downloading:
    "bg-zinc-800 text-white hover:bg-zinc-700 focus-visible:outline-zinc-700 hover:bg-zinc-700",
  Validating:
    "bg-zinc-800 text-white hover:bg-zinc-700 focus-visible:outline-zinc-700 hover:bg-zinc-700",
  Updating:
    "bg-zinc-800 text-white hover:bg-zinc-700 focus-visible:outline-zinc-700 hover:bg-zinc-700",
  Uninstalling:
    "bg-zinc-800 text-white hover:bg-zinc-700 focus-visible:outline-zinc-700 hover:bg-zinc-700",
  Running:
    "bg-zinc-800 text-white hover:bg-zinc-700 focus-visible:outline-zinc-700 hover:bg-zinc-700",
};

const buttonNames: { [key in EmptyGameStatusEnum]: string } = {
  Remote: "Install",
  Queued: "Queued",
  Downloading: "Downloading",
  Validating: "Validating",
  Updating: "Updating",
  Uninstalling: "Uninstalling",
  Running: "Stop",
};

const buttonIcons: { [key in EmptyGameStatusEnum]: Component } = {
  Remote: ArrowDownTrayIcon,
  Queued: QueueListIcon,
  Downloading: ArrowDownTrayIcon,
  Validating: ServerIcon,
  Updating: ArrowDownTrayIcon,
  Uninstalling: TrashIcon,
  Running: StopIcon,
};

const buttonActions: { [key in EmptyGameStatusEnum]: () => void } = {
  Remote: () => emit("install"),
  Queued: () => emit("queue"),
  Downloading: () => emit("queue"),
  Validating: () => emit("queue"),
  Updating: () => emit("queue"),
  Uninstalling: () => {},
  Running: () => emit("kill"),
};
</script>
