<template>
  <div class="h-full flex flex-col">
    <!-- Header: close + progress dots -->
    <div
      class="px-8 pt-6 pb-4 flex items-center gap-6 shrink-0"
      :style="{ borderBottom: '1px solid var(--bpm-border)' }"
    >
      <button
        :ref="(el: any) => registerChrome(el, { onSelect: exitWizard })"
        class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm font-medium transition-colors"
        :style="{ color: 'var(--bpm-muted)' }"
        @click="exitWizard"
        aria-label="Close wizard"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 20 20"
          fill="currentColor"
          class="size-4"
        >
          <path d="M6.28 5.22a.75.75 0 0 0-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 1 0 1.06 1.06L10 11.06l3.72 3.72a.75.75 0 1 0 1.06-1.06L11.06 10l3.72-3.72a.75.75 0 0 0-1.06-1.06L10 8.94 6.28 5.22Z" />
        </svg>
        Close
      </button>

      <!-- Progress dots -->
      <div class="flex items-center gap-1.5">
        <div
          v-for="i in total"
          :key="i"
          class="rounded-full transition-all"
          :class="i === step ? 'size-2.5' : 'size-1.5'"
          :style="{
            backgroundColor:
              i < step
                ? 'var(--bpm-accent-hex)'
                : i === step
                  ? 'var(--bpm-accent-hex)'
                  : 'var(--bpm-border)',
            opacity: i < step ? 0.7 : i === step ? 1 : 0.5,
          }"
        />
      </div>

      <div class="ml-auto text-xs" :style="{ color: 'var(--bpm-muted)' }">
        Step {{ step }} of {{ total }}
      </div>
    </div>

    <!-- Title + subtitle -->
    <div class="px-8 pt-8 pb-2 shrink-0">
      <h1
        class="text-3xl font-display font-semibold"
        :style="{ color: 'var(--bpm-text)' }"
      >
        {{ title }}
      </h1>
      <p
        v-if="subtitle"
        class="mt-2 text-sm max-w-2xl"
        :style="{ color: 'var(--bpm-muted)' }"
      >
        {{ subtitle }}
      </p>
    </div>

    <!-- Scrollable content slot -->
    <div data-bp-scroll class="flex-1 min-h-0 px-8 py-6 overflow-y-auto bp-scroll-hint">
      <slot />
    </div>

    <!-- Footer: Back / Skip / Next -->
    <div
      class="px-8 py-4 flex items-center gap-3 shrink-0"
      :style="{ borderTop: '1px solid var(--bpm-border)' }"
    >
      <button
        v-if="canBack"
        :ref="(el: any) => registerChrome(el, { onSelect: goBack })"
        class="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          color: 'var(--bpm-text)',
          border: '1px solid var(--bpm-border)',
        }"
        @click="goBack"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 20 20"
          fill="currentColor"
          class="size-4"
        >
          <path
            fill-rule="evenodd"
            d="M11.78 5.22a.75.75 0 0 1 0 1.06L8.06 10l3.72 3.72a.75.75 0 1 1-1.06 1.06l-4.25-4.25a.75.75 0 0 1 0-1.06l4.25-4.25a.75.75 0 0 1 1.06 0Z"
            clip-rule="evenodd"
          />
        </svg>
        Back
      </button>

      <!-- Footer hint legend -->
      <span
        class="hidden sm:inline-flex items-center gap-1.5 text-[11px] ml-2"
        :style="{ color: 'var(--bpm-muted)' }"
      >
        <kbd class="px-1.5 py-0.5 rounded border" :style="{ borderColor: 'var(--bpm-border)' }">B</kbd>
        <span v-if="canBack">back</span>
        <span v-else>close</span>
        <span class="mx-1">·</span>
        <kbd class="px-1.5 py-0.5 rounded border" :style="{ borderColor: 'var(--bpm-border)' }">A</kbd>
        <span>select</span>
        <span class="mx-1">·</span>
        <kbd class="px-1.5 py-0.5 rounded border" :style="{ borderColor: 'var(--bpm-border)' }">Start</kbd>
        <span>exit</span>
      </span>

      <div class="flex-1" />

      <button
        v-if="showSkip"
        :ref="(el: any) => registerChrome(el, { onSelect: onSkip })"
        class="px-4 py-2 rounded-lg text-sm font-medium transition-colors"
        :style="{ color: 'var(--bpm-muted)' }"
        @click="onSkip"
      >
        {{ skipLabel }}
      </button>

      <button
        v-if="showNext"
        :ref="(el: any) => registerPrimary(el, { onSelect: onNext })"
        class="flex items-center gap-2 px-6 py-2.5 rounded-lg text-sm font-semibold transition-colors disabled:opacity-40 disabled:cursor-not-allowed shadow"
        :style="{
          backgroundColor: nextDisabled
            ? 'var(--bpm-surface)'
            : 'var(--bpm-accent-hex)',
          color: nextDisabled ? 'var(--bpm-muted)' : 'white',
        }"
        :disabled="nextDisabled"
        @click="onNext"
      >
        {{ nextLabel }}
        <svg
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 20 20"
          fill="currentColor"
          class="size-4"
        >
          <path
            fill-rule="evenodd"
            d="M8.22 5.22a.75.75 0 0 1 1.06 0l4.25 4.25a.75.75 0 0 1 0 1.06l-4.25 4.25a.75.75 0 0 1-1.06-1.06L11.94 10 8.22 6.28a.75.75 0 0 1 0-1.06Z"
            clip-rule="evenodd"
          />
        </svg>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, nextTick } from "vue";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { useOnboarding, type WizardStep } from "~/composables/onboarding";

