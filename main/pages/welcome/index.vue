<template>
  <BpmWizardShell
    step-key="welcome"
    title="Welcome to Drop"
    subtitle="Let's take a couple of minutes to get you set up. You can skip any step — revisit the wizard later from Settings → Help."
  >
    <div class="max-w-2xl">
      <div
        class="rounded-2xl p-6"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: '1px solid var(--bpm-border)',
        }"
      >
        <p class="text-sm uppercase tracking-wide font-medium mb-4" :style="{ color: 'var(--bpm-muted)' }">
          Signed in as
        </p>
        <div class="flex items-center gap-4">
          <div
            class="size-16 rounded-full flex items-center justify-center text-2xl font-display font-semibold"
            :style="{
              backgroundColor: 'color-mix(in srgb, var(--bpm-accent-hex) 20%, transparent)',
              color: 'var(--bpm-accent-hex)',
            }"
          >
            {{ initial }}
          </div>
          <div>
            <p class="text-xl font-display font-semibold" :style="{ color: 'var(--bpm-text)' }">
              {{ displayName }}
            </p>
            <p class="text-sm" :style="{ color: 'var(--bpm-muted)' }">
              @{{ username }}
            </p>
          </div>
        </div>
      </div>

      <div class="mt-6 space-y-3 text-sm" :style="{ color: 'var(--bpm-muted)' }">
        <p>In the next few screens we'll cover:</p>
        <ul class="space-y-1.5 list-disc list-inside ml-2">
          <li>Your profile — display name, avatar, bio</li>
          <li>Where games install and how Proton runs Windows titles</li>
          <!-- Cloud saves (Ludusavi) — listed only when dev mode is on,
               since the saves wizard step itself is also gated. -->
          <li v-if="devMode.enabled.value">Cloud saves (Ludusavi)</li>
          <li>RetroAchievements for retro titles</li>
          <li>Controller navigation</li>
          <li>What common warnings and errors mean</li>
        </ul>
      </div>
    </div>
  </BpmWizardShell>
</template>

<script setup lang="ts">
import { useAppState } from "~/composables/app-state";
import BpmWizardShell from "~/components/bigpicture/BpmWizardShell.vue";

definePageMeta({ layout: "bpm-wizard" });

const appState = useAppState();
const devMode = useDevMode();

const displayName = computed(
  () => appState.value?.user?.displayName || appState.value?.user?.username || "User",
);
const username = computed(() => appState.value?.user?.username || "unknown");
const initial = computed(() =>
  (displayName.value || "U").slice(0, 1).toUpperCase(),
);
</script>
