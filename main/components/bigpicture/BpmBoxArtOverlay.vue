<template>
  <div class="box-art-overlay">
    <!-- Themes with real template images -->
    <img
      v-if="templateSrc"
      :src="templateSrc"
      alt=""
      class="template-img"
      :class="blendClass"
    />

    <!-- Fallback CSS overlay for themes without a template image (Steam). -->
    <template v-else>
      <div v-if="themeId === 'steam'" class="steam-bottom-gradient" />
    </template>
  </div>
</template>

<script setup lang="ts">
/**
 * The console frame that rides on top of a cover.
 *
 * Which PNG, how it composites, and where its art window is all live in
 * `boxart-templates.ts`, because the tile underneath needs the same geometry to
 * place the cover — see `boxArtWindowStyle`.
 */
import { computed } from "vue";
import {
  boxArtTemplate,
  boxArtTemplateSrc,
} from "~/composables/bigpicture/boxart-templates";

interface Props {
  themeId: string;
}

const props = defineProps<Props>();

const templateSrc = computed(() => boxArtTemplateSrc(props.themeId));

const blendClass = computed(() => {
  switch (boxArtTemplate(props.themeId)?.blend) {
    case "multiply":
      return "blend-multiply";
    case "screen":
      return "blend-screen";
    default:
      return "";
  }
});
</script>

<style scoped>
.box-art-overlay {
  position: absolute;
  inset: 0;
  pointer-events: none;
  z-index: 2;
  overflow: hidden;
  /* No border-radius — let template define the visual shape */
}

/* Real template image — stretches to fill the tile */
.template-img {
  width: 100%;
  height: 100%;
  object-fit: fill;
  display: block;
}

.template-img.blend-multiply {
  mix-blend-mode: multiply;
}

.template-img.blend-screen {
  mix-blend-mode: screen;
}

/* ====== Steam (CSS fallback) ====== */
.steam-bottom-gradient {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 30%;
  background: linear-gradient(to top, rgba(23, 26, 33, 0.7) 0%, transparent 100%);
}

</style>
