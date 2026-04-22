<template>
  <BpmWizardShell
    step-key="retroachievements"
    :title="title"
    :subtitle="subtitle"
    manual-advance
    :next-disabled="nextDisabled"
    :next-label="nextLabel"
    :skip-label="subStep === 'link' && !raLinked ? 'Skip' : 'Skip'"
    @next="handleNext"
    @skip="handleSkip"
  >
    <!-- Sub-step 1: intro -->
    <div v-if="subStep === 'intro'" class="max-w-2xl space-y-5">
      <div
        class="rounded-xl p-5"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: '1px solid var(--bpm-border)',
        }"
      >
        <p class="text-sm" :style="{ color: 'var(--bpm-text)' }">
          <strong>RetroAchievements</strong> is a community-run service that adds Xbox-style
          achievements to classic games — NES, SNES, Genesis, PlayStation, GameBoy, and hundreds more.
          It's free and independent of Drop.
        </p>
        <p class="text-sm mt-3" :style="{ color: 'var(--bpm-muted)' }">
          When you launch a supported retro game in Drop's RetroArch integration, it talks to the
          RetroAchievements servers to award unlocks in real time.
        </p>
      </div>

      <!-- Simple flow diagram -->
      <div class="flex items-center justify-between gap-3 py-4">
        <div class="flex-1 text-center">
          <div
            class="size-12 mx-auto rounded-lg flex items-center justify-center"
            :style="{ backgroundColor: 'var(--bpm-surface)', border: '1px solid var(--bpm-border)' }"
          >
            <span class="text-lg">🎮</span>
          </div>
          <p class="text-xs mt-2" :style="{ color: 'var(--bpm-muted)' }">Retro game</p>
        </div>
        <div :style="{ color: 'var(--bpm-muted)' }">→</div>
        <div class="flex-1 text-center">
          <div
            class="size-12 mx-auto rounded-lg flex items-center justify-center"
            :style="{ backgroundColor: 'var(--bpm-surface)', border: '1px solid var(--bpm-border)' }"
          >
            <span class="text-lg">📦</span>
          </div>
          <p class="text-xs mt-2" :style="{ color: 'var(--bpm-muted)' }">RetroArch</p>
        </div>
        <div :style="{ color: 'var(--bpm-muted)' }">→</div>
        <div class="flex-1 text-center">
          <div
            class="size-12 mx-auto rounded-lg flex items-center justify-center"
            :style="{ backgroundColor: 'var(--bpm-surface)', border: '1px solid var(--bpm-border)' }"
          >
            <span class="text-lg">🌐</span>
          </div>
          <p class="text-xs mt-2" :style="{ color: 'var(--bpm-muted)' }">RA servers</p>
        </div>
        <div :style="{ color: 'var(--bpm-muted)' }">→</div>
        <div class="flex-1 text-center">
          <div
            class="size-12 mx-auto rounded-lg flex items-center justify-center"
            :style="{ backgroundColor: 'color-mix(in srgb, var(--bpm-accent-hex) 20%, transparent)' }"
          >
            <span class="text-lg">🏆</span>
          </div>
          <p class="text-xs mt-2" :style="{ color: 'var(--bpm-accent-hex)' }">Unlock</p>
        </div>
      </div>

      <div class="text-xs" :style="{ color: 'var(--bpm-muted)' }">
        You'll need a free account at
        <span class="font-mono">retroachievements.org</span>. If you already have one, you can sign
        in on the next screen. No account? Skip — you can always come back later.
      </div>
    </div>

    <!-- Sub-step 2: link -->
    <div v-else-if="subStep === 'link'" class="max-w-xl space-y-4">
      <div
        v-if="raLinked"
        class="rounded-xl p-5"
        :style="{
          backgroundColor: 'color-mix(in srgb, var(--bpm-accent-hex) 10%, transparent)',
          border: '1px solid var(--bpm-accent-hex)',
        }"
      >
        <p class="text-sm font-semibold" :style="{ color: 'var(--bpm-accent-hex)' }">
          Linked as {{ raUsername }}
        </p>
        <p class="text-xs mt-1" :style="{ color: 'var(--bpm-muted)' }">
          Achievement tracking is active. You can unlink from Settings → Achievements.
        </p>
      </div>

      <template v-else>
        <!-- Username -->
        <div
          :ref="(el: any) => registerContent(el, { onSelect: () => openKeyboard('username') })"
          class="cursor-pointer rounded-xl p-4 transition-colors"
          :style="{
            backgroundColor: 'var(--bpm-surface)',
            border: '1px solid var(--bpm-border)',
          }"
          @click="openKeyboard('username')"
        >
          <p class="text-xs uppercase tracking-wide font-medium mb-2" :style="{ color: 'var(--bpm-muted)' }">
            RetroAchievements username
          </p>
          <p v-if="raUsername" class="text-base" :style="{ color: 'var(--bpm-text)' }">
            {{ raUsername }}
          </p>
          <p v-else class="text-sm italic" :style="{ color: 'var(--bpm-muted)' }">
            Press A to enter
          </p>
        </div>

        <!-- Password -->
        <div
          :ref="(el: any) => registerContent(el, { onSelect: () => openKeyboard('password') })"
          class="cursor-pointer rounded-xl p-4 transition-colors"
          :style="{
            backgroundColor: 'var(--bpm-surface)',
            border: '1px solid var(--bpm-border)',
          }"
          @click="openKeyboard('password')"
        >
          <p class="text-xs uppercase tracking-wide font-medium mb-2" :style="{ color: 'var(--bpm-muted)' }">
            Password
          </p>
          <p v-if="raPassword" class="text-base font-mono" :style="{ color: 'var(--bpm-text)' }">
            {{ "•".repeat(Math.min(raPassword.length, 16)) }}
          </p>
          <p v-else class="text-sm italic" :style="{ color: 'var(--bpm-muted)' }">
            Press A to enter
          </p>
        </div>

        <p class="text-xs" :style="{ color: 'var(--bpm-muted)' }">
          Drop only sends these to retroachievements.org's official API. After login we store a
          session token, not the password.
        </p>

        <div
          v-if="raError"
          class="rounded-xl border border-red-500/40 bg-red-500/10 px-4 py-3 text-sm text-red-300"
        >
          {{ raError }}
        </div>

        <button
          :ref="(el: any) => registerContent(el, { onSelect: doLink })"
          :disabled="linking || !raUsername || !raPassword"
          class="flex items-center gap-2 px-5 py-2.5 rounded-lg text-sm font-semibold transition-colors disabled:opacity-50"
          :style="{
            backgroundColor: 'var(--bpm-accent-hex)',
            color: 'white',
          }"
          @click="doLink"
        >
          {{ linking ? "Linking…" : "Link account" }}
        </button>
      </template>
    </div>

    <!-- Sub-step 3: hardcore info -->
    <div v-else-if="subStep === 'tips'" class="max-w-2xl space-y-4">
      <div
        class="rounded-xl p-5"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: '1px solid var(--bpm-border)',
        }"
      >
        <p class="text-sm font-semibold mb-3" :style="{ color: 'var(--bpm-text)' }">
          Hardcore vs. Softcore
        </p>
        <div class="grid grid-cols-2 gap-4">
          <div>
            <p class="text-xs font-semibold uppercase tracking-wide" :style="{ color: 'var(--bpm-accent-hex)' }">
              Hardcore
            </p>
            <p class="text-xs mt-1" :style="{ color: 'var(--bpm-muted)' }">
              No save states, no rewind, no cheats. Achievements count for leaderboards.
            </p>
          </div>
          <div>
            <p class="text-xs font-semibold uppercase tracking-wide" :style="{ color: 'var(--bpm-muted)' }">
              Softcore
            </p>
            <p class="text-xs mt-1" :style="{ color: 'var(--bpm-muted)' }">
              Save states and rewind allowed. Achievements still unlock but don't count for
              leaderboards.
            </p>
          </div>
        </div>
      </div>

      <div
        class="rounded-xl p-4 text-sm"
        :style="{
          backgroundColor: 'color-mix(in srgb, var(--bpm-muted) 10%, transparent)',
          color: 'var(--bpm-muted)',
        }"
      >
        Drop currently runs RetroArch in <strong>softcore mode</strong> so save-states work while
        you learn a game. Advanced users can switch to hardcore by editing RetroArch's config
        directly.
      </div>
    </div>

    <BigPictureKeyboard
      :visible="keyboardOpen"
      :model-value="keyboardValue"
      :placeholder="keyboardPlaceholder"
      @update:model-value="onKeyboardInput"
      @close="keyboardOpen = false"
      @submit="onKeyboardSubmit"
    />
  </BpmWizardShell>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import BpmWizardShell from "~/components/bigpicture/BpmWizardShell.vue";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useOnboarding } from "~/composables/onboarding";

