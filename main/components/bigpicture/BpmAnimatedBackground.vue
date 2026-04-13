<template>
  <div v-show="props.enabled" class="animated-background" :class="`theme-${themeId}`">
    <!-- Steam -->
    <div v-if="themeId === 'steam'" class="steam-bg"></div>

    <!-- PS2 — floating translucent cubes/shapes like XMB system config screen -->
    <div v-else-if="themeId === 'ps2'" class="ps2-bg">
      <div class="ps2-cube c1"></div>
      <div class="ps2-cube c2"></div>
      <div class="ps2-cube c3"></div>
      <div class="ps2-cube c4"></div>
      <div class="ps2-cube c5"></div>
      <div class="ps2-cube c6"></div>
      <div class="ps2-cube c7"></div>
      <div class="ps2-cube c8"></div>
      <div class="ps2-cube c9"></div>
      <div class="ps2-cube c10"></div>
      <div class="ps2-cube c11"></div>
      <div class="ps2-cube c12"></div>
    </div>

    <!-- Dreamcast — pulsing swirl + orbiting particles + color shift -->
    <div v-else-if="themeId === 'dreamcast'" class="dreamcast-bg">
      <div class="dc-gradient-shift"></div>
      <div class="dc-swirl"></div>
      <div class="dc-orbit">
        <div class="dc-particle"></div>
      </div>
      <div class="dc-orbit o2">
        <div class="dc-particle p2"></div>
      </div>
      <div class="dc-orbit o3">
        <div class="dc-particle p3"></div>
      </div>
    </div>

    <!-- Xbox -->
    <div v-else-if="themeId === 'xbox'" class="xbox-bg"></div>

    <!-- Switch -->
    <!-- No background animation for switch -->

    <!-- Wii — floating bubbles with varied colors + gentle sky shift -->
    <div v-else-if="themeId === 'wii'" class="wii-bg">
      <div class="wii-sky-shift"></div>
      <div class="bubble"></div>
      <div class="bubble b2"></div>
      <div class="bubble b3"></div>
      <div class="bubble b4"></div>
      <div class="bubble b5"></div>
      <div class="bubble b6"></div>
      <div class="bubble b7"></div>
      <div class="bubble b8"></div>
    </div>

    <!-- DS -->
    <div v-else-if="themeId === 'ds'" class="ds-bg"></div>

    <!-- GameCube — 3D cube with faces + orbiting mini cubes + nebula + mist -->
    <div v-else-if="themeId === 'gamecube'" class="gamecube-bg">
      <div class="gc-nebula"></div>
      <!-- Mist layers -->
      <div class="gc-mist m1"></div>
      <div class="gc-mist m2"></div>
      <div class="gc-mist m3"></div>
      <div class="gc-scene">
        <div class="gc-cube">
          <div class="gc-face front"></div>
          <div class="gc-face back"></div>
          <div class="gc-face left"></div>
          <div class="gc-face right"></div>
          <div class="gc-face top"></div>
          <div class="gc-face bottom"></div>
        </div>
      </div>
      <!-- Orbiting mini cubes at different radii, speeds, and starting angles -->
      <div class="gc-mini-orbit" style="--orbit-dur: 18s; --orbit-r: 18vw; --start: 0deg"><div class="gc-mini-cube"></div></div>
      <div class="gc-mini-orbit" style="--orbit-dur: 24s; --orbit-r: 22vw; --start: 90deg"><div class="gc-mini-cube s2"></div></div>
      <div class="gc-mini-orbit" style="--orbit-dur: 15s; --orbit-r: 14vw; --start: 200deg"><div class="gc-mini-cube s3"></div></div>
      <div class="gc-mini-orbit" style="--orbit-dur: 30s; --orbit-r: 26vw; --start: 140deg"><div class="gc-mini-cube s4"></div></div>
      <div class="gc-mini-orbit" style="--orbit-dur: 21s; --orbit-r: 20vw; --start: 310deg"><div class="gc-mini-cube s5"></div></div>
    </div>

    <!-- PSP -->
    <div v-else-if="themeId === 'psp'" class="psp-bg"></div>

    <!-- Game Boy -->
    <div v-else-if="themeId === 'gameboy'" class="gameboy-bg"></div>

    <!-- SNES — Mode 7 checkerboard + colored button orbs -->
    <div v-else-if="themeId === 'snes'" class="snes-bg">
      <div class="snes-checker"></div>
      <div class="snes-orb orb-red"></div>
      <div class="snes-orb orb-blue"></div>
      <div class="snes-orb orb-green"></div>
      <div class="snes-orb orb-yellow"></div>
    </div>
  </div>
