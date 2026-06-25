<template>
  <div class="bpm-clock" :style="clockStyle">
    <span class="bpm-clock-time" :style="timeStyle">{{ time }}</span>
    <span v-if="showDate" class="bpm-clock-date" :style="dateStyle">{{ date }}</span>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'

interface Props {
  themeId: string
  showDate?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  showDate: true,
})

const currentTime = ref(new Date())

const time = computed(() => {
  const hours = String(currentTime.value.getHours()).padStart(2, '0')
  const minutes = String(currentTime.value.getMinutes()).padStart(2, '0')
  return `${hours}:${minutes}`
})

const date = computed(() => {
  const options: Intl.DateTimeFormatOptions = {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
  }
  return currentTime.value.toLocaleDateString('en-US', options)
})

// Theme color map
const themeStyles: Record<string, { timeColor: string; dateColor: string; fontFamily?: string }> = {
  steam:     { timeColor: '#66c0f4', dateColor: '#66c0f4', fontFamily: "'Courier New', monospace" },
  switch:    { timeColor: '#ffffff', dateColor: '#ffffff' },
  xbox:      { timeColor: '#107c10', dateColor: '#107c10' },
  ps2:       { timeColor: '#4060e0', dateColor: '#4060e0' },
  dreamcast: { timeColor: '#d05010', dateColor: '#d05010' },
  wii:       { timeColor: '#1a1a2e', dateColor: '#4a5568' },
  ds:        { timeColor: '#1a1a2e', dateColor: '#4a5568', fontFamily: 'monospace' },
  gamecube:  { timeColor: '#b8a0ff', dateColor: '#8070c0' },
  psp:       { timeColor: '#e0e0e8', dateColor: '#9099a8', fontFamily: 'monospace' },
  gameboy:   { timeColor: '#0f380f', dateColor: '#306230', fontFamily: 'monospace' },
  n64:       { timeColor: '#ffff44', dateColor: '#ffff44' },
  ps1:       { timeColor: '#c8c8d8', dateColor: '#9090a8' },
  snes:      { timeColor: '#1a1a2e', dateColor: '#4a5568' },
  custom:    { timeColor: '#e0e0e8', dateColor: '#a0a0b0' },
}

const currentThemeStyle = computed(() => themeStyles[props.themeId] ?? themeStyles.steam)

const clockStyle = computed(() => ({
  display: 'inline-flex',
  flexDirection: 'column' as const,
  alignItems: 'center',
  gap: '0.3vh',
}))

const timeStyle = computed(() => ({
  fontSize: '0.9vw',
  fontWeight: 'bold',
  letterSpacing: '0.05em',
  color: currentThemeStyle.value.timeColor,
  fontFamily: currentThemeStyle.value.fontFamily ?? 'inherit',
}))

const dateStyle = computed(() => ({
  fontSize: '0.7vw',
  opacity: '0.85',
  color: currentThemeStyle.value.dateColor,
  fontFamily: currentThemeStyle.value.fontFamily ?? 'inherit',
}))

let intervalId: ReturnType<typeof setInterval>

onMounted(() => {
  currentTime.value = new Date()
  intervalId = setInterval(() => {
    currentTime.value = new Date()
  }, 1000)
})

onUnmounted(() => {
  if (intervalId) clearInterval(intervalId)
})
</script>
