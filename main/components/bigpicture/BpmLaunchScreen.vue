<template>
  <Transition name="launch">
    <div v-if="isVisible" class="launch-screen" :class="`theme-${themeId}`">
      <div class="launch-content">
        <img
          v-if="game?.coverUrl"
          :src="game.coverUrl"
          :alt="game.name"
          class="cover-image"
        />
        <div v-else class="cover-image" style="background: linear-gradient(135deg, rgba(100, 100, 100, 0.5), rgba(150, 150, 150, 0.5))"></div>
        <div class="game-name">{{ game?.name }}</div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'

interface Game {
  name: string
  coverUrl?: string
}

interface Props {
  themeId: string
  game: Game | null
}

const props = defineProps<Props>()
const emit = defineEmits<{
  ready: []
}>()

const isVisible = ref(false)

watch(
  () => props.game,
  (newGame) => {
    if (newGame) {
      isVisible.value = true
      setTimeout(() => {
        isVisible.value = false
        setTimeout(() => {
          emit('ready')
        }, 500)
      }, 1500)
    }
  }
)
</script>

<style scoped>
@keyframes launch-expand {
  from {
    transform: scale(0.1);
    opacity: 0;
  }
  to {
    transform: scale(1);
    opacity: 1;
  }
}

@keyframes launch-fade-out {
  from {
    opacity: 1;
  }
  to {
    opacity: 0;
  }
}

.launch-screen {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  z-index: 9998;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}

.launch-content {
  position: relative;
  width: 80vw;
  height: 80vh;
  display: flex;
  align-items: center;
  justify-content: center;
  animation: launch-expand 1.5s cubic-bezier(0.34, 1.56, 0.64, 1) forwards;
}

.cover-image {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: 1vw;
  box-shadow: 0 0 5vw rgba(0, 0, 0, 0.8);
}

.game-name {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  padding: 3vh 3vw;
  font-size: 3.5vw;
  font-weight: bold;
  background: linear-gradient(to top, rgba(0, 0, 0, 0.9), transparent);
  color: #fff;
  text-align: center;
  animation: launch-expand 1.5s cubic-bezier(0.34, 1.56, 0.64, 1) forwards;
}

/* Transition classes */
.launch-enter-active,
.launch-leave-active {
  transition: opacity 0.5s;
}

.launch-enter-from,
.launch-leave-to {
  opacity: 0;
}

/* Theme: steam */
.theme-steam {
  background: #000;
}

