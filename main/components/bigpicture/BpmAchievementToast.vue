<template>
  <Transition :enter-active-class="enterActiveClass" :leave-active-class="leaveActiveClass">
    <div v-if="show" class="toast" :style="toastStyle">
      <div class="achievement-label">{{ achievementLabel }}</div>
      <div class="achievement-name">{{ achievement?.title }}</div>
      <div class="achievement-game">{{ achievement?.game }}</div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue'

interface Achievement {
  title: string
  game: string
  icon?: string
}

interface Props {
  themeId: string
  achievement: Achievement | null
}

const props = defineProps<Props>()
const emit = defineEmits<{
  dismissed: []
}>()

const show = ref(false)
let timeout: ReturnType<typeof setTimeout>

const themeConfig = {
  steam: {
    bgGradient: 'linear-gradient(135deg, #1b4620 0%, #0d6633 100%)',
    borderColor: '#66c0f4',
    textColor: '#fff',
    boxShadow: '0 0 2vw rgba(102, 192, 244, 0.5)',
    position: { bottom: '5vh', right: '2vw' },
    maxWidth: '25vw',
    animationClass: 'steam-animation',
    label: 'Achievement Unlocked'
  },
  xbox: {
    bgGradient: 'linear-gradient(135deg, #d4af37 0%, #aa8c2e 100%)',
    borderColor: '#107c10',
    textColor: '#000',
    boxShadow: '0 0 3vw rgba(212, 175, 55, 0.7)',
    position: { top: '2vh', left: '50%', transform: 'translateX(-50%)' },
    maxWidth: '60vw',
    animationClass: 'xbox-animation',
    label: 'ACHIEVEMENT'
  },
  ps2: {
    bgGradient: 'rgba(32, 64, 192, 0.9)',
    borderColor: '#2040c0',
    textColor: '#fff',
    boxShadow: '0 0 3vw rgba(32, 64, 192, 0.8), inset 0 0 2vw rgba(32, 64, 192, 0.4)',
    position: { bottom: '5vh', right: '2vw' },
    maxWidth: '20vw',
    animationClass: 'ps2-animation',
    label: '🏆 Trophy Unlocked'
  },
  ps1: {
    bgGradient: 'rgba(100, 100, 100, 0.95)',
    borderColor: '#999',
    textColor: '#fff',
    boxShadow: '0 0 2vw rgba(100, 100, 100, 0.8)',
    position: { bottom: '5vh', right: '2vw' },
    maxWidth: '20vw',
    animationClass: 'ps1-animation',
    label: '🏆 Achievement'
  },
  switch: {
    bgGradient: 'linear-gradient(135deg, #ffd700 0%, #ffed4e 100%)',
    borderColor: '#e60012',
    textColor: '#000',
    boxShadow: '0 0.5vw 2vw rgba(230, 0, 18, 0.5)',
    position: { top: '3vh', left: '50%', transform: 'translateX(-50%)' },
    maxWidth: '50vw',
    animationClass: 'switch-animation',
    label: 'Achievement!'
  },
  wii: {
    bgGradient: 'linear-gradient(135deg, #fff 0%, #f0f0f0 100%)',
    borderColor: '#e60012',
    textColor: '#000',
    boxShadow: '0 0.5vw 2vw rgba(0, 0, 0, 0.2)',
    position: { top: '3vh', left: '50%', transform: 'translateX(-50%)' },
    maxWidth: '50vw',
    animationClass: 'wii-animation',
    label: 'Achievement Get!'
  },
  ds: {
    bgGradient: '#fff',
    borderColor: '#000',
    textColor: '#000',
    boxShadow: '0 0.3vw 1vw rgba(0, 0, 0, 0.3)',
    position: { top: '50%', left: '50%', transform: 'translate(-50%, -50%)' },
    maxWidth: '40vw',
    animationClass: 'ds-animation',
    label: 'Achievement'
  },
  dreamcast: {
    bgGradient: 'linear-gradient(135deg, #d05010 0%, #f08030 100%)',
    borderColor: '#f08030',
    textColor: '#fff',
    boxShadow: '0 0 2vw rgba(240, 128, 48, 0.6)',
    position: { bottom: '5vh', right: '2vw' },
    maxWidth: '25vw',
    animationClass: 'dreamcast-animation',
    label: 'Achievement'
  },
  gamecube: {
    bgGradient: 'linear-gradient(135deg, #524eaa 0%, #6b5bc6 100%)',
    borderColor: '#6b5bc6',
    textColor: '#fff',
    boxShadow: '0 0 2vw rgba(107, 91, 198, 0.5)',
    position: { bottom: '5vh', left: '50%', transform: 'translateX(-50%)' },
    maxWidth: '40vw',
    animationClass: 'gamecube-animation',
    label: 'Achievement Unlocked'
  },
  psp: {
    bgGradient: '#000',
    borderColor: '#fff',
    textColor: '#fff',
    boxShadow: '0 0 1vw rgba(0, 0, 0, 0.8)',
    position: { top: '2vh', left: '50%', transform: 'translateX(-50%)' },
    maxWidth: '40vw',
    animationClass: 'psp-animation',
    label: 'Achievement'
  },
  gameboy: {
    bgGradient: '#0f380f',
    borderColor: '#9bbc0f',
    textColor: '#9bbc0f',
    boxShadow: '0 0 1.5vw rgba(155, 188, 15, 0.5)',
    position: { bottom: '5vh', left: '50%', transform: 'translateX(-50%)' },
    maxWidth: '35vw',
    animationClass: 'gameboy-animation',
    label: 'ACHIEVEMENT!',
    fontFamily: 'monospace'
  },
  n64: {
    bgGradient: 'linear-gradient(135deg, #ffff44 0%, #ffff99 100%)',
    borderColor: '#ffff44',
    textColor: '#00256c',
    boxShadow: '0 0 2vw rgba(255, 255, 68, 0.6)',
    position: { top: '2vh', left: '50%', transform: 'translateX(-50%)' },
    maxWidth: '50vw',
    animationClass: 'n64-animation',
    label: 'ACHIEVEMENT'
  },
  snes: {
    bgGradient: 'linear-gradient(135deg, #c0c0c0 0%, #e0e0e0 100%)',
    borderColor: 'multi',
    textColor: '#000',
    boxShadow: '0 0.3vw 1vw rgba(0, 0, 0, 0.3)',
    position: { bottom: '5vh', left: '50%', transform: 'translateX(-50%)' },
    maxWidth: '40vw',
    animationClass: 'snes-animation',
    label: 'ACHIEVEMENT!'
  }
}

