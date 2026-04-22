<template>
  <BpmWizardShell
    step-key="navigation"
    title="Controller navigation"
    subtitle="A cheat sheet you can revisit from Settings → Help anytime."
  >
    <div class="max-w-3xl space-y-6">
      <!-- Primary inputs -->
      <div>
        <p class="text-xs uppercase tracking-wide font-medium mb-3" :style="{ color: 'var(--bpm-muted)' }">
          Core buttons
        </p>
        <div class="grid grid-cols-2 gap-3">
          <div
            v-for="row in coreRows"
            :key="row.label"
            :ref="(el: any) => registerContent(el, {})"
            tabindex="0"
            class="flex items-center gap-3 rounded-xl px-4 py-3 outline-none"
            :style="{
              backgroundColor: 'var(--bpm-surface)',
              border: '1px solid var(--bpm-border)',
            }"
          >
            <div
              class="size-10 rounded-lg flex items-center justify-center text-xs font-bold shrink-0"
              :class="row.colorClass"
            >
              {{ row.button }}
            </div>
            <div class="min-w-0">
              <p class="text-sm font-semibold" :style="{ color: 'var(--bpm-text)' }">
                {{ row.label }}
              </p>
              <p class="text-xs" :style="{ color: 'var(--bpm-muted)' }">
                {{ row.description }}
              </p>
            </div>
          </div>
        </div>
      </div>

      <!-- Navigation -->
      <div>
        <p class="text-xs uppercase tracking-wide font-medium mb-3" :style="{ color: 'var(--bpm-muted)' }">
          Moving around
        </p>
        <div class="grid grid-cols-2 gap-3">
          <div
            v-for="row in navRows"
            :key="row.label"
            :ref="(el: any) => registerContent(el, {})"
            tabindex="0"
            class="flex items-center gap-3 rounded-xl px-4 py-3 outline-none"
            :style="{
              backgroundColor: 'var(--bpm-surface)',
              border: '1px solid var(--bpm-border)',
            }"
          >
            <div
              class="size-10 rounded-lg flex items-center justify-center text-[10px] font-bold shrink-0"
              :class="row.colorClass"
            >
              {{ row.button }}
            </div>
            <div class="min-w-0">
              <p class="text-sm font-semibold" :style="{ color: 'var(--bpm-text)' }">
                {{ row.label }}
              </p>
              <p class="text-xs" :style="{ color: 'var(--bpm-muted)' }">
                {{ row.description }}
              </p>
            </div>
          </div>
        </div>
      </div>

      <!-- RetroArch hotkey -->
      <div
        class="rounded-xl p-4"
        :style="{
          backgroundColor: 'color-mix(in srgb, var(--bpm-accent-hex) 8%, transparent)',
          border: '1px solid var(--bpm-accent-hex)',
        }"
      >
        <p class="text-sm font-semibold mb-2" :style="{ color: 'var(--bpm-accent-hex)' }">
          Inside a RetroArch game
        </p>
        <p class="text-xs" :style="{ color: 'var(--bpm-muted)' }">
          Hold <strong :style="{ color: 'var(--bpm-text)' }">R3</strong> + any button for state save/load, fast-forward, or
          to quit back to Drop. A full list is under Library → Game → Cheatsheet.
        </p>
      </div>

      <!-- Keyboard reminder -->
      <div class="text-xs" :style="{ color: 'var(--bpm-muted)' }">
        Text entry brings up Drop's on-screen keyboard. Press <strong :style="{ color: 'var(--bpm-text)' }">LT</strong>
        to paste from clipboard.
      </div>
    </div>
  </BpmWizardShell>
</template>

<script setup lang="ts">
import BpmWizardShell from "~/components/bigpicture/BpmWizardShell.vue";
import { useBpFocusableGroup } from "~/composables/bp-focusable";

definePageMeta({ layout: "bpm-wizard" });

const registerContent = useBpFocusableGroup("content");

const coreRows = [
  { button: "A", label: "Select / confirm", description: "Activates the focused item.", colorClass: "bg-green-600 text-white" },
  { button: "B", label: "Back / cancel", description: "Closes dialogs, goes up one level.", colorClass: "bg-rose-600 text-white" },
  { button: "Y", label: "Favorite / context", description: "Adds to favorites or opens a menu.", colorClass: "bg-yellow-500 text-black" },
  { button: "X", label: "Secondary action", description: "Sort, filter, or page-specific shortcut.", colorClass: "bg-blue-600 text-white" },
  { button: "Start", label: "Game menu / wizard exit", description: "Opens the in-game menu or closes this wizard.", colorClass: "bg-zinc-700 text-zinc-100" },
  { button: "Select", label: "Debug console", description: "Select + Start together toggles the debug overlay.", colorClass: "bg-zinc-700 text-zinc-100" },
];

const navRows = [
  { button: "D-Pad", label: "Navigate", description: "Move focus up/down/left/right.", colorClass: "bg-zinc-800 text-zinc-200 border border-zinc-600" },
  { button: "LB/RB", label: "Cycle sections", description: "Jump between page tabs or sections.", colorClass: "bg-purple-600 text-white" },
  { button: "LT/RT", label: "Fast scroll / paste", description: "Scroll pages fast; LT also pastes into the keyboard.", colorClass: "bg-amber-600 text-white" },
  { button: "L-Stick", label: "Smooth scroll", description: "Scroll in any direction.", colorClass: "bg-zinc-700 text-zinc-100" },
];
</script>
