<template>
  <div class="border-b border-zinc-700 py-5">
    <h3 class="text-base font-semibold font-display leading-6 text-zinc-100">
      Developer
    </h3>
    <p class="mt-1 text-sm text-zinc-400">
      Reveals features that are still in active development. Off by default —
      flip it on to expose streaming, cloud save sync, and per-game compatibility
      tests. The same toggle exists in Big Picture Mode → Settings → Developer.
    </p>
  </div>

  <div class="mt-5 space-y-8">
    <div class="flex flex-row items-center justify-between">
      <div>
        <h3 class="text-sm font-medium leading-6 text-zinc-100">
          Enable dev mode
        </h3>
        <p class="mt-1 text-sm leading-6 text-zinc-400">
          Show in-development features and tagged debug logs. Setting persists
          across restarts.
        </p>
      </div>
      <Switch
        v-model="enabled"
        :class="[
          enabled ? 'bg-blue-600' : 'bg-zinc-700',
          'relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out',
        ]"
      >
        <span
          :class="[
            enabled ? 'translate-x-5' : 'translate-x-0',
            'pointer-events-none relative inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out',
          ]"
        />
      </Switch>
    </div>

    <div
      v-if="enabled"
      class="rounded-md bg-blue-500/10 p-4 outline outline-blue-500/20"
    >
      <h4 class="text-sm font-medium text-blue-200">Currently revealed</h4>
      <ul class="mt-2 ml-5 list-disc text-sm text-blue-200/85 space-y-1">
        <li>Streaming (Sunshine/Moonlight) — settings tab, per-game button</li>
        <li>Cloud save sync (Ludusavi) — Saves tab in Big Picture Mode</li>
        <li>Per-game compatibility tests and the batch tester</li>
      </ul>
      <p class="mt-3 text-xs text-blue-200/70">
        Tagged debug logs are also enabled. Configure individual log categories
        in Big Picture Mode → Settings → Developer.
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Switch } from "@headlessui/vue";

const dev = useDevMode();

const enabled = computed({
  get: () => dev.enabled.value,
  set: (v: boolean) => dev.setEnabled(v),
});
</script>