const config = computed(() => themeConfig[props.themeId] || themeConfig.steam)

const toastStyle = computed(() => {
  const cfg = config.value
  const style: Record<string, string> = {
    background: cfg.bgGradient,
    color: cfg.textColor,
    border: `2px solid ${cfg.borderColor}`,
    borderRadius: '0.5vw',
    padding: '2vh 3vw',
    zIndex: '9996',
    boxShadow: cfg.boxShadow,
    maxWidth: cfg.maxWidth,
    position: 'fixed'
  }

  if (props.themeId === 'snes') {
    style.borderColor = 'transparent'
    style.borderTop = '3px solid #ffff44'
    style.borderRight = '3px solid #ff4444'
    style.borderBottom = '3px solid #4444ff'
    style.borderLeft = '3px solid #44ff44'
  } else if (props.themeId === 'xbox' || props.themeId === 'switch' || props.themeId === 'wii' || props.themeId === 'n64') {
    style.textAlign = 'center'
    style.border = `3px solid ${cfg.borderColor}`
  } else if (props.themeId === 'ds') {
    style.textAlign = 'center'
    style.border = `2px solid ${cfg.borderColor}`
  } else if (props.themeId === 'gamecube') {
    style.textAlign = 'center'
  } else if (props.themeId === 'gameboy') {
    style.textAlign = 'center'
    style.fontFamily = 'monospace'
  } else if (props.themeId === 'psp') {
    style.textAlign = 'center'
    style.fontSize = '1.2vh'
  }

  Object.assign(style, cfg.position)
  return style
})

