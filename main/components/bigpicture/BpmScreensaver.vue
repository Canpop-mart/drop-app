<template>
  <Transition name="screensaver">
    <div v-if="active" class="screensaver-overlay" @click="dismiss" @mousemove="dismiss" @keydown="dismiss">
      <!-- Steam -->
      <div v-if="themeId === 'steam'" class="saver-steam">
        <div class="steam-title">Half-Life 2</div>
        <div class="steam-title">Portal</div>
        <div class="steam-title">Deus Ex</div>
        <div class="steam-title">Grim Fandango</div>
      </div>

      <!-- Xbox -->
      <div v-else-if="themeId === 'xbox'" class="saver-xbox">
        <div class="xbox-logo" :style="xboxLogoStyle"></div>
      </div>

      <!-- PS2 -->
      <div v-else-if="themeId === 'ps2'" class="saver-ps2">
        <div class="ps2-column"></div>
        <div class="ps2-column"></div>
        <div class="ps2-column"></div>
        <div class="ps2-column"></div>
        <div class="ps2-column"></div>
        <div class="ps2-column"></div>
      </div>

      <!-- Dreamcast -->
      <div v-else-if="themeId === 'dreamcast'" class="saver-dreamcast">
        <div class="dreamcast-swirl"></div>
      </div>

      <!-- Switch -->
      <div v-else-if="themeId === 'switch'" class="saver-switch"></div>

      <!-- Wii -->
      <div v-else-if="themeId === 'wii'" class="saver-wii">
        <div class="bubble"></div>
        <div class="bubble"></div>
        <div class="bubble"></div>
        <div class="bubble"></div>
        <div class="bubble"></div>
        <div class="bubble"></div>
      </div>

      <!-- DS -->
      <div v-else-if="themeId === 'ds'" class="saver-ds">
        <div class="dot-pattern"></div>
        <div class="dot-pattern"></div>
        <div class="dot-pattern"></div>
      </div>

      <!-- Gamecube -->
      <div v-else-if="themeId === 'gamecube'" class="saver-gamecube">
        <div class="cube"></div>
      </div>

      <!-- N64 -->
      <div v-else-if="themeId === 'n64'" class="saver-n64"></div>

      <!-- PSP -->
      <div v-else-if="themeId === 'psp'" class="saver-psp">
        <div class="wave-line"></div>
        <div class="wave-line"></div>
        <div class="wave-line"></div>
        <div class="wave-line"></div>
      </div>

      <!-- PS1 -->
      <div v-else-if="themeId === 'ps1'" class="saver-ps1">
        <div class="polygon"></div>
      </div>

      <!-- SNES -->
      <div v-else-if="themeId === 'snes'" class="saver-snes">
        <div class="diamond"></div>
        <div class="diamond"></div>
        <div class="diamond"></div>
        <div class="diamond"></div>
        <div class="diamond"></div>
      </div>

      <!-- Gameboy -->
      <div v-else-if="themeId === 'gameboy'" class="saver-gameboy">
        <div class="pixel"></div>
        <div class="pixel"></div>
        <div class="pixel"></div>
        <div class="pixel"></div>
        <div class="pixel"></div>
        <div class="pixel"></div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, reactive } from 'vue'

interface Props {
  themeId: string
  active: boolean
}

defineProps<Props>()
const emit = defineEmits<{
  dismiss: []
}>()

const dismiss = () => {
  emit('dismiss')
}

// Xbox bouncing logo state
const xboxLogoStyle = reactive({
  transform: 'translate(5vw, 5vh)'
})

let rafId: number | null = null
let xboxVelocityX = 2
let xboxVelocityY = 1.5
let xboxX = 5
let xboxY = 5

const animateXboxLogo = () => {
  // Simple bouncing logic
  xboxX += xboxVelocityX
  xboxY += xboxVelocityY

  // Bounce off walls
  if (xboxX <= 0 || xboxX >= 92) {
    xboxVelocityX *= -1
    xboxX = Math.max(0, Math.min(92, xboxX))
  }
  if (xboxY <= 0 || xboxY >= 92) {
    xboxVelocityY *= -1
    xboxY = Math.max(0, Math.min(92, xboxY))
  }

  xboxLogoStyle.transform = `translate(${xboxX}vw, ${xboxY}vh)`
  rafId = requestAnimationFrame(animateXboxLogo)
}

