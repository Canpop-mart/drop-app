<template>
  <div class="box-art-overlay">
    <!-- Themes with real template images -->
    <img
      v-if="templateSrc"
      :src="templateSrc"
      alt=""
      class="template-img"
      :class="templateBlend"
    />

    <!-- Fallback CSS overlays for themes without templates -->
    <template v-else>
      <!-- Steam: bottom gradient fade -->
      <template v-if="themeId === 'steam'">
        <div class="steam-bottom-gradient" />
      </template>

      <!-- Switch: red left spine -->
      <template v-else-if="themeId === 'switch'">
        <div class="switch-spine" />
      </template>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";

interface Props {
  themeId: string;
}

const props = defineProps<Props>();

// Themes that have real PNG front-cover templates
const templateMap: Record<string, string> = {
  gamecube: "/img/boxart/templates/gamecube.png",
  psp: "/img/boxart/templates/psp.png",
  gameboy: "/img/boxart/templates/gameboy.png",
  snes: "/img/boxart/templates/snes.png",
  ps2: "/img/boxart/templates/ps2.png",
  wii: "/img/boxart/templates/wii.png",
  ds: "/img/boxart/templates/ds.png",
  xbox: "/img/boxart/templates/xbxo360.png",
  dreamcast: "/img/boxart/templates/dreamcast.png",
};

const templateSrc = computed(() => templateMap[props.themeId] ?? null);

// White-bg templates use "multiply" (white disappears, colored elements show)
// Dark-bg templates use "screen" (black disappears, light elements show)
// DS needs no blend — it's a cart frame that should sit fully on top
const noBlend = new Set(["ds"]);
const screenBlend = new Set(["psp"]);
const templateBlend = computed(() => {
  if (noBlend.has(props.themeId)) return "";
  if (screenBlend.has(props.themeId)) return "blend-screen";
  return "blend-multiply";
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

/* ====== Switch (CSS fallback) ====== */
.switch-spine {
  position: absolute;
  top: 0;
  left: 0;
  bottom: 0;
  width: 7%;
  background: linear-gradient(180deg, #e60012 0%, #c00010 100%);
  opacity: 0.8;
}
</style>