const enterActiveClass = computed(() => `${config.value.animationClass}-enter-active`)
const leaveActiveClass = computed(() => `${config.value.animationClass}-leave-active`)

const achievementLabel = computed(() => config.value.label)

watch(
  () => props.achievement,
  (newAchievement) => {
    if (newAchievement) {
      show.value = true
      clearTimeout(timeout)
      timeout = setTimeout(() => {
        show.value = false
        setTimeout(() => {
          emit('dismissed')
        }, 500)
      }, 5000)
    }
  }
)
</script>

<style scoped>
.achievement-label {
  font-size: 0.9em;
  font-weight: bold;
  margin-bottom: 0.3vh;
  opacity: 0.9;
}

.achievement-name {
  font-size: 1em;
  font-weight: bold;
  margin-bottom: 0.3vh;
}

.achievement-game {
  font-size: 0.85em;
  opacity: 0.8;
}

/* Steam animations */
@keyframes steam-slide-in {
  from {
    transform: translateX(100%);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}

@keyframes steam-slide-out {
  from {
    transform: translateX(0);
    opacity: 1;
  }
  to {
    transform: translateX(100%);
    opacity: 0;
  }
}

.steam-animation-enter-active {
  animation: steam-slide-in 0.5s ease-out;
}

.steam-animation-leave-active {
  animation: steam-slide-out 0.5s ease-out;
}

/* Xbox animations */
@keyframes xbox-slide-in {
  from {
    transform: translateY(-100%);
    opacity: 0;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}

@keyframes xbox-slide-out {
  from {
    transform: translateY(0);
    opacity: 1;
  }
  to {
    transform: translateY(-100%);
    opacity: 0;
  }
}

.xbox-animation-enter-active {
  animation: xbox-slide-in 0.6s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.xbox-animation-leave-active {
  animation: xbox-slide-out 0.5s ease-out;
}

/* PS2 animations */
@keyframes ps2-slide-in {
  from {
    transform: translateX(100%);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}

@keyframes ps2-slide-out {
  from {
    transform: translateX(0);
    opacity: 1;
  }
  to {
    transform: translateX(100%);
    opacity: 0;
  }
}

.ps2-animation-enter-active {
  animation: ps2-slide-in 0.5s ease-out;
}

.ps2-animation-leave-active {
  animation: ps2-slide-out 0.5s ease-out;
}

/* PS1 animations */
@keyframes ps1-slide-in {
  from {
    transform: translateX(100%);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}

@keyframes ps1-slide-out {
  from {
    transform: translateX(0);
    opacity: 1;
  }
  to {
    transform: translateX(100%);
    opacity: 0;
  }
}

.ps1-animation-enter-active {
  animation: ps1-slide-in 0.5s ease-out;
}

.ps1-animation-leave-active {
  animation: ps1-slide-out 0.5s ease-out;
}

/* Switch animations */
@keyframes switch-slide-in {
  from {
    transform: translateY(-100%);
    opacity: 0;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}

@keyframes switch-slide-out {
  from {
    transform: translateY(0);
    opacity: 1;
  }
  to {
    transform: translateY(-100%);
    opacity: 0;
  }
}

.switch-animation-enter-active {
  animation: switch-slide-in 0.5s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.switch-animation-leave-active {
  animation: switch-slide-out 0.5s ease-out;
}

/* Wii animations */
@keyframes wii-slide-in {
  from {
    transform: translateY(-100%);
    opacity: 0;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}

@keyframes wii-slide-out {
  from {
    transform: translateY(0);
    opacity: 1;
  }
  to {
    transform: translateY(-100%);
    opacity: 0;
  }
}

.wii-animation-enter-active {
  animation: wii-slide-in 0.5s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.wii-animation-leave-active {
  animation: wii-slide-out 0.5s ease-out;
}

/* DS animations */
@keyframes ds-slide-in {
  from {
    transform: translateY(-100%);
    opacity: 0;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}

@keyframes ds-slide-out {
  from {
    transform: translateY(0);
    opacity: 1;
  }
  to {
    transform: translateY(-100%);
    opacity: 0;
  }
}

.ds-animation-enter-active {
  animation: ds-slide-in 0.5s ease-out;
}

.ds-animation-leave-active {
  animation: ds-slide-out 0.5s ease-out;
}

/* Dreamcast animations */
@keyframes dreamcast-slide-in {
  from {
    transform: translateX(100%);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}

@keyframes dreamcast-slide-out {
  from {
    transform: translateX(0);
    opacity: 1;
  }
  to {
    transform: translateX(100%);
    opacity: 0;
  }
}

.dreamcast-animation-enter-active {
  animation: dreamcast-slide-in 0.5s ease-out;
}

.dreamcast-animation-leave-active {
  animation: dreamcast-slide-out 0.5s ease-out;
}

/* Gamecube animations */
@keyframes gamecube-slide-in {
  from {
    transform: translateY(100%);
    opacity: 0;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}

@keyframes gamecube-slide-out {
  from {
    transform: translateY(0);
    opacity: 1;
  }
  to {
    transform: translateY(100%);
    opacity: 0;
  }
}

.gamecube-animation-enter-active {
  animation: gamecube-slide-in 0.5s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.gamecube-animation-leave-active {
  animation: gamecube-slide-out 0.5s ease-out;
}

/* PSP animations */
@keyframes psp-slide-in {
  from {
    transform: translateY(-100%);
    opacity: 0;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}

@keyframes psp-slide-out {
  from {
    transform: translateY(0);
    opacity: 1;
  }
  to {
    transform: translateY(-100%);
    opacity: 0;
  }
}

.psp-animation-enter-active {
  animation: psp-slide-in 0.5s ease-out;
}

.psp-animation-leave-active {
  animation: psp-slide-out 0.5s ease-out;
}

/* Gameboy animations */
@keyframes gameboy-slide-in {
  from {
    transform: translateY(100%);
    opacity: 0;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}

@keyframes gameboy-slide-out {
  from {
    transform: translateY(0);
    opacity: 1;
  }
  to {
    transform: translateY(100%);
    opacity: 0;
  }
}

.gameboy-animation-enter-active {
  animation: gameboy-slide-in 0.5s ease-out;
}

.gameboy-animation-leave-active {
  animation: gameboy-slide-out 0.5s ease-out;
}

/* N64 animations */
@keyframes n64-slide-in {
  from {
    transform: translateY(-100%);
    opacity: 0;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}

@keyframes n64-slide-out {
  from {
    transform: translateY(0);
    opacity: 1;
  }
  to {
    transform: translateY(-100%);
    opacity: 0;
  }
}

.n64-animation-enter-active {
  animation: n64-slide-in 0.5s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.n64-animation-leave-active {
  animation: n64-slide-out 0.5s ease-out;
}

/* SNES animations */
@keyframes snes-slide-in {
  from {
    transform: translateY(100%);
    opacity: 0;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}

@keyframes snes-slide-out {
  from {
    transform: translateY(0);
    opacity: 1;
  }
  to {
    transform: translateY(100%);
    opacity: 0;
  }
}

.snes-animation-enter-active {
  animation: snes-slide-in 0.5s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.snes-animation-leave-active {
  animation: snes-slide-out 0.5s ease-out;
}
</style>
