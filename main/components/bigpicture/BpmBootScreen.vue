<template>
  <Transition name="boot-fade">
    <div v-if="isVisible" class="boot-screen">
      <div class="boot-container">
        <!-- Steam -->
        <div v-if="themeId === 'steam'" class="steam-boot">
          <div class="steam-particles"></div>
          <div class="steam-particles"></div>
          <div class="steam-particles"></div>
          <div class="steam-logo">STEAM</div>
        </div>

        <!-- Switch -->
        <div v-else-if="themeId === 'switch'" class="switch-boot">
          <div class="switch-logo">Nintendo Switch</div>
        </div>

        <!-- Xbox -->
        <div v-else-if="themeId === 'xbox'" class="xbox-boot">
          <div class="xbox-sphere"></div>
          <div class="xbox-burst"></div>
        </div>

        <!-- PS2 -->
        <div v-else-if="themeId === 'ps2'" class="ps2-boot">
          <div class="ps2-column"></div>
          <div class="ps2-column"></div>
          <div class="ps2-column"></div>
          <div class="ps2-text">PS2</div>
        </div>

        <!-- Dreamcast -->
        <div v-else-if="themeId === 'dreamcast'" class="dreamcast-boot">
          <div class="dreamcast-swirl"></div>
        </div>

        <!-- Wii -->
        <div v-else-if="themeId === 'wii'" class="wii-boot">
          <div class="wii-grid">
            <div class="wii-cell"></div>
            <div class="wii-cell"></div>
            <div class="wii-cell"></div>
            <div class="wii-cell"></div>
            <div class="wii-cell"></div>
            <div class="wii-cell"></div>
            <div class="wii-cell"></div>
            <div class="wii-cell"></div>
            <div class="wii-cell"></div>
            <div class="wii-cell"></div>
            <div class="wii-cell"></div>
            <div class="wii-cell"></div>
            <div class="wii-cell"></div>
            <div class="wii-cell"></div>
            <div class="wii-cell"></div>
            <div class="wii-cell"></div>
          </div>
        </div>

        <!-- DS -->
        <div v-else-if="themeId === 'ds'" class="ds-boot">
          <div class="ds-top"></div>
          <div class="ds-line"></div>
          <div class="ds-bottom"></div>
        </div>

        <!-- Gamecube -->
        <div v-else-if="themeId === 'gamecube'" class="gamecube-boot">
          <div class="gamecube-cube"></div>
        </div>

        <!-- PSP -->
        <div v-else-if="themeId === 'psp'" class="psp-boot">
          <div class="psp-sweep"></div>
          <div class="psp-text">PSP</div>
        </div>

        <!-- Game Boy -->
        <div v-else-if="themeId === 'gameboy'" class="gameboy-boot">
          <div class="gameboy-grid">
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
            <div class="gameboy-pixel"></div>
          </div>
          <div class="gameboy-text">GAME BOY</div>
        </div>

        <!-- N64 -->
        <div v-else-if="themeId === 'n64'" class="n64-boot">
          <div class="n64-logo">
            <span class="n64-r">N</span>
            <span class="n64-g">6</span>
            <span class="n64-b">4</span>
          </div>
        </div>

        <!-- PS1 -->
        <div v-else-if="themeId === 'ps1'" class="ps1-boot">
          <div class="ps1-mesh"></div>
          <div class="ps1-text">PlayStation</div>
        </div>

        <!-- SNES -->
        <div v-else-if="themeId === 'snes'" class="snes-boot">
          <div class="snes-buttons">
            <div class="snes-button red"></div>
            <div class="snes-button blue"></div>
            <div class="snes-button green"></div>
            <div class="snes-button yellow"></div>
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'

interface Props {
  themeId: string
}

defineProps<Props>()
const emit = defineEmits<{
  complete: []
}>()

const isVisible = ref(true)

onMounted(() => {
  setTimeout(() => {
    isVisible.value = false
    setTimeout(() => {
      emit('complete')
    }, 500)
  }, 2800)
})
</script>

<style scoped>
/* === Boot Fade Transition === */
.boot-fade-enter-active,
.boot-fade-leave-active {
  transition: opacity 0.5s ease;
}
.boot-fade-enter-from,
.boot-fade-leave-to {
  opacity: 0;
}