definePageMeta({ layout: "bpm-wizard" });

const registerContent = useBpFocusableGroup("content");
const focusNav = useFocusNavigation();
const onboarding = useOnboarding();

type SubStep = "intro" | "link" | "tips";
const subStep = ref<SubStep>("intro");

const raUsername = ref("");
const raPassword = ref("");
const raLinked = ref(false);
const raError = ref("");
const linking = ref(false);

const keyboardOpen = ref(false);
const keyboardValue = ref("");
const keyboardPlaceholder = ref("");
const keyboardField = ref<"username" | "password" | null>(null);

onMounted(async () => {
  try {
    const settings = await invoke<Record<string, any>>("fetch_settings");
    if (settings.raUsername) {
      raUsername.value = settings.raUsername;
      raLinked.value = !!(settings.raToken && settings.raToken.length > 0);
    }
  } catch {
    // Settings unavailable — fall back to empty.
  }
});

// Re-seed focus into the newly visible sub-step. Without this, the previous
// sub-step's focus target gets unmounted and the controller has no ring until
// the user presses a D-pad direction.
watch(subStep, () => {
  nextTick(() => focusNav.focusGroup("content"));
});

const title = computed(() => {
  if (subStep.value === "intro") return "RetroAchievements";
  if (subStep.value === "link") return "Link your RetroAchievements account";
  return "A quick note on Hardcore mode";
});