const props = withDefaults(
  defineProps<{
    stepKey: WizardStep;
    title: string;
    subtitle?: string;
    nextLabel?: string;
    skipLabel?: string;
    showNext?: boolean;
    showSkip?: boolean;
    nextDisabled?: boolean;
    /** Where "Next" goes. Defaults to the next step in WIZARD_STEPS. */
    nextTo?: string;
    /** Where Close (X / Start) exits to. */
    exitTo?: string;
    /**
     * If true, the shell will emit "next" but NOT auto-navigate. The parent
     * page is responsible for calling `navigateTo` after async work (e.g.
     * save a profile, then advance).
     */
    manualAdvance?: boolean;
  }>(),
  {
    nextLabel: "Next",
    skipLabel: "Skip",
    showNext: true,
    showSkip: true,
    nextDisabled: false,
    exitTo: "/bigpicture/settings",
    manualAdvance: false,
  },
);

const emit = defineEmits<{
  (e: "next"): void;
  (e: "skip"): void;
  (e: "back"): void;
  (e: "exit"): void;
}>();

const onboarding = useOnboarding();
const gamepad = useGamepad();
const focusNav = useFocusNavigation();

// Chrome buttons (Close, Back, Skip) — reachable via LB/RB cycle or mouse.
// Kept separate from "content" so D-pad flow inside the page is uncluttered
// and the page's primary action (Next) reliably receives initial focus.
const registerChrome = useBpFocusableGroup("wizard-chrome");

// Next lives in "content" so it is spatially reachable by D-pad-down from
// the last page card, and is the default landing focus on content-less steps.
const registerPrimary = useBpFocusableGroup("content");

const step = computed(() => onboarding.stepNumber(props.stepKey));
const total = computed(() => onboarding.total);
const canBack = computed(() => onboarding.prevRoute(props.stepKey) !== null);

function goBack() {
  const prev = onboarding.prevRoute(props.stepKey);
  if (!prev) return;
  emit("back");
  navigateTo(prev);
}

function onNext() {
  if (props.nextDisabled) return;
  onboarding.markStepSeen(props.stepKey);
  emit("next");
  if (props.manualAdvance) return;
  if (props.nextTo) {
    navigateTo(props.nextTo);
  } else {
    navigateTo(onboarding.nextRoute(props.stepKey));
  }
}

function onSkip() {
  emit("skip");
  if (props.nextTo) {
    navigateTo(props.nextTo);
  } else {
    navigateTo(onboarding.nextRoute(props.stepKey));
  }
}

function exitWizard() {
  emit("exit");
  navigateTo(props.exitTo);
}

// Gamepad: B = back (or exit if first step), Start = exit
const unsubs: (() => void)[] = [];

onMounted(() => {
  unsubs.push(
    gamepad.onButton(GamepadButton.East, () => {
      if (canBack.value) goBack();
      else exitWizard();
    }),
  );
  unsubs.push(gamepad.onButton(GamepadButton.Start, () => exitWizard()));

  // Seed focus into the content group so controller users see a ring as soon
  // as the step renders. If the page has cards, picks the first card; if
  // not (info-only step), lands on the Next button (also in "content").
  nextTick(() => {
    focusNav.focusGroup("content");
    if (!focusNav.currentFocused.value) {
      focusNav.focusGroup("wizard-chrome");
    }
  });
});

onUnmounted(() => {
  for (const u of unsubs) u();
  unsubs.length = 0;
});
</script>
