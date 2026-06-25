<template>
  <BpmWizardShell
    step-key="warnings"
    title="Common warnings and what they mean"
    subtitle="A quick orientation so you've seen the vocabulary once. You can revisit this page any time from Settings → Help → Error reference."
  >
    <div class="max-w-3xl">
      <BpmErrorReference />
    </div>
  </BpmWizardShell>
</template>

<script setup lang="ts">
import BpmWizardShell from "~/components/bigpicture/BpmWizardShell.vue";
import BpmErrorReference from "~/components/bigpicture/BpmErrorReference.vue";

definePageMeta({ layout: "bpm-wizard" });
</script>