.theme-steam .launch-content {
  background: radial-gradient(ellipse at 50% 50%, #1a3a52, #000);
}

.theme-steam .game-name {
  border-top: 3px solid #66c0f4;
  background: linear-gradient(to top, rgba(0, 0, 0, 0.95), rgba(102, 192, 244, 0.1));
  color: #fff;
  text-shadow: 0 0 1vw #66c0f4;
}

/* Theme: switch */
.theme-switch {
  background: #fff;
}

.theme-switch .launch-content {
  background: linear-gradient(135deg, #e60012, #ffd700);
}

.theme-switch .game-name {
  border-top: 3px solid #e60012;
  background: linear-gradient(to top, rgba(255, 255, 255, 0.95), rgba(230, 0, 18, 0.1));
  color: #000;
}

/* Theme: xbox */
.theme-xbox {
  background: #000;
}

.theme-xbox .launch-content {
  background: linear-gradient(135deg, #107c10, #0a5605);
}

.theme-xbox .game-name {
  background: linear-gradient(to top, rgba(0, 0, 0, 0.95), rgba(16, 124, 16, 0.2));
  border-top: 3px solid #107c10;
  color: #107c10;
}

/* Theme: ps2 */
.theme-ps2 {
  background: #080818;
}

.theme-ps2 .launch-content {
  background: radial-gradient(ellipse at 50% 50%, rgba(32, 64, 192, 0.2), #080818);
}

.theme-ps2 .game-name {
  border-top: 3px solid #2040c0;
  background: linear-gradient(to top, rgba(8, 8, 24, 0.95), rgba(32, 64, 192, 0.15));
  color: #4060e0;
  text-shadow: 0 0 1vh #2040c0;
}

/* Theme: dreamcast */
.theme-dreamcast {
  background: #0a2a3a;
}

.theme-dreamcast .launch-content {
  background: conic-gradient(from 45deg, rgba(208, 80, 16, 0.1), rgba(240, 128, 48, 0.1));
}

.theme-dreamcast .game-name {
  border-top: 3px solid #d05010;
  background: linear-gradient(to top, rgba(10, 42, 58, 0.95), rgba(208, 80, 16, 0.2));
  color: #f08030;
}

/* Theme: wii */
.theme-wii {
  background: #fff;
}

.theme-wii .launch-content {
  background: linear-gradient(135deg, #f0f0f0, #ffffff);
}

.theme-wii .game-name {
  border-top: 3px solid #009ac7;
  background: linear-gradient(to top, rgba(255, 255, 255, 0.95), rgba(0, 154, 199, 0.1));
  color: #009ac7;
}

/* Theme: ds */
.theme-ds {
  background: #f5f5f5;
}

.theme-ds .launch-content {
  background: linear-gradient(45deg, #fff, #e0e0e0);
}

.theme-ds .game-name {
  border-top: 3px solid #d05028;
  background: linear-gradient(to top, rgba(255, 255, 255, 0.95), rgba(208, 80, 40, 0.1));
  color: #d05028;
}

/* Theme: gamecube */
.theme-gamecube {
  background: #524eaa;
}

.theme-gamecube .launch-content {
  background: linear-gradient(135deg, #524eaa, #6b5bc6);
}

.theme-gamecube .game-name {
  border-top: 3px solid #fff;
  background: linear-gradient(to top, rgba(82, 78, 170, 0.95), rgba(107, 91, 198, 0.2));
  color: #fff;
  text-shadow: 0 0 0.5vw #6b5bc6;
}

/* Theme: psp */
.theme-psp {
  background: #0a0a0e;
}

.theme-psp .launch-content {
  background: linear-gradient(135deg, #1a1a2e, #000);
}

.theme-psp .game-name {
  border-top: 3px solid #3C5078;
  background: linear-gradient(to top, rgba(0, 0, 0, 0.98), rgba(60, 80, 120, 0.1));
  color: #3C5078;
  font-family: monospace;
  font-size: 2.5vw;
}

/* Theme: gameboy */
.theme-gameboy {
  background: #c4cfa1;
}

.theme-gameboy .launch-content {
  background: linear-gradient(135deg, #c4cfa1, #d0d8b0);
}

.theme-gameboy .game-name {
  border-top: 3px solid #0F380F;
  background: linear-gradient(to top, rgba(196, 207, 161, 0.95), rgba(15, 56, 15, 0.2));
  color: #0F380F;
  font-family: monospace;
  text-shadow: 0 0 0.5vw #0F380F;
}

/* Theme: n64 */
.theme-n64 {
  background: #00142c;
}

.theme-n64 .launch-content {
  background: linear-gradient(135deg, #00142c, #001e40);
}

.theme-n64 .game-name {
  border-top: 3px solid #0064C8;
  background: linear-gradient(to top, rgba(0, 20, 44, 0.95), rgba(0, 100, 200, 0.2));
  color: #0064C8;
  text-shadow: 0 0 1vw #0064C8;
}

/* Theme: ps1 */
.theme-ps1 {
  background: #0e0e12;
}

.theme-ps1 .launch-content {
  background: linear-gradient(135deg, #0e0e12, #1a1a20);
}

.theme-ps1 .game-name {
  border-top: 3px solid #9696AA;
  background: linear-gradient(to top, rgba(14, 14, 18, 0.95), rgba(150, 150, 170, 0.1));
  color: #9696AA;
}

/* Theme: snes */
.theme-snes {
  background: #e8e8ec;
}

.theme-snes .launch-content {
  background: linear-gradient(135deg, #f0f0f4, #e8e8ec);
}

.theme-snes .game-name {
  border-top: 3px solid #6464B4;
  background: linear-gradient(135deg, rgba(240, 240, 244, 0.95), rgba(232, 232, 236, 0.9));
  color: #6464B4;
  font-weight: bold;
  text-shadow: 0.1vw 0.1vw 0 rgba(100, 100, 180, 0.3);
}
</style>