</template>

<script setup lang="ts">
interface Props {
  themeId: string
  enabled?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  enabled: true,
})
</script>

<style scoped>
.animated-background {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: 0;
  pointer-events: none;
  overflow: hidden;
}

/* ══════════════════════════════════════════════════════════════════════════════
   STEAM — slow radial gradient drift (unchanged)
   ══════════════════════════════════════════════════════════════════════════════ */
@keyframes particle-drift {
  0%   { background-position: 0% 0%; }
  50%  { background-position: 50% 50%; }
  100% { background-position: 0% 0%; }
}

.steam-bg {
  width: 100%;
  height: 100%;
  background: radial-gradient(ellipse at 50% 50%, #1a3a52 0%, #0a0a0a 100%);
  background-size: 200% 200%;
  animation: particle-drift 15s ease-in-out infinite;
}

/* ══════════════════════════════════════════════════════════════════════════════
   PS2 — floating translucent cubes/shapes like XMB system config screen
   Reference: PS2 XMB background with rotating translucent glass cubes
   ══════════════════════════════════════════════════════════════════════════════ */
@keyframes ps2-float {
  0%   { transform: translate(0, 0) rotate(var(--r0, 0deg)) scale(var(--s0, 1)); opacity: 0; }
  8%   { opacity: var(--peak-o, 0.35); }
  50%  { transform: translate(var(--dx, 5vw), var(--dy, -15vh)) rotate(var(--r1, 180deg)) scale(var(--s1, 1.1)); }
  92%  { opacity: var(--peak-o, 0.35); }
  100% { transform: translate(var(--ex, 10vw), var(--ey, -30vh)) rotate(var(--r2, 360deg)) scale(var(--s2, 0.8)); opacity: 0; }
}

.ps2-bg {
  width: 100%;
  height: 100%;
  position: relative;
  overflow: hidden;
}

.ps2-cube {
  position: absolute;
  border: 1.5px solid rgba(60,100,220,0.35);
  background: linear-gradient(135deg, rgba(40,70,180,0.12) 0%, rgba(80,120,255,0.06) 50%, rgba(40,70,180,0.1) 100%);
  box-shadow: inset 0 0 15px rgba(60,100,220,0.08), 0 0 20px rgba(40,80,200,0.06);
  backdrop-filter: blur(1px);
  animation: ps2-float var(--dur, 20s) ease-in-out infinite;
  animation-delay: var(--delay, 0s);
  will-change: transform;
}

/* Large cubes — slow, prominent */
.ps2-cube.c1  { width: 8vw; height: 8vw; bottom: 20%; left: 15%; --dur: 22s; --delay: 0s; --r0: 15deg; --r1: 195deg; --r2: 375deg; --dx: 6vw; --dy: -25vh; --ex: 10vw; --ey: -55vh; --s0: 0.9; --s1: 1.15; --s2: 0.7; --peak-o: 0.4; border-radius: 12%; }
.ps2-cube.c2  { width: 10vw; height: 10vw; bottom: 10%; left: 55%; --dur: 28s; --delay: 3s; --r0: -10deg; --r1: 170deg; --r2: 350deg; --dx: -4vw; --dy: -20vh; --ex: -8vw; --ey: -50vh; --s0: 1; --s1: 1.05; --s2: 0.6; --peak-o: 0.3; border-radius: 8%; }
.ps2-cube.c3  { width: 7vw; height: 7vw; bottom: 30%; left: 75%; --dur: 24s; --delay: 7s; --r0: 30deg; --r1: 210deg; --r2: 390deg; --dx: -3vw; --dy: -30vh; --ex: -5vw; --ey: -60vh; --s0: 0.85; --s1: 1.2; --s2: 0.75; --peak-o: 0.35; border-radius: 15%; }

/* Medium cubes — moderate pace */
.ps2-cube.c4  { width: 5vw; height: 5vw; bottom: 15%; left: 35%; --dur: 18s; --delay: 2s; --r0: -20deg; --r1: 160deg; --r2: 340deg; --dx: 3vw; --dy: -35vh; --ex: 5vw; --ey: -65vh; --s0: 1; --s1: 1.1; --s2: 0.8; --peak-o: 0.3; border-radius: 10%; }
.ps2-cube.c5  { width: 4.5vw; height: 4.5vw; bottom: 25%; left: 8%; --dur: 20s; --delay: 10s; --r0: 45deg; --r1: 225deg; --r2: 405deg; --dx: 8vw; --dy: -22vh; --ex: 12vw; --ey: -48vh; --s0: 0.9; --s1: 1.15; --s2: 0.7; --peak-o: 0.35; border-radius: 18%; }
.ps2-cube.c6  { width: 6vw; height: 6vw; bottom: 5%; left: 88%; --dur: 25s; --delay: 5s; --r0: -35deg; --r1: 145deg; --r2: 325deg; --dx: -7vw; --dy: -28vh; --ex: -12vw; --ey: -58vh; --s0: 0.95; --s1: 1.1; --s2: 0.65; --peak-o: 0.3; border-radius: 12%; }

/* Small cubes — quicker, more scattered */
.ps2-cube.c7  { width: 3vw; height: 3vw; bottom: 40%; left: 45%; --dur: 16s; --delay: 1s; --r0: 60deg; --r1: 240deg; --r2: 420deg; --dx: 2vw; --dy: -18vh; --ex: 3vw; --ey: -42vh; --s0: 1; --s1: 1.2; --s2: 0.9; --peak-o: 0.25; border-radius: 20%; }
.ps2-cube.c8  { width: 2.5vw; height: 2.5vw; bottom: 35%; left: 25%; --dur: 14s; --delay: 8s; --r0: -50deg; --r1: 130deg; --r2: 310deg; --dx: -2vw; --dy: -20vh; --ex: -4vw; --ey: -45vh; --s0: 0.8; --s1: 1.3; --s2: 0.9; --peak-o: 0.2; border-radius: 25%; }
.ps2-cube.c9  { width: 3.5vw; height: 3.5vw; bottom: 50%; left: 65%; --dur: 17s; --delay: 12s; --r0: 25deg; --r1: 205deg; --r2: 385deg; --dx: 4vw; --dy: -15vh; --ex: 6vw; --ey: -38vh; --s0: 0.9; --s1: 1.15; --s2: 0.85; --peak-o: 0.25; border-radius: 15%; }

/* Tiny accent cubes — fast, atmospheric */
.ps2-cube.c10 { width: 2vw; height: 2vw; bottom: 45%; left: 5%; --dur: 12s; --delay: 4s; --r0: 80deg; --r1: 260deg; --r2: 440deg; --dx: 5vw; --dy: -25vh; --ex: 8vw; --ey: -52vh; --s0: 1; --s1: 1.4; --s2: 0.7; --peak-o: 0.2; border-radius: 30%; }
.ps2-cube.c11 { width: 1.8vw; height: 1.8vw; bottom: 55%; left: 50%; --dur: 13s; --delay: 6s; --r0: -70deg; --r1: 110deg; --r2: 290deg; --dx: -3vw; --dy: -22vh; --ex: -5vw; --ey: -46vh; --s0: 0.85; --s1: 1.3; --s2: 0.8; --peak-o: 0.18; border-radius: 25%; }
.ps2-cube.c12 { width: 2.2vw; height: 2.2vw; bottom: 60%; left: 82%; --dur: 15s; --delay: 9s; --r0: 40deg; --r1: 220deg; --r2: 400deg; --dx: -6vw; --dy: -18vh; --ex: -9vw; --ey: -40vh; --s0: 0.9; --s1: 1.25; --s2: 0.75; --peak-o: 0.22; border-radius: 20%; }

/* ══════════════════════════════════════════════════════════════════════════════
   DREAMCAST — pulsing swirl logo + orbiting particles + color shift
   Reference: Dreamcast's swirl logo and VMU screen aesthetic
   ══════════════════════════════════════════════════════════════════════════════ */
@keyframes dc-color-shift {
  0%   { background: radial-gradient(ellipse at 40% 40%, rgba(208,80,16,0.08) 0%, transparent 70%); }
  33%  { background: radial-gradient(ellipse at 60% 50%, rgba(240,140,40,0.1) 0%, transparent 70%); }
  66%  { background: radial-gradient(ellipse at 50% 60%, rgba(180,60,10,0.08) 0%, transparent 70%); }
  100% { background: radial-gradient(ellipse at 40% 40%, rgba(208,80,16,0.08) 0%, transparent 70%); }
}

@keyframes dc-swirl-pulse {
  0%   { transform: translate(-50%, -50%) rotate(0deg) scale(0.9); opacity: 0.12; }
  50%  { transform: translate(-50%, -50%) rotate(180deg) scale(1.1); opacity: 0.2; }
  100% { transform: translate(-50%, -50%) rotate(360deg) scale(0.9); opacity: 0.12; }
}

@keyframes dc-orbit-spin {
  0%   { transform: translate(-50%, -50%) rotate(0deg); }
  100% { transform: translate(-50%, -50%) rotate(360deg); }
}

.dreamcast-bg {
  width: 100%;
  height: 100%;
  position: relative;
  overflow: hidden;
}

.dc-gradient-shift {
  position: absolute;
  inset: 0;
  animation: dc-color-shift 20s ease-in-out infinite;
}

.dc-swirl {
  position: absolute;
  top: 50%;
  left: 50%;
  width: 25vw;
  height: 25vw;
  background: conic-gradient(from 0deg, rgba(208,80,16,0.3), rgba(240,128,48,0.15), transparent, rgba(208,80,16,0.3));
  border-radius: 50%;
  animation: dc-swirl-pulse 20s linear infinite;
  will-change: transform;
}

.dc-orbit {
  position: absolute;
  top: 50%;
  left: 50%;
  width: 35vw;
  height: 35vw;
  border: 1px solid rgba(208,100,30,0.06);
  border-radius: 50%;
  animation: dc-orbit-spin 25s linear infinite;
  will-change: transform;
}
.dc-orbit.o2 { width: 28vw; height: 28vw; animation-duration: 18s; animation-direction: reverse; border-color: rgba(240,140,50,0.05); }
.dc-orbit.o3 { width: 42vw; height: 42vw; animation-duration: 35s; border-color: rgba(208,80,16,0.04); }

.dc-particle {
  position: absolute;
  top: 0;
  left: 50%;
  width: 6px;
  height: 6px;
  margin-left: -3px;
  margin-top: -3px;
  background: radial-gradient(circle, rgba(240,140,50,0.8), rgba(208,80,16,0.4));
  border-radius: 50%;
  box-shadow: 0 0 8px rgba(240,140,50,0.4);
}
.dc-particle.p2 { width: 4px; height: 4px; margin-left: -2px; margin-top: -2px; background: radial-gradient(circle, rgba(255,180,80,0.7), rgba(240,140,50,0.3)); }
.dc-particle.p3 { width: 5px; height: 5px; margin-left: -2.5px; margin-top: -2.5px; background: radial-gradient(circle, rgba(208,80,16,0.6), rgba(180,60,10,0.3)); }

/* ══════════════════════════════════════════════════════════════════════════════
   XBOX — pulsing green grid (unchanged)
   ══════════════════════════════════════════════════════════════════════════════ */
@keyframes grid-pulse {
  0%   { opacity: 0.3; }
  50%  { opacity: 0.7; }
  100% { opacity: 0.3; }
}

.xbox-bg {
  width: 100%;
  height: 100%;
  background: #000;
  background-image: linear-gradient(0deg, transparent 24%, rgba(16, 124, 16, 0.3) 25%, rgba(16, 124, 16, 0.3) 26%, transparent 27%, transparent 74%, rgba(16, 124, 16, 0.3) 75%, rgba(16, 124, 16, 0.3) 76%, transparent 77%, transparent),
    linear-gradient(90deg, transparent 24%, rgba(16, 124, 16, 0.3) 25%, rgba(16, 124, 16, 0.3) 26%, transparent 27%, transparent 74%, rgba(16, 124, 16, 0.3) 75%, rgba(16, 124, 16, 0.3) 76%, transparent 77%, transparent);
  background-size: 50px 50px;
  animation: grid-pulse 4s ease-in-out infinite;
  will-change: opacity;
}

/* ══════════════════════════════════════════════════════════════════════════════
   WII — floating bubbles with color tints + gentle sky shift
   Reference: Wii Menu / Mii Channel soft bubbly aesthetic
   ══════════════════════════════════════════════════════════════════════════════ */
@keyframes wii-sky {
  0%   { background: linear-gradient(180deg, rgba(200,230,255,0.3) 0%, transparent 100%); }
  50%  { background: linear-gradient(180deg, rgba(220,240,255,0.4) 0%, transparent 100%); }
  100% { background: linear-gradient(180deg, rgba(200,230,255,0.3) 0%, transparent 100%); }
}

@keyframes wii-float {
  0%   { transform: translate(0, 0) scale(1); opacity: 0; }
  5%   { opacity: 1; }
  50%  { transform: translate(var(--drift-x, 3vw), calc(var(--drift-y, -40vh))) scale(var(--mid-scale, 1.1)); }
  95%  { opacity: 1; }
  100% { transform: translate(var(--end-x, 5vw), var(--end-y, -90vh)) scale(var(--end-scale, 0.6)); opacity: 0; }
}

.wii-bg {
  width: 100%;
  height: 100%;
  position: relative;
  overflow: hidden;
}

.wii-sky-shift {
  position: absolute;
  inset: 0;
  animation: wii-sky 12s ease-in-out infinite;
}

.wii-bg .bubble {
  position: absolute;
  bottom: -5%;
  border-radius: 50%;
  will-change: transform;
  animation: wii-float var(--dur, 10s) ease-in-out infinite;
  animation-delay: var(--delay, 0s);
  background: radial-gradient(circle at 35% 30%,
    var(--bubble-highlight, rgba(255,255,255,0.6)),
    var(--bubble-mid, rgba(180,210,255,0.2)),
    var(--bubble-edge, rgba(130,180,240,0.05))
  );
  box-shadow: inset 0 -2px 6px var(--bubble-shadow, rgba(100,160,230,0.1));
}

/* Individual bubble configs using CSS custom properties */
.wii-bg .bubble    { --dur: 9s;  --delay: 0s;   left: 5%;  width: 7vw; height: 7vw; --drift-x: 2vw; --drift-y: -35vh; --end-x: 4vw; --end-y: -85vh; --mid-scale: 1.05; --end-scale: 0.7; }
.wii-bg .bubble.b2 { --dur: 12s; --delay: 1.5s; left: 15%; width: 10vw; height: 10vw; --drift-x: -2vw; --drift-y: -30vh; --end-x: -3vw; --end-y: -80vh; --mid-scale: 1.15; --end-scale: 0.5; --bubble-mid: rgba(200,180,255,0.15); }
.wii-bg .bubble.b3 { --dur: 10s; --delay: 3s;   left: 28%; width: 6vw; height: 6vw; --drift-x: 4vw; --drift-y: -40vh; --end-x: 6vw; --end-y: -90vh; --mid-scale: 1.0; --end-scale: 0.8; --bubble-mid: rgba(180,230,200,0.15); }
.wii-bg .bubble.b4 { --dur: 14s; --delay: 0.5s; left: 40%; width: 12vw; height: 12vw; --drift-x: -1vw; --drift-y: -25vh; --end-x: -2vw; --end-y: -75vh; --mid-scale: 1.1; --end-scale: 0.4; }
.wii-bg .bubble.b5 { --dur: 8s;  --delay: 4s;   left: 55%; width: 5vw; height: 5vw; --drift-x: 3vw; --drift-y: -45vh; --end-x: 5vw; --end-y: -95vh; --mid-scale: 1.0; --end-scale: 0.9; --bubble-mid: rgba(255,200,200,0.12); }
.wii-bg .bubble.b6 { --dur: 11s; --delay: 2s;   left: 68%; width: 9vw; height: 9vw; --drift-x: -3vw; --drift-y: -32vh; --end-x: -4vw; --end-y: -82vh; --mid-scale: 1.08; --end-scale: 0.55; --bubble-mid: rgba(200,220,255,0.18); }
.wii-bg .bubble.b7 { --dur: 13s; --delay: 5s;   left: 80%; width: 8vw; height: 8vw; --drift-x: 2vw; --drift-y: -28vh; --end-x: 3vw; --end-y: -78vh; --mid-scale: 1.12; --end-scale: 0.6; }
.wii-bg .bubble.b8 { --dur: 9.5s; --delay: 3.5s; left: 92%; width: 6vw; height: 6vw; --drift-x: -5vw; --drift-y: -38vh; --end-x: -7vw; --end-y: -88vh; --mid-scale: 1.05; --end-scale: 0.65; --bubble-mid: rgba(220,200,255,0.15); }

/* ══════════════════════════════════════════════════════════════════════════════
   DS — diagonal stripe sweep (unchanged)
   ══════════════════════════════════════════════════════════════════════════════ */
@keyframes diagonal-sweep {
  0%   { background-position: 0% 0%; }
  100% { background-position: 100% 100%; }
}

.ds-bg {
  width: 100%;
  height: 100%;
  background: repeating-linear-gradient(
    45deg,
    #000,
    #000 10px,
    #1a1a2e 10px,
    #1a1a2e 20px
  );
  animation: diagonal-sweep 20s linear infinite;
  will-change: background-position;
}

/* ══════════════════════════════════════════════════════════════════════════════
   GAMECUBE — translucent 3D cube with faces + orbiting mini cube + nebula
   Reference: GameCube boot sequence spinning cube
   ══════════════════════════════════════════════════════════════════════════════ */
@keyframes gc-nebula-shift {
  0%   { background-position: 0% 50%; }
  50%  { background-position: 100% 50%; }
  100% { background-position: 0% 50%; }
}

@keyframes gc-cube-spin {
  0%   { transform: rotateX(-20deg) rotateY(0deg); }
  100% { transform: rotateX(-20deg) rotateY(360deg); }
}

@keyframes gc-mini-orbit {
  0%   { transform: translate(-50%, -50%) rotate(var(--start, 0deg)) translateX(var(--orbit-r, 18vw)) rotate(calc(-1 * var(--start, 0deg))); }
  100% { transform: translate(-50%, -50%) rotate(calc(var(--start, 0deg) + 360deg)) translateX(var(--orbit-r, 18vw)) rotate(calc(-1 * var(--start, 0deg) - 360deg)); }
}

@keyframes gc-mini-self-spin {
  0%   { transform: rotateX(0deg) rotateY(0deg); }
  100% { transform: rotateX(360deg) rotateY(360deg); }
}

@keyframes gc-mist-drift {
  0%   { transform: translateX(-10%) translateY(5%); opacity: 0; }
  20%  { opacity: var(--mist-peak, 0.07); }
  80%  { opacity: var(--mist-peak, 0.07); }
  100% { transform: translateX(10%) translateY(-5%); opacity: 0; }
}

.gamecube-bg {
  width: 100%;
  height: 100%;
  position: relative;
  overflow: hidden;
}

.gc-nebula {
  position: absolute;
  inset: 0;
  background:
    radial-gradient(ellipse at 30% 40%, rgba(120,80,200,0.12) 0%, transparent 50%),
    radial-gradient(ellipse at 70% 60%, rgba(80,60,180,0.1) 0%, transparent 50%),
    radial-gradient(ellipse at 50% 80%, rgba(100,70,190,0.08) 0%, transparent 40%);
  background-size: 200% 200%;
  animation: gc-nebula-shift 30s ease-in-out infinite;
}

/* Mist / fog layers */
.gc-mist {
  position: absolute;
  border-radius: 50%;
  filter: blur(6vw);
  pointer-events: none;
  will-change: transform, opacity;
}
.gc-mist.m1 {
  width: 50vw; height: 30vh;
  top: 55%; left: 10%;
  background: radial-gradient(ellipse, rgba(120,100,200,0.14) 0%, transparent 70%);
  --mist-peak: 0.09;
  animation: gc-mist-drift 25s ease-in-out infinite;
}
.gc-mist.m2 {
  width: 40vw; height: 25vh;
  top: 20%; left: 50%;
  background: radial-gradient(ellipse, rgba(90,70,180,0.12) 0%, transparent 70%);
  --mist-peak: 0.07;
  animation: gc-mist-drift 35s ease-in-out infinite reverse;
}
.gc-mist.m3 {
  width: 60vw; height: 20vh;
  top: 70%; left: 25%;
  background: radial-gradient(ellipse, rgba(140,120,220,0.1) 0%, transparent 70%);
  --mist-peak: 0.06;
  animation: gc-mist-drift 30s ease-in-out 10s infinite;
}

.gc-scene {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  perspective: 800px;
}

.gc-cube {
  width: 12vw;
  height: 12vw;
  position: relative;
  transform-style: preserve-3d;
  animation: gc-cube-spin 25s linear infinite;
  will-change: transform;
}

.gc-face {
  position: absolute;
  width: 12vw;
  height: 12vw;
  border: 1px solid rgba(160,140,255,0.15);
  background: rgba(82,78,170,0.04);
  backface-visibility: visible;
}
.gc-face.front  { transform: translateZ(6vw); }
.gc-face.back   { transform: translateZ(-6vw) rotateY(180deg); }
.gc-face.left   { transform: translateX(-6vw) rotateY(-90deg); }
.gc-face.right  { transform: translateX(6vw) rotateY(90deg); }
.gc-face.top    { transform: translateY(-6vw) rotateX(90deg); }
.gc-face.bottom { transform: translateY(6vw) rotateX(-90deg); }

.gc-mini-orbit {
  position: absolute;
  top: 50%;
  left: 50%;
  animation: gc-mini-orbit var(--orbit-dur, 18s) linear infinite;
  will-change: transform;
}

.gc-mini-cube {
  width: 2.5vw;
  height: 2.5vw;
  border: 1px solid rgba(180,160,255,0.2);
  background: rgba(82,78,170,0.08);
  animation: gc-mini-self-spin 8s linear infinite;
  will-change: transform;
}
.gc-mini-cube.s2 {
  width: 1.8vw; height: 1.8vw;
  border-color: rgba(160,140,255,0.25);
  background: rgba(100,80,200,0.06);
  animation-duration: 12s;
}
.gc-mini-cube.s3 {
  width: 3.2vw; height: 3.2vw;
  border-color: rgba(140,120,255,0.18);
  background: rgba(70,60,160,0.05);
  animation-duration: 6s;
  animation-direction: reverse;
}
.gc-mini-cube.s4 {
  width: 1.2vw; height: 1.2vw;
  border-color: rgba(180,170,255,0.3);
  background: rgba(110,90,210,0.07);
  animation-duration: 15s;
}
.gc-mini-cube.s5 {
  width: 2vw; height: 2vw;
  border-color: rgba(150,130,255,0.22);
  background: rgba(90,75,190,0.06);
  animation-duration: 10s;
  animation-direction: reverse;
}

/* ══════════════════════════════════════════════════════════════════════════════
   PSP — horizontal line scrolling (unchanged)
   ══════════════════════════════════════════════════════════════════════════════ */
@keyframes wave-motion {
  0%   { background-position: 0 0; }
  100% { background-position: 100% 0; }
}

.psp-bg {
  width: 100%;
  height: 100%;
  background: #000;
  background-image: repeating-linear-gradient(
    0deg,
    transparent,
    transparent 10px,
    rgba(255, 255, 255, 0.05) 10px,
    rgba(255, 255, 255, 0.05) 20px
  );
  animation: wave-motion 8s linear infinite;
  will-change: background-position;
}

/* ══════════════════════════════════════════════════════════════════════════════
   GAME BOY — falling pixel lines (unchanged)
   ══════════════════════════════════════════════════════════════════════════════ */
@keyframes pixel-fall {
  0%   { background-position: 0 0; }
  100% { background-position: 0 100px; }
}

.gameboy-bg {
  width: 100%;
  height: 100%;
  background: #0f380f;
  background-image: repeating-linear-gradient(
    0deg,
    #9bbc0f 0px,
    #9bbc0f 2px,
    transparent 2px,
    transparent 20px
  );
  animation: pixel-fall 4s linear infinite;
  will-change: background-position;
}

/* ══════════════════════════════════════════════════════════════════════════════
   SNES — Mode 7 perspective checkerboard + floating button-colored orbs
   Reference: SNES Mode 7 rotating floors + the 4 SNES button colors
   ══════════════════════════════════════════════════════════════════════════════ */
@keyframes snes-checker-scroll {
  0%   { background-position: 0 0; }
  100% { background-position: 80px 80px; }
}

@keyframes snes-orb-float {
  0%   { transform: translate(0, 0) scale(1); opacity: 0; }
  15%  { opacity: var(--orb-opacity, 0.12); }
  50%  { transform: translate(var(--orb-dx, 5vw), var(--orb-dy, -20vh)) scale(var(--orb-mid-s, 1.3)); }
  85%  { opacity: var(--orb-opacity, 0.12); }
  100% { transform: translate(var(--orb-ex, 8vw), var(--orb-ey, -50vh)) scale(0.6); opacity: 0; }
}

.snes-bg {
  width: 100%;
  height: 100%;
  position: relative;
  overflow: hidden;
}

.snes-checker {
  position: absolute;
  inset: 0;
  background-image:
    linear-gradient(45deg, rgba(100,100,180,0.04) 25%, transparent 25%),
    linear-gradient(-45deg, rgba(100,100,180,0.04) 25%, transparent 25%),
    linear-gradient(45deg, transparent 75%, rgba(100,100,180,0.04) 75%),
    linear-gradient(-45deg, transparent 75%, rgba(100,100,180,0.04) 75%);
  background-size: 40px 40px;
  background-position: 0 0, 0 20px, 20px -20px, -20px 0px;
  animation: snes-checker-scroll 12s linear infinite;
  /* Perspective distortion to simulate Mode 7 */
  transform: perspective(400px) rotateX(25deg);
  transform-origin: center 120%;
  opacity: 0.6;
}

.snes-orb {
  position: absolute;
  border-radius: 50%;
  animation: snes-orb-float var(--orb-dur, 16s) ease-in-out infinite;
  animation-delay: var(--orb-delay, 0s);
  filter: blur(30px);
  will-change: transform;
}

.snes-orb.orb-red {
  width: 18vw; height: 18vw;
  bottom: 10%; left: 5%;
  background: rgba(234,51,35,0.25);
  --orb-dur: 18s; --orb-delay: 0s;
  --orb-dx: 8vw; --orb-dy: -25vh; --orb-ex: 12vw; --orb-ey: -55vh; --orb-mid-s: 1.2; --orb-opacity: 0.15;
}

.snes-orb.orb-blue {
  width: 15vw; height: 15vw;
  bottom: 15%; left: 60%;
  background: rgba(40,98,200,0.25);
  --orb-dur: 22s; --orb-delay: 4s;
  --orb-dx: -5vw; --orb-dy: -30vh; --orb-ex: -8vw; --orb-ey: -60vh; --orb-mid-s: 1.4; --orb-opacity: 0.12;
}

.snes-orb.orb-green {
  width: 14vw; height: 14vw;
  bottom: 5%; left: 35%;
  background: rgba(90,176,51,0.2);
  --orb-dur: 20s; --orb-delay: 8s;
  --orb-dx: 3vw; --orb-dy: -20vh; --orb-ex: 5vw; --orb-ey: -50vh; --orb-mid-s: 1.3; --orb-opacity: 0.1;
}

.snes-orb.orb-yellow {
  width: 12vw; height: 12vw;
  bottom: 20%; left: 80%;
  background: rgba(245,197,27,0.2);
  --orb-dur: 16s; --orb-delay: 12s;
  --orb-dx: -6vw; --orb-dy: -22vh; --orb-ex: -10vw; --orb-ey: -48vh; --orb-mid-s: 1.25; --orb-opacity: 0.12;
}
</style>