const subtitle = computed(() => {
  if (subStep.value === "intro")
    return "Achievements for retro games. Independent of Drop — skip if you don't play retro.";
  if (subStep.value === "link")
    return "Already have an account at retroachievements.org? Sign in.";
  return "The difference matters if you care about leaderboards.";
});

const nextLabel = computed(() => {
  if (subStep.value === "intro") return "I have an account";
  if (subStep.value === "link") return raLinked.value ? "Continue" : "Continue";
  return "Continue";
});

const nextDisabled = computed(() => linking.value);

function handleNext() {
  if (subStep.value === "intro") {
    subStep.value = "link";
    return;
  }
  if (subStep.value === "link") {
    subStep.value = "tips";
    return;
  }
  onboarding.markStepSeen("retroachievements");
  navigateTo(onboarding.nextRoute("retroachievements"));
}

function handleSkip() {
  // Skip whole step entirely.
  onboarding.markStepSeen("retroachievements");
  navigateTo(onboarding.nextRoute("retroachievements"));
}

function openKeyboard(field: "username" | "password") {
  keyboardField.value = field;
  keyboardValue.value = field === "username" ? raUsername.value : raPassword.value;
  keyboardPlaceholder.value =
    field === "username" ? "RetroAchievements username" : "Password";
  keyboardOpen.value = true;
}

function onKeyboardInput(val: string) {
  if (keyboardField.value === "username") raUsername.value = val.slice(0, 64);
  else if (keyboardField.value === "password") raPassword.value = val.slice(0, 128);
  keyboardValue.value = val;
}

function onKeyboardSubmit() {
  keyboardOpen.value = false;
}

async function doLink() {
  raError.value = "";
  linking.value = true;
  try {
    const user = await invoke<string>("ra_login_and_save", {
      username: raUsername.value,
      password: raPassword.value,
    });
    raUsername.value = user;
    raPassword.value = "";
    raLinked.value = true;
  } catch (e: any) {
    raError.value = typeof e === "string" ? e : String(e?.message ?? e);
  } finally {
    linking.value = false;
  }
}
</script>