onMounted(() => {
  // Start Xbox animation (bouncing logo)
  rafId = requestAnimationFrame(animateXboxLogo)
})

onUnmounted(() => {
  if (rafId !== null) {
    cancelAnimationFrame(rafId)
  }
})
</script>

<style scoped>
/* Transition styles */
.screensaver-enter-active,
.screensaver-leave-active {
  transition: opacity 0.5s;
}

.screensaver-enter-from,
.screensaver-leave-to {
  opacity: 0;
}

/* Overlay */
.screensaver-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  z-index: 9999;
  cursor: none;
}

/* ===== STEAM SCREENSAVER ===== */
@keyframes float-drift {
  0% {
    transform: translate(0, 0);
    opacity: 0.3;
  }
  50% {
    opacity: 0.8;
  }
  100% {
    transform: translate(20vw, -30vh);
    opacity: 0;
  }
}

.saver-steam {
  width: 100%;
  height: 100%;
  background: linear-gradient(135deg, #000 0%, #0a0a0a 100%);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}

.steam-title {
  font-size: 5vw;
  color: #66c0f4;
  position: absolute;
  font-weight: bold;
  white-space: nowrap;
  animation: float-drift 12s ease-in-out infinite;
}

.steam-title:nth-child(1) {
  top: 10%;
  left: 5%;
  animation-delay: 0s;
}

.steam-title:nth-child(2) {
  top: 30%;
  left: 10%;
  animation-delay: 3s;
}

.steam-title:nth-child(3) {
  top: 50%;
  left: 15%;
  animation-delay: 6s;
}

.steam-title:nth-child(4) {
  top: 70%;
  left: 10%;
  animation-delay: 9s;
}

/* ===== XBOX SCREENSAVER ===== */
.saver-xbox {
  width: 100%;
  height: 100%;
  background: #000;
  position: relative;
  overflow: hidden;
}

.xbox-logo {
  position: absolute;
  width: 8vw;
  height: 8vw;
  background: #107c10;
}

/* ===== PS2 SCREENSAVER ===== */
@keyframes column-float {
  0% {
    transform: translateY(0);
    opacity: 0.3;
  }
  50% {
    opacity: 0.8;
  }
  100% {
    transform: translateY(-100vh);
    opacity: 0;
  }
}

.saver-ps2 {
  width: 100%;
  height: 100%;
  background: #080818;
  position: relative;
  overflow: hidden;
}

.ps2-column {
  position: absolute;
  width: 5vw;
  height: 60vh;
  background: linear-gradient(to right, transparent, #2040c0, transparent);
  animation: column-float 8s ease-in infinite;
}

.ps2-column:nth-child(1) {
  left: 10%;
  animation-delay: 0s;
}

.ps2-column:nth-child(2) {
  left: 25%;
  animation-delay: 2s;
}

.ps2-column:nth-child(3) {
  left: 40%;
  animation-delay: 4s;
}

.ps2-column:nth-child(4) {
  left: 55%;
  animation-delay: 6s;
}

.ps2-column:nth-child(5) {
  left: 70%;
  animation-delay: 8s;
}

.ps2-column:nth-child(6) {
  left: 85%;
  animation-delay: 10s;
}

/* ===== DREAMCAST SCREENSAVER ===== */
@keyframes swirl-rotate {
  0% {
    transform: rotate(0deg) scale(0.8);
    opacity: 0.5;
  }
  50% {
    opacity: 0.9;
  }
  100% {
    transform: rotate(360deg) scale(1.2);
    opacity: 0.3;
  }
}

@keyframes pulse {
  0% {
    opacity: 0.5;
  }
  50% {
    opacity: 1;
  }
  100% {
    opacity: 0.5;
  }
}

.saver-dreamcast {
  width: 100%;
  height: 100%;
  background: #0a2a3a;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}

.dreamcast-swirl {
  width: 30vw;
  height: 30vw;
  background: conic-gradient(from 0deg, #d05010, #f08030, #d05010);
  border-radius: 50%;
  animation: swirl-rotate 8s linear infinite, pulse 3s ease-in-out infinite;
}

/* ===== SWITCH SCREENSAVER ===== */
@keyframes color-cycle {
  0% {
    background: linear-gradient(135deg, #e60012, #4169e1);
  }
  25% {
    background: linear-gradient(135deg, #4169e1, #ffff00);
  }
  50% {
    background: linear-gradient(135deg, #ffff00, #00ff00);
  }
  75% {
    background: linear-gradient(135deg, #00ff00, #e60012);
  }
  100% {
    background: linear-gradient(135deg, #e60012, #4169e1);
  }
}

.saver-switch {
  width: 100%;
  height: 100%;
  animation: color-cycle 8s ease-in-out infinite;
}

/* ===== WII SCREENSAVER ===== */
@keyframes bubble-float {
  0% {
    transform: translate(0, 100vh) scale(1);
    opacity: 0;
  }
  10% {
    opacity: 1;
  }
  90% {
    opacity: 1;
  }
  100% {
    transform: translate(15vw, -100vh) scale(0.5);
    opacity: 0;
  }
}

.saver-wii {
  width: 100%;
  height: 100%;
  background: #fff;
  position: relative;
  overflow: hidden;
}

.bubble {
  position: absolute;
  background: radial-gradient(circle at 35% 35%, rgba(255, 255, 255, 0.8), rgba(100, 150, 255, 0.3));
  border-radius: 50%;
  animation: bubble-float 6s ease-in infinite;
}

.bubble:nth-child(1) {
  width: 5vw;
  height: 5vw;
  left: 10%;
  animation-delay: 0s;
}

.bubble:nth-child(2) {
  width: 8vw;
  height: 8vw;
  left: 25%;
  animation-delay: 1s;
}

.bubble:nth-child(3) {
  width: 6vw;
  height: 6vw;
  left: 40%;
  animation-delay: 2s;
}

.bubble:nth-child(4) {
  width: 7vw;
  height: 7vw;
  left: 55%;
  animation-delay: 1.5s;
}

.bubble:nth-child(5) {
  width: 5vw;
  height: 5vw;
  left: 70%;
  animation-delay: 2.5s;
}

.bubble:nth-child(6) {
  width: 6vw;
  height: 6vw;
  left: 85%;
  animation-delay: 0.5s;
}

/* ===== DS SCREENSAVER ===== */
@keyframes dot-scroll {
  0% {
    transform: translateX(-10vw);
  }
  100% {
    transform: translateX(110vw);
  }
}

.saver-ds {
  width: 100%;
  height: 100%;
  background: #000;
  position: relative;
  overflow: hidden;
}

.dot-pattern {
  position: absolute;
  width: 100%;
  height: 100%;
  background-image: radial-gradient(circle, #fff 1vw, transparent 1vw);
  background-size: 4vw 4vh;
  background-position: 0 0;
  animation: dot-scroll 15s linear infinite;
}

.dot-pattern:nth-child(2) {
  animation-delay: 5s;
}

.dot-pattern:nth-child(3) {
  animation-delay: 10s;
}

/* ===== GAMECUBE SCREENSAVER ===== */
@keyframes cube-spin {
  0% {
    transform: rotateX(0deg) rotateY(0deg);
  }
  100% {
    transform: rotateX(360deg) rotateY(360deg);
  }
}

.saver-gamecube {
  width: 100%;
  height: 100%;
  background: #524eaa;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  perspective: 1000px;
}

.cube {
  width: 15vw;
  height: 15vw;
  border: 2px solid #fff;
  animation: cube-spin 8s linear infinite;
  transform-style: preserve-3d;
  opacity: 0.3;
}

/* ===== N64 SCREENSAVER ===== */
@keyframes starfield {
  0% {
    transform: perspective(500px) rotateZ(0deg);
  }
  100% {
    transform: perspective(500px) rotateZ(360deg);
  }
}

.saver-n64 {
  width: 100%;
  height: 100%;
  background: #00256c;
  position: relative;
  overflow: hidden;
}

.saver-n64::before {
  content: '';
  position: absolute;
  width: 100%;
  height: 100%;
  background-image: radial-gradient(circle, #fff 0.5px, transparent 0.5px);
  background-size: 50px 50px;
  background-position: 0 0;
  animation: starfield 20s linear infinite;
}

/* ===== PSP SCREENSAVER ===== */
@keyframes wave {
  0% {
    transform: translateY(0);
  }
  50% {
    transform: translateY(-5vh);
  }
  100% {
    transform: translateY(0);
  }
}

.saver-psp {
  width: 100%;
  height: 100%;
  background: #000;
  position: relative;
  overflow: hidden;
}

.wave-line {
  position: absolute;
  width: 100%;
  height: 2vh;
  background: linear-gradient(90deg, transparent, #fff, transparent);
  animation: wave 4s ease-in-out infinite;
}

.wave-line:nth-child(1) {
  top: 20%;
  animation-delay: 0s;
}

.wave-line:nth-child(2) {
  top: 40%;
  animation-delay: 0.5s;
}

.wave-line:nth-child(3) {
  top: 60%;
  animation-delay: 1s;
}

.wave-line:nth-child(4) {
  top: 80%;
  animation-delay: 1.5s;
}

/* ===== PS1 SCREENSAVER ===== */
@keyframes poly-spin {
  0% {
    transform: rotate(0deg) scale(0.8);
  }
  50% {
    transform: rotate(180deg) scale(1);
  }
  100% {
    transform: rotate(360deg) scale(0.8);
  }
}

.saver-ps1 {
  width: 100%;
  height: 100%;
  background: #111;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}

.polygon {
  width: 20vw;
  height: 20vw;
  border: 2px solid #aaa;
  clip-path: polygon(50% 0%, 100% 25%, 100% 75%, 50% 100%, 0% 75%, 0% 25%);
  animation: poly-spin 6s linear infinite;
}

/* ===== SNES SCREENSAVER ===== */
@keyframes rainbow-cycle {
  0% {
    background: linear-gradient(90deg, #ff4444, #4444ff);
  }
  25% {
    background: linear-gradient(90deg, #4444ff, #44ff44);
  }
  50% {
    background: linear-gradient(90deg, #44ff44, #ffff44);
  }
  75% {
    background: linear-gradient(90deg, #ffff44, #ff4444);
  }
  100% {
    background: linear-gradient(90deg, #ff4444, #4444ff);
  }
}

@keyframes diamond-scroll {
  0% {
    transform: translateY(-100%);
  }
  100% {
    transform: translateY(100vh);
  }
}

.saver-snes {
  width: 100%;
  height: 100%;
  animation: rainbow-cycle 8s ease-in-out infinite;
  position: relative;
  overflow: hidden;
}

.diamond {
  position: absolute;
  width: 5vw;
  height: 5vw;
  background: #fff;
  transform: rotate(45deg);
  animation: diamond-scroll 6s linear infinite;
  opacity: 0.3;
}

.diamond:nth-child(1) {
  left: 10%;
  animation-delay: 0s;
}

.diamond:nth-child(2) {
  left: 30%;
  animation-delay: 2s;
}

.diamond:nth-child(3) {
  left: 50%;
  animation-delay: 4s;
}

.diamond:nth-child(4) {
  left: 70%;
  animation-delay: 1s;
}

.diamond:nth-child(5) {
  left: 90%;
  animation-delay: 3s;
}

/* ===== GAMEBOY SCREENSAVER ===== */
@keyframes pixel-fall {
  0% {
    transform: translateY(-100vh);
    opacity: 1;
  }
  100% {
    transform: translateY(100vh);
    opacity: 0;
  }
}

.saver-gameboy {
  width: 100%;
  height: 100%;
  background: #0f380f;
  position: relative;
  overflow: hidden;
}

.pixel {
  position: absolute;
  width: 2vw;
  height: 2vw;
  background: #9bbc0f;
  animation: pixel-fall 6s linear infinite;
}

.pixel:nth-child(1) {
  left: 10%;
  animation-delay: 0s;
}

.pixel:nth-child(2) {
  left: 25%;
  animation-delay: 1s;
}

.pixel:nth-child(3) {
  left: 40%;
  animation-delay: 2s;
}

.pixel:nth-child(4) {
  left: 55%;
  animation-delay: 0.5s;
}

.pixel:nth-child(5) {
  left: 70%;
  animation-delay: 1.5s;
}

.pixel:nth-child(6) {
  left: 85%;
  animation-delay: 2.5s;
}
</style>
