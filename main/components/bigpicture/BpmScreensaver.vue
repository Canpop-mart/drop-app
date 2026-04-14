<template>
  <Transition name="screensaver">
    <div
      v-if="active"
      class="screensaver-overlay"
      @click="dismiss"
      @mousemove="dismiss"
      @keydown="dismiss"
    >
      <canvas ref="canvas" class="screensaver-canvas" />
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";

interface Props {
  active: boolean;
}

const props = defineProps<Props>();
const emit = defineEmits<{ dismiss: [] }>();

const dismiss = () => emit("dismiss");

const canvas = ref<HTMLCanvasElement | null>(null);

// ── Particle system ──────────────────────────────────────────────────────────

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  radius: number;
  baseAlpha: number;
  alpha: number;
  hue: number;
  hueDrift: number;
  pulse: number;
  pulseSpeed: number;
}

const PARTICLE_COUNT = 60;
const CONNECTION_DISTANCE = 180;
const MAX_SPEED = 0.35;
const MIN_SPEED = 0.1;

let particles: Particle[] = [];
let rafId: number | null = null;
let time = 0;
let bgHue = 220;

function createParticle(w: number, h: number): Particle {
  const angle = Math.random() * Math.PI * 2;
  const speed = MIN_SPEED + Math.random() * (MAX_SPEED - MIN_SPEED);
  return {
    x: Math.random() * w,
    y: Math.random() * h,
    vx: Math.cos(angle) * speed,
    vy: Math.sin(angle) * speed,
    radius: 1.5 + Math.random() * 2.5,
    baseAlpha: 0.15 + Math.random() * 0.35,
    alpha: 0,
    hue: 200 + Math.random() * 60, // blue-purple range
    hueDrift: (Math.random() - 0.5) * 0.1,
    pulse: Math.random() * Math.PI * 2,
    pulseSpeed: 0.005 + Math.random() * 0.015,
  };
}

function initParticles(w: number, h: number) {
  particles = [];
  for (let i = 0; i < PARTICLE_COUNT; i++) {
    particles.push(createParticle(w, h));
  }
}

function updateAndDraw(ctx: CanvasRenderingContext2D, w: number, h: number) {
  time++;

  // Slowly drift the background hue so nothing stays constant
  bgHue = 220 + Math.sin(time * 0.0003) * 20;

  // Clear with a very dark, slowly shifting background
  const bgL = 2 + Math.sin(time * 0.0005) * 1;
  ctx.fillStyle = `hsl(${bgHue}, 30%, ${bgL}%)`;
  ctx.fillRect(0, 0, w, h);

  // Update particles
  for (const p of particles) {
    // Gentle drift with slight sine wobble to prevent linear paths
    p.x += p.vx + Math.sin(time * 0.001 + p.pulse) * 0.05;
    p.y += p.vy + Math.cos(time * 0.0012 + p.pulse) * 0.05;

    // Wrap around edges (not bounce — avoids any static edges)
    if (p.x < -20) p.x = w + 20;
    if (p.x > w + 20) p.x = -20;
    if (p.y < -20) p.y = h + 20;
    if (p.y > h + 20) p.y = -20;

    // Pulse alpha for breathing effect
    p.pulse += p.pulseSpeed;
    p.alpha = p.baseAlpha * (0.5 + 0.5 * Math.sin(p.pulse));

    // Drift hue slowly
    p.hue += p.hueDrift;
    if (p.hue > 280) p.hueDrift = -Math.abs(p.hueDrift);
    if (p.hue < 190) p.hueDrift = Math.abs(p.hueDrift);
  }

  // Draw connections between nearby particles
  for (let i = 0; i < particles.length; i++) {
    for (let j = i + 1; j < particles.length; j++) {
      const dx = particles[i].x - particles[j].x;
      const dy = particles[i].y - particles[j].y;
      const dist = Math.sqrt(dx * dx + dy * dy);

      if (dist < CONNECTION_DISTANCE) {
        const lineAlpha =
          (1 - dist / CONNECTION_DISTANCE) *
          Math.min(particles[i].alpha, particles[j].alpha) *
          0.6;
        const midHue = (particles[i].hue + particles[j].hue) / 2;
        ctx.strokeStyle = `hsla(${midHue}, 50%, 55%, ${lineAlpha})`;
        ctx.lineWidth = 0.5;
        ctx.beginPath();
        ctx.moveTo(particles[i].x, particles[i].y);
        ctx.lineTo(particles[j].x, particles[j].y);
        ctx.stroke();
      }
    }
  }

  // Draw particles
  for (const p of particles) {
    ctx.beginPath();
    ctx.arc(p.x, p.y, p.radius, 0, Math.PI * 2);
    ctx.fillStyle = `hsla(${p.hue}, 60%, 60%, ${p.alpha})`;
    ctx.fill();

    // Subtle glow
    ctx.beginPath();
    ctx.arc(p.x, p.y, p.radius * 3, 0, Math.PI * 2);
    ctx.fillStyle = `hsla(${p.hue}, 60%, 60%, ${p.alpha * 0.08})`;
    ctx.fill();
  }
}

function startAnimation() {
  const el = canvas.value;
  if (!el) return;

  const ctx = el.getContext("2d");
  if (!ctx) return;

  const resize = () => {
    el.width = window.innerWidth;
    el.height = window.innerHeight;
  };
  resize();
  window.addEventListener("resize", resize);

  initParticles(el.width, el.height);
  time = 0;

  const loop = () => {
    if (!canvas.value) return;
    updateAndDraw(ctx, el.width, el.height);
    rafId = requestAnimationFrame(loop);
  };
  rafId = requestAnimationFrame(loop);

  // Store cleanup for resize listener
  return () => window.removeEventListener("resize", resize);
}

let cleanupResize: (() => void) | null = null;

function stopAnimation() {
  if (rafId !== null) {
    cancelAnimationFrame(rafId);
    rafId = null;
  }
  cleanupResize?.();
  cleanupResize = null;
  particles = [];
}

watch(
  () => props.active,
  (active) => {
    if (active) {
      // Wait for the canvas to mount
      nextTick(() => {
        cleanupResize = startAnimation() ?? null;
      });
    } else {
      stopAnimation();
    }
  },
);

onMounted(() => {
  if (props.active) {
    cleanupResize = startAnimation() ?? null;
  }
});

onUnmounted(() => {
  stopAnimation();
});
</script>

<style scoped>
.screensaver-enter-active,
.screensaver-leave-active {
  transition: opacity 0.5s;
}

.screensaver-enter-from,
.screensaver-leave-to {
  opacity: 0;
}

.screensaver-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  z-index: 9999;
  cursor: none;
  background: #030308;
}

.screensaver-canvas {
  display: block;
  width: 100%;
  height: 100%;
}
</style>
