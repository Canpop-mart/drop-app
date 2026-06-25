<template>
  <BpmWizardShell
    step-key="done"
    title="You're all set"
    subtitle="Drop is ready to use. Head to the Store to pick up your first game."
    manual-advance
    next-label="Browse the Store"
    skip-label="Back to Big Picture"
    @next="finishToStore"
    @skip="finishToHome"
  >
    <div class="max-w-2xl space-y-5">
      <!-- Confetti-ish success card -->
      <div
        class="rounded-2xl p-8 text-center"
        :style="{
          backgroundColor: 'color-mix(in srgb, var(--bpm-accent-hex) 12%, transparent)',
          border: '1px solid var(--bpm-accent-hex)',
        }"
      >
        <div
          class="size-16 mx-auto rounded-full flex items-center justify-center"
          :style="{ backgroundColor: 'var(--bpm-accent-hex)' }"
        >
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="white" class="size-8">
            <path fill-rule="evenodd" d="M16.704 4.153a.75.75 0 0 1 .143 1.052l-8 10.5a.75.75 0 0 1-1.127.075l-4.5-4.5a.75.75 0 0 1 1.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 0 1 1.05-.143Z" clip-rule="evenodd" />
          </svg>
        </div>
        <p class="text-xl font-display font-semibold mt-4" :style="{ color: 'var(--bpm-text)' }">
          Setup complete
        </p>
        <p class="text-sm mt-2" :style="{ color: 'var(--bpm-muted)' }">
          You can revisit any of these screens later from Settings → Help.
        </p>
      </div>

      <!-- Recap -->
      <div
        class="rounded-xl p-5"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: '1px solid var(--bpm-border)',
        }"
      >
        <p class="text-xs uppercase tracking-wide font-medium mb-3" :style="{ color: 'var(--bpm-muted)' }">
          Quick tips to take with you
        </p>
        <ul class="space-y-2 text-sm" :style="{ color: 'var(--bpm-text)' }">
          <li class="flex items-start gap-2">
            <span :style="{ color: 'var(--bpm-accent-hex)' }">→</span>
            <span>Press <strong>Start</strong> anywhere to open the game menu or close an overlay.</span>
          </li>
          <li class="flex items-start gap-2">
            <span :style="{ color: 'var(--bpm-accent-hex)' }">→</span>
            <span>Hold <strong>Select + Start</strong> to toggle the debug overlay when something looks off.</span>
          </li>
          <li class="flex items-start gap-2">
            <span :style="{ color: 'var(--bpm-accent-hex)' }">→</span>
            <span>Bug report? Settings → Submit Bug Report attaches logs automatically.</span>
          </li>
        </ul>
      </div>
    </div>
  </BpmWizardShell>
</template>

<script setup lang="ts">
import BpmWizardShell from "~/components/bigpicture/BpmWizardShell.vue";
import { useOnboarding } from "~/composables/onboarding";

definePageMeta({ layout: "bpm-wizard" });

const onboarding = useOnboarding();

function finishToStore() {
  onboarding.markCompleted();
  onboarding.markStepSeen("done");
  navigateTo("/bigpicture/store");
}

function finishToHome() {
  onboarding.markCompleted();
  onboarding.markStepSeen("done");
  navigateTo("/bigpicture");
}
</script>
