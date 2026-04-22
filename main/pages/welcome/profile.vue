<template>
  <BpmWizardShell
    step-key="profile"
    title="Your profile"
    subtitle="Pick a display name and a short bio. You can edit these anytime, and avatar / showcase live in the full profile editor."
    :next-disabled="saving"
    :next-label="saving ? 'Saving…' : 'Save & continue'"
    manual-advance
    @next="saveAndAdvance"
  >
    <div class="max-w-xl space-y-5">
      <!-- Display name -->
      <div
        :ref="(el: any) => registerContent(el, { onSelect: () => openKeyboard('displayName') })"
        class="cursor-pointer rounded-xl p-4 transition-colors"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: '1px solid var(--bpm-border)',
        }"
        @click="openKeyboard('displayName')"
      >
        <p class="text-xs uppercase tracking-wide font-medium mb-2" :style="{ color: 'var(--bpm-muted)' }">
          Display name
        </p>
        <p v-if="displayName" class="text-base" :style="{ color: 'var(--bpm-text)' }">
          {{ displayName }}
        </p>
        <p v-else class="text-sm italic" :style="{ color: 'var(--bpm-muted)' }">
          Press A to enter your name
        </p>
      </div>

      <!-- Bio -->
      <div
        :ref="(el: any) => registerContent(el, { onSelect: () => openKeyboard('bio') })"
        class="cursor-pointer rounded-xl p-4 transition-colors"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: '1px solid var(--bpm-border)',
        }"
        @click="openKeyboard('bio')"
      >
        <p class="text-xs uppercase tracking-wide font-medium mb-2" :style="{ color: 'var(--bpm-muted)' }">
          Bio <span class="normal-case" :style="{ color: 'var(--bpm-muted)' }">(optional, max 500)</span>
        </p>
        <p v-if="bio" class="text-sm whitespace-pre-wrap line-clamp-4" :style="{ color: 'var(--bpm-text)' }">
          {{ bio }}
        </p>
        <p v-else class="text-sm italic" :style="{ color: 'var(--bpm-muted)' }">
          Press A to add a bio
        </p>
      </div>

      <div
        v-if="saveError"
        class="rounded-xl border border-red-500/40 bg-red-500/10 px-4 py-3 text-sm text-red-300"
      >
        Couldn't save profile: {{ saveError }}
      </div>

      <div class="text-xs" :style="{ color: 'var(--bpm-muted)' }">
        Want an avatar, banner, or showcase? You can set those from your profile page after the wizard.
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
import { ref, onMounted } from "vue";
import BpmWizardShell from "~/components/bigpicture/BpmWizardShell.vue";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useAppState } from "~/composables/app-state";
import { useServerApi } from "~/composables/use-server-api";
import { useOnboarding } from "~/composables/onboarding";

definePageMeta({ layout: "bpm-wizard" });

const registerContent = useBpFocusableGroup("content");
const appState = useAppState();
const api = useServerApi();
const onboarding = useOnboarding();

const displayName = ref("");
const bio = ref("");
const saving = ref(false);
const saveError = ref("");

const keyboardOpen = ref(false);
const keyboardValue = ref("");
const keyboardPlaceholder = ref("");
const keyboardField = ref<"displayName" | "bio" | null>(null);

onMounted(async () => {
  const userId = appState.value?.user?.id;
  if (!userId) return;
  try {
    const profile = await api.profile.get(userId);
    displayName.value = profile.displayName ?? "";
    bio.value = profile.bio ?? "";
  } catch {
    // Non-blocking — if the profile endpoint is temporarily unavailable the
    // user can still type new values and the PATCH below will retry.
  }
});

function openKeyboard(field: "displayName" | "bio") {
  keyboardField.value = field;
  keyboardValue.value = field === "displayName" ? displayName.value : bio.value;
  keyboardPlaceholder.value =
    field === "displayName" ? "Your display name" : "Tell everyone about yourself…";
  keyboardOpen.value = true;
}

function onKeyboardInput(val: string) {
  if (keyboardField.value === "displayName") {
    displayName.value = val.slice(0, 64);
  } else if (keyboardField.value === "bio") {
    bio.value = val.slice(0, 500);
  }
  keyboardValue.value = val;
}

function onKeyboardSubmit() {
  keyboardOpen.value = false;
}

async function saveAndAdvance() {
  saveError.value = "";
  saving.value = true;
  try {
    await api.profile.update({
      displayName: displayName.value.trim(),
      bio: bio.value.trim(),
    });
    onboarding.markStepSeen("profile");
    navigateTo(onboarding.nextRoute("profile"));
  } catch (e: any) {
    saveError.value = typeof e === "string" ? e : String(e?.message ?? e);
  } finally {
    saving.value = false;
  }
}
</script>