/* === Root Boot Screen === */
.boot-screen {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  z-index: 9998;
  display: flex;
  align-items: center;
  justify-content: center;
}

.boot-container {
  width: 100%;
  height: 100%;
}

/* === Steam === */
@keyframes steam-particles {
  0% {
    opacity: 0;
    transform: scale(0.5);
  }
  50% {
    opacity: 1;
  }
  100% {
    opacity: 0;
    transform: scale(1.5);
  }
}

@keyframes steam-logo {
  0% {
    opacity: 0;
  }
  50% {
    opacity: 1;
  }
  100% {
    opacity: 0;
  }
}

.steam-boot {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #000;
  position: relative;
  overflow: hidden;
}

.steam-particles {
  position: absolute;
  width: 40vw;
  height: 40vh;
  background: radial-gradient(circle at 50% 50%, #66c0f4 0%, transparent 70%);
  animation: steam-particles 2.5s ease-out forwards;
}

.steam-particles:nth-child(1) {
  top: 20%;
  left: 30%;
  animation-delay: 0s;
}

.steam-particles:nth-child(2) {
  top: 15%;
  left: 50%;
  animation-delay: 0.3s;
}

.steam-particles:nth-child(3) {
  top: 25%;
  left: 70%;
  animation-delay: 0.6s;
}

.steam-logo {
  position: absolute;
  font-size: 8vw;
  font-weight: bold;
  color: #66c0f4;
  letter-spacing: 0.2em;
  animation: steam-logo 2.5s ease-out forwards;
  animation-delay: 1s;
  z-index: 10;
}

/* === Switch === */
@keyframes switch-flash {
  0% {
    background: #000;
  }
  20% {
    background: #fff;
  }
  40% {
    background: #000;
  }
}

@keyframes switch-logo {
  0% {
    opacity: 0;
    transform: scale(0.8);
  }
  30% {
    opacity: 1;
  }
  100% {
    opacity: 0;
  }
}

.switch-boot {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  animation: switch-flash 2.5s ease-out forwards;
  position: relative;
  overflow: hidden;
}

.switch-logo {
  font-size: 10vw;
  font-weight: bold;
  color: #e60012;
  animation: switch-logo 2.5s ease-out forwards;
  animation-delay: 0.5s;
}

/* === Xbox === */
@keyframes xbox-sphere {
  0% {
    opacity: 0;
    transform: scale(0);
  }
  50% {
    opacity: 1;
    transform: scale(1);
  }
  80% {
    opacity: 1;
  }
  100% {
    opacity: 0;
    transform: scale(1.5);
  }
}

@keyframes xbox-burst {
  0% {
    opacity: 0;
  }
  50% {
    opacity: 1;
  }
  100% {
    opacity: 0;
  }
}

.xbox-boot {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #000;
  position: relative;
  overflow: hidden;
}

.xbox-sphere {
  width: 25vw;
  height: 25vw;
  background: radial-gradient(circle at 40% 40%, #107c10, #0a5605);
  border-radius: 50%;
  animation: xbox-sphere 2.5s ease-out forwards;
}

.xbox-burst {
  position: absolute;
  width: 50vw;
  height: 50vw;
  border: 3px solid #107c10;
  border-radius: 50%;
  animation: xbox-burst 2.5s ease-out forwards;
  animation-delay: 1s;
}

/* === PS2 === */
@keyframes ps2-columns {
  0% {
    opacity: 0;
    transform: translateY(20vh);
  }
  50% {
    opacity: 1;
    transform: translateY(0);
  }
  100% {
    opacity: 0;
    transform: translateY(-20vh);
  }
}

@keyframes ps2-text {
  0% {
    opacity: 0;
  }
  50% {
    opacity: 1;
  }
  100% {
    opacity: 0;
  }
}

.ps2-boot {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #080818;
  position: relative;
  overflow: hidden;
}

.ps2-column {
  position: absolute;
  width: 8vw;
  height: 60vh;
  background: linear-gradient(to right, transparent, #2040c0, transparent);
  animation: ps2-columns 2.5s ease-in-out forwards;
}

.ps2-column:nth-child(1) {
  left: 20%;
  animation-delay: 0s;
}

.ps2-column:nth-child(2) {
  left: 40%;
  animation-delay: 0.2s;
}

.ps2-column:nth-child(3) {
  left: 60%;
  animation-delay: 0.4s;
}

.ps2-text {
  font-size: 10vw;
  font-weight: bold;
  color: #2040c0;
  text-shadow: 0 0 2vw #2040c0;
  animation: ps2-text 2.5s ease-out forwards;
  animation-delay: 1.2s;
  position: absolute;
  z-index: 10;
}

/* === Dreamcast === */
@keyframes dreamcast-swirl {
  0% {
    opacity: 0;
    transform: rotate(-180deg) scale(0);
  }
  50% {
    opacity: 1;
  }
  100% {
    opacity: 0;
    transform: rotate(180deg) scale(1.2);
  }
}

.dreamcast-boot {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #0a2a3a;
  position: relative;
  overflow: hidden;
}

.dreamcast-swirl {
  width: 40vw;
  height: 40vw;
  background: conic-gradient(from 0deg, #d05010, #f08030, #d05010);
  border-radius: 50%;
  animation: dreamcast-swirl 2.5s ease-out forwards;
}

/* === Wii === */
@keyframes wii-grid {
  0% {
    opacity: 0;
  }
  40% {
    opacity: 1;
  }
  100% {
    opacity: 0;
  }
}

.wii-boot {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #fff;
  position: relative;
  overflow: hidden;
}

.wii-grid {
  width: 60vw;
  height: 60vh;
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  grid-template-rows: repeat(4, 1fr);
  gap: 1vw;
  animation: wii-grid 2.5s ease-out forwards;
}

.wii-cell {
  background: linear-gradient(135deg, #e60012, #4169e1);
  border-radius: 0.5vw;
}

/* === DS === */
@keyframes ds-line {
  0% {
    width: 0;
  }
  40% {
    width: 100%;
  }
  100% {
    width: 100%;
  }
}

@keyframes ds-fade {
  0% {
    opacity: 0;
  }
  40% {
    opacity: 1;
  }
  100% {
    opacity: 0;
  }
}

.ds-boot {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  background: #000;
  position: relative;
  overflow: hidden;
}

.ds-line {
  width: 100%;
  height: 1px;
  background: #fff;
  animation: ds-line 2.5s ease-out forwards;
  margin: 20vh 0;
}

.ds-top,
.ds-bottom {
  position: absolute;
  width: 100%;
  height: 35%;
  animation: ds-fade 2.5s ease-out forwards;
  background: linear-gradient(180deg, #1a1a2e, transparent);
}

.ds-top {
  top: 0;
  animation-delay: 0.2s;
}

.ds-bottom {
  bottom: 0;
  animation-delay: 0.2s;
  background: linear-gradient(0deg, #1a1a2e, transparent);
}

/* === Gamecube === */
@keyframes gamecube-spin {
  0% {
    opacity: 0;
    transform: rotateX(0deg) rotateY(0deg);
  }
  40% {
    opacity: 1;
  }
  100% {
    opacity: 0;
    transform: rotateX(360deg) rotateY(360deg);
  }
}

.gamecube-boot {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #524eaa;
  position: relative;
  overflow: hidden;
  perspective: 1000px;
}

.gamecube-cube {
  width: 20vw;
  height: 20vw;
  border: 3px solid #fff;
  animation: gamecube-spin 2.5s ease-out forwards;
  transform-style: preserve-3d;
}

/* === PSP === */
@keyframes psp-sweep {
  0% {
    left: -100%;
  }
  30% {
    left: 100%;
  }
  100% {
    left: 100%;
  }
}

@keyframes psp-text {
  0% {
    opacity: 0;
  }
  50% {
    opacity: 1;
  }
  100% {
    opacity: 0;
  }
}

.psp-boot {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #000;
  position: relative;
  overflow: hidden;
}

.psp-sweep {
  position: absolute;
  width: 5vw;
  height: 100%;
  background: linear-gradient(
    90deg,
    transparent,
    #fff,
    transparent
  );
  animation: psp-sweep 2.5s ease-out forwards;
}

.psp-text {
  font-size: 8vw;
  font-weight: bold;
  color: #fff;
  animation: psp-text 2.5s ease-out forwards;
  animation-delay: 0.8s;
  position: absolute;
  z-index: 10;
}

/* === Game Boy === */
@keyframes gameboy-grid {
  0% {
    opacity: 0;
  }
  40% {
    opacity: 1;
  }
  100% {
    opacity: 0;
  }
}

@keyframes gameboy-text {
  0% {
    opacity: 0;
    transform: scale(0.8);
  }
  50% {
    opacity: 1;
  }
  100% {
    opacity: 0;
  }
}

.gameboy-boot {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #0f380f;
  position: relative;
  overflow: hidden;
  font-family: monospace;
}

.gameboy-grid {
  width: 50vw;
  height: 50vh;
  display: grid;
  grid-template-columns: repeat(8, 1fr);
  grid-template-rows: repeat(8, 1fr);
  gap: 0.3vw;
  animation: gameboy-grid 2.5s ease-out forwards;
}

.gameboy-pixel {
  background: #9bbc0f;
  border-radius: 0.2vw;
}

.gameboy-text {
  font-size: 6vw;
  color: #9bbc0f;
  animation: gameboy-text 2.5s ease-out forwards;
  animation-delay: 1s;
  position: absolute;
  z-index: 10;
  text-shadow: 0 0 0.5vw #9bbc0f;
}

/* === N64 === */
@keyframes n64-spin {
  0% {
    opacity: 0;
    transform: rotateX(0deg) rotateY(0deg) scale(0.5);
  }
  50% {
    opacity: 1;
  }
  100% {
    opacity: 0;
    transform: rotateX(180deg) rotateY(180deg) scale(1);
  }
}

.n64-boot {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #00256c;
  position: relative;
  overflow: hidden;
  perspective: 1000px;
}

.n64-logo {
  font-size: 15vw;
  font-weight: bold;
  animation: n64-spin 2.5s ease-out forwards;
  position: absolute;
  z-index: 10;
}

.n64-r {
  color: #ff4444;
  position: absolute;
}

.n64-g {
  color: #44ff44;
  position: absolute;
  margin-left: -2vw;
}

.n64-b {
  color: #4444ff;
  position: absolute;
  margin-left: -4vw;
}

.n64-y {
  color: #ffff44;
  position: absolute;
  margin-left: -6vw;
}

/* === PS1 === */
@keyframes ps1-mesh {
  0% {
    opacity: 0;
    transform: scale(0.5);
  }
  50% {
    opacity: 1;
    transform: scale(1);
  }
  100% {
    opacity: 0;
  }
}

@keyframes ps1-text {
  0% {
    opacity: 0;
  }
  50% {
    opacity: 1;
  }
  100% {
    opacity: 0;
  }
}

.ps1-boot {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #333;
  position: relative;
  overflow: hidden;
}

.ps1-mesh {
  width: 40vw;
  height: 40vh;
  border: 2px solid #aaa;
  background: repeating-linear-gradient(
    0deg,
    transparent,
    transparent 5vh,
    #ddd 5vh,
    #ddd 5.5vh
  );
  animation: ps1-mesh 2.5s ease-out forwards;
  position: absolute;
}

.ps1-text {
  font-size: 6vw;
  color: #999;
  animation: ps1-text 2.5s ease-out forwards;
  animation-delay: 1s;
  position: absolute;
  z-index: 10;
}

/* === SNES === */
@keyframes snes-button {
  0% {
    opacity: 0;
    transform: scale(0);
  }
  40% {
    opacity: 1;
  }
  100% {
    opacity: 0;
  }
}

.snes-boot {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #c0c0c0;
  position: relative;
  overflow: hidden;
}

.snes-buttons {
  display: flex;
  gap: 3vw;
}

.snes-button {
  width: 8vw;
  height: 8vw;
  border-radius: 50%;
  animation: snes-button 2.5s ease-out forwards;
  position: relative;
}

.snes-button.red {
  background: #ff4444;
  animation-delay: 0s;
}

.snes-button.blue {
  background: #4444ff;
  animation-delay: 0.3s;
}

.snes-button.green {
  background: #44ff44;
  animation-delay: 0.6s;
}

.snes-button.yellow {
  background: #ffff44;
  animation-delay: 0.9s;
}
</style>
