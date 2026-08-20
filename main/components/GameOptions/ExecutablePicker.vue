<template>
  <div class="mt-6">
    <label class="block text-sm/6 font-medium text-zinc-100">
      Executable to launch
    </label>

    <p v-if="loading" class="mt-2 text-sm text-zinc-400">
      Looking through the install folder...
    </p>

    <p v-else-if="error" class="mt-2 text-sm text-red-400">
      {{ error }}
    </p>

    <p v-else-if="scan && !scan.supported" class="mt-2 text-sm text-zinc-400">
      {{ unsupportedText }}
    </p>

    <template v-else-if="scan">
      <p class="mt-2 text-sm text-zinc-400">
        Drop found these programs in the install folder. Picking one only
        changes what this machine launches. It is not saved to your account.
      </p>

      <!-- The launch string template runs last and wins outright. Before this
           picker existed, typing a literal path into that field was the only
           way to change the executable, so anyone still doing that would pick
           a file here and see nothing happen. -->
      <p
        v-if="templateOverridesPick"
        class="mt-2 rounded-md bg-amber-500/10 px-3 py-2 text-sm text-amber-300 ring-1 ring-inset ring-amber-500/30"
      >
        The launch string template above has no
        <span class="font-mono">{}</span> in it, so Drop runs that text as
        written and this pick is ignored. Put
        <span class="font-mono">{}</span> back in the template to use it.
      </p>

      <div class="mt-3 max-h-56 overflow-y-auto rounded-md ring-1 ring-zinc-800">
        <button
          type="button"
          class="w-full text-left px-3 py-2 transition-colors border-b border-zinc-800"
          :class="
            model
              ? 'bg-zinc-800/40 hover:bg-zinc-800'
              : 'bg-blue-600/20 ring-1 ring-inset ring-blue-600'
          "
          @click="model = null"
        >
          <span class="block text-sm font-medium text-zinc-100">Automatic</span>
          <span class="block text-xs text-zinc-400 truncate">
            {{
              scan.automatic
                ? `Uses ${scan.automatic}`
                : "Uses whatever the server set up for this game"
            }}
          </span>
        </button>

        <button
          v-for="candidate in scan.candidates"
          :key="candidate.relativePath"
          type="button"
          class="w-full text-left px-3 py-2 transition-colors border-b border-zinc-800 last:border-b-0"
          :class="
            model === candidate.relativePath
              ? 'bg-blue-600/20 ring-1 ring-inset ring-blue-600'
              : 'bg-zinc-900 hover:bg-zinc-800'
          "
          @click="model = candidate.relativePath"
        >
          <span class="flex items-baseline justify-between gap-2">
            <span
              class="text-sm font-medium truncate"
              :class="candidate.likelyNoise ? 'text-zinc-400' : 'text-zinc-100'"
            >
              {{ candidate.fileName }}
            </span>
            <span class="shrink-0 text-xs text-zinc-500">
              <span v-if="candidate.isCurrent" class="mr-2 text-blue-400"
                >in use</span
              >{{ formatExecutableSize(candidate.size) }}
            </span>
          </span>
          <span class="block text-xs text-zinc-500 truncate">
            {{ candidate.relativePath }}
            <span v-if="candidate.likelyNoise" class="text-amber-500/80">
              &middot; setup or support program
            </span>
          </span>
        </button>

        <p
          v-if="scan.candidates.length === 0"
          class="px-3 py-2 text-sm text-zinc-400"
        >
          Nothing that looks like a program turned up in the install folder.
        </p>
      </div>

      <p v-if="model" class="mt-2 text-sm text-zinc-400">
        Saved as
        <span class="font-mono text-zinc-200">{{ model }}</span
        >. If that file is gone the next time you play, Drop logs a warning and
        falls back to Automatic.
      </p>
    </template>
  </div>
</template>

<script setup lang="ts">
import {
  EXECUTABLE_UNSUPPORTED_TEXT,
  formatExecutableSize,
  launchTemplateUsesExecutable,
  scanGameExecutables,
  type ExecutableScanResult,
} from "~/composables/game-detail/executable-picker";

const model = defineModel<string | null>({ required: true });

const props = defineProps<{
  gameId: string;
  /** The template the launcher applies after this pick is resolved. */
  launchTemplate: string;
}>();

const scan = ref<ExecutableScanResult | null>(null);
const loading = ref(true);
const error = ref<string | null>(null);

const templateOverridesPick = computed(
  () => !launchTemplateUsesExecutable(props.launchTemplate),
);

const unsupportedText = computed(() => {
  const reason = scan.value?.unsupportedReason;
  return reason ? EXECUTABLE_UNSUPPORTED_TEXT[reason] : "";
});

onMounted(async () => {
  try {
    scan.value = await scanGameExecutables(props.gameId);
  } catch (e) {
    error.value = `Could not read the install folder: ${
      e instanceof Error ? e.message : String(e)
    }`;
  } finally {
    loading.value = false;
  }
});
</script>
