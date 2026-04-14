/**
 * BPM Audio Feedback System
 *
 * Generates UI sounds programmatically using the Web Audio API.
 * No external .wav files needed — all sounds are synthesized.
 *
 * Sound profiles are tuned to closely match the actual UI sounds of
 * each console using frequency analysis and documented note data:
 *   - Steam Deck: Soft ticks, warm muted sine tones
 *   - Nintendo Switch: Sharp metallic snaps, bandpass-filtered noise bursts
 *   - Xbox: Deep resonant taps, low-frequency whooshes
 *   - PlayStation 5: High-pitched crisp tones, fast sharp envelopes
 *   - Wii: Musical notes (G4=392Hz select, D3=147Hz back), bouncy feel
 */

// ── Audio Context ────────────────────────────────────────────────────────────

let audioCtx: AudioContext | null = null;
let masterGain: GainNode | null = null;
let audioFailed = false;
let enabled = true;
let activeProfile: SoundProfileId = "steam";

function ctx(): { ac: AudioContext; out: GainNode } | null {
  if (audioFailed) return null;
  try {
    if (!audioCtx) {
      audioCtx = new AudioContext();
      masterGain = audioCtx.createGain();
      masterGain.gain.value = 0.18;
      masterGain.connect(audioCtx.destination);
    }
    if (audioCtx.state === "suspended") {
      audioCtx.resume().catch(() => {
        /* audio device not ready — silently skip */
      });
    }
    if (audioCtx.state === "closed") {
      // Context was closed; recreate on next call
      audioCtx = null;
      masterGain = null;
      return null;
    }
    return { ac: audioCtx, out: masterGain! };
  } catch {
    console.warn("[BPM:AUDIO] Audio device unavailable — disabling sound feedback");
    audioFailed = true;
    return null;
  }
}

// ── Primitive sound generators ──────────────────────────────────────────────

/** Single oscillator tone with ADSR envelope */
function tone(
  freq: number,
  dur: number,
  type: OscillatorType = "sine",
  attack = 0.005,
  release?: number,
  volume = 1,
) {
  if (!enabled) return;
  const c = ctx();
  if (!c) return;
  const { ac, out } = c;
  const t = ac.currentTime;

  const osc = ac.createOscillator();
  const env = ac.createGain();
  osc.type = type;
  osc.frequency.value = freq;

  const rel = release ?? dur;
  env.gain.setValueAtTime(0, t);
  env.gain.linearRampToValueAtTime(volume, t + attack);
  env.gain.linearRampToValueAtTime(0, t + rel);

  osc.connect(env);
  env.connect(out);
  osc.start(t);
  osc.stop(t + dur + 0.01);
}

/** Pitch-sweeping tone (for whoosh effects) */
function sweep(
  freqStart: number,
  freqEnd: number,
  dur: number,
  type: OscillatorType = "sine",
  volume = 0.7,
) {
  if (!enabled) return;
  const c = ctx();
  if (!c) return;
  const { ac, out } = c;
  const t = ac.currentTime;

  const osc = ac.createOscillator();
  const env = ac.createGain();
  osc.type = type;
  osc.frequency.setValueAtTime(freqStart, t);
  osc.frequency.exponentialRampToValueAtTime(freqEnd, t + dur);

  env.gain.setValueAtTime(volume, t);
  env.gain.linearRampToValueAtTime(0, t + dur);

  osc.connect(env);
  env.connect(out);
  osc.start(t);
  osc.stop(t + dur + 0.01);
}

/** Filtered noise burst (for click/snap sounds) */
function noise(
  dur: number,
  filterFreq: number,
  filterQ = 1,
  filterType: BiquadFilterType = "bandpass",
  volume = 0.6,
) {
  if (!enabled) return;
  const c = ctx();
  if (!c) return;
  const { ac, out } = c;
  const t = ac.currentTime;

  const len = Math.floor(ac.sampleRate * dur);
  const buf = ac.createBuffer(1, len, ac.sampleRate);
  const data = buf.getChannelData(0);
  for (let i = 0; i < len; i++) data[i] = Math.random() * 2 - 1;

  const src = ac.createBufferSource();
  src.buffer = buf;

  const filt = ac.createBiquadFilter();
  filt.type = filterType;
  filt.frequency.value = filterFreq;
  filt.Q.value = filterQ;

  const env = ac.createGain();
  env.gain.setValueAtTime(volume, t);
  env.gain.exponentialRampToValueAtTime(0.001, t + dur);

  src.connect(filt);
  filt.connect(env);
  env.connect(out);
  src.start(t);
  src.stop(t + dur + 0.01);
}

/** Delayed tone (for multi-note sequences) */
function delayed(ms: number, fn: () => void) {
  setTimeout(fn, ms);
}

// ── Sound Types ─────────────────────────────────────────────────────────────

export type BpSound =
  | "focus"
  | "select"
  | "back"
  | "error"
  | "overlay-open"
  | "overlay-close";

// ── Sound Profiles ──────────────────────────────────────────────────────────

export type SoundProfileId =
  | "steam"
  | "switch"
  | "xbox"
  | "playstation"
  | "wii"
  | "ps2"
  | "ds"
  | "dreamcast"
  | "gamecube"
  | "psp"
  | "gameboy"
  | "n64"
  | "ps1"
  | "snes";

export interface SoundProfile {
  id: SoundProfileId;
  label: string;
  description: string;
  sounds: Record<BpSound, () => void>;
}

// ═══════════════════════════════════════════════════════════════════════════
// STEAM DECK — Soft, warm, muted. Low-volume ticks and gentle tones.
// The Steam Deck UI has very subtle, almost "tapping" focus sounds
// and gentle confirmation tones. Warm sine waves, short durations.
// ═══════════════════════════════════════════════════════════════════════════
const steamProfile: SoundProfile = {
  id: "steam",
  label: "Steam",
  description: "Soft clicks, warm tones",
  sounds: {
    // Focus: very subtle tick — filtered noise + tiny sine ping
    focus: () => {
      noise(0.015, 3000, 2, "highpass", 0.25);
      tone(800, 0.02, "sine", 0.002, 0.015, 0.3);
    },
    // Select: warm confirmation — two gentle sine notes ascending
    select: () => {
      tone(523, 0.06, "sine", 0.005, 0.05, 0.8);   // C5
      delayed(45, () => tone(659, 0.05, "sine", 0.005, 0.04, 0.7)); // E5
    },
    // Back: descending soft tone
    back: () => {
      tone(523, 0.05, "sine", 0.005, 0.04, 0.6);
      delayed(40, () => tone(392, 0.06, "sine", 0.005, 0.05, 0.5));
    },
    error: () => tone(220, 0.1, "triangle", 0.01, 0.08, 0.6),
    "overlay-open": () => {
      tone(392, 0.08, "sine", 0.008, 0.07, 0.5);
      delayed(50, () => tone(523, 0.07, "sine", 0.008, 0.06, 0.5));
    },
    "overlay-close": () => {
      tone(523, 0.07, "sine", 0.005, 0.06, 0.4);
      delayed(40, () => tone(392, 0.08, "sine", 0.005, 0.07, 0.4));
    },
  },
};

// ═══════════════════════════════════════════════════════════════════════════
// NINTENDO SWITCH — Sharp metallic snaps and clicks.
// The Switch is famous for its distinctive "snap" sound on navigation.
// Uses high-frequency bandpass-filtered noise for the iconic click,
// plus quick square wave bursts for confirmations.
// ═══════════════════════════════════════════════════════════════════════════
const switchProfile: SoundProfile = {
  id: "switch",
  label: "Switch",
  description: "Sharp snaps and clicks",
  sounds: {
    // Focus: the iconic Switch snap — high-freq filtered noise burst
    focus: () => {
      noise(0.018, 6000, 4, "bandpass", 0.7);
      tone(2000, 0.01, "square", 0.001, 0.008, 0.15); // tiny metallic ring
    },
    // Select: quick two-tone confirmation with square wave character
    select: () => {
      noise(0.012, 5000, 3, "bandpass", 0.4);
      tone(1047, 0.03, "square", 0.002, 0.025, 0.5); // C6
      delayed(30, () => tone(1319, 0.025, "square", 0.002, 0.02, 0.45)); // E6
    },
    // Back: lower snap
    back: () => {
      noise(0.015, 3000, 3, "bandpass", 0.5);
      tone(784, 0.03, "square", 0.002, 0.025, 0.4); // G5
    },
    error: () => {
      tone(330, 0.04, "square", 0.003, 0.035, 0.5);
      delayed(50, () => tone(262, 0.06, "square", 0.003, 0.05, 0.5));
    },
    "overlay-open": () => {
      noise(0.01, 5000, 3, "bandpass", 0.3);
      tone(784, 0.04, "square", 0.002, 0.035, 0.4);
      delayed(35, () => tone(1047, 0.035, "square", 0.002, 0.03, 0.4));
    },
    "overlay-close": () => {
      noise(0.01, 4000, 3, "bandpass", 0.3);
      tone(1047, 0.035, "square", 0.002, 0.03, 0.35);
      delayed(35, () => tone(784, 0.04, "square", 0.002, 0.035, 0.35));
    },
  },
};

// ═══════════════════════════════════════════════════════════════════════════
// XBOX — Deep, resonant, subtle. Whoosh-like transitions.
// The Xbox dashboard uses subtle low-frequency taps and airy whooshes.
// Deep sine/triangle tones with longer envelopes, sub-bass presence.
// ═══════════════════════════════════════════════════════════════════════════
const xboxProfile: SoundProfile = {
  id: "xbox",
  label: "Xbox",
  description: "Deep taps, airy whooshes",
  sounds: {
    // Focus: subtle low tap with a hint of whoosh
    focus: () => {
      tone(300, 0.035, "triangle", 0.003, 0.03, 0.5);
      sweep(800, 300, 0.03, "sine", 0.15); // subtle whoosh down
    },
    // Select: resonant confirmation with body
    select: () => {
      tone(220, 0.05, "sine", 0.005, 0.04, 0.3); // sub layer
      tone(440, 0.08, "sine", 0.008, 0.065, 0.7);
      delayed(50, () => tone(554, 0.06, "sine", 0.008, 0.05, 0.5)); // C#5
    },
    // Back: downward sweep
    back: () => {
      sweep(500, 250, 0.06, "triangle", 0.5);
      tone(250, 0.05, "sine", 0.005, 0.04, 0.4);
    },
    error: () => {
      tone(150, 0.12, "triangle", 0.01, 0.1, 0.6);
      tone(165, 0.1, "sawtooth", 0.01, 0.08, 0.2); // slight dissonance
    },
    "overlay-open": () => {
      sweep(200, 500, 0.12, "sine", 0.4);
      delayed(40, () => tone(440, 0.1, "sine", 0.01, 0.08, 0.5));
    },
    "overlay-close": () => {
      sweep(500, 200, 0.1, "sine", 0.35);
      delayed(40, () => tone(220, 0.1, "sine", 0.008, 0.08, 0.4));
    },
  },
};

// ═══════════════════════════════════════════════════════════════════════════
// PLAYSTATION 5 — Crisp, high-pitched, sharp.
// PS5 sounds are notably higher-pitched than PS4, very clean sine waves
// with extremely fast attack/decay. Almost "glass tapping" quality.
// ═══════════════════════════════════════════════════════════════════════════
const playstationProfile: SoundProfile = {
  id: "playstation",
  label: "PlayStation",
  description: "Crisp, glassy tones",
  sounds: {
    // Focus: sharp, high-pitched ping — very short
    focus: () => {
      tone(1200, 0.018, "sine", 0.001, 0.015, 0.5);
    },
    // Select: two crisp ascending tones
    select: () => {
      tone(988, 0.035, "sine", 0.002, 0.03, 0.7);    // B5
      delayed(28, () => tone(1319, 0.03, "sine", 0.002, 0.025, 0.6)); // E6
    },
    // Back: descending with slight resonance
    back: () => {
      tone(880, 0.03, "sine", 0.002, 0.025, 0.5);    // A5
      delayed(25, () => tone(659, 0.035, "sine", 0.002, 0.03, 0.45)); // E5
    },
    error: () => {
      tone(440, 0.035, "sine", 0.003, 0.03, 0.5);
      delayed(40, () => tone(392, 0.045, "sine", 0.003, 0.04, 0.5));
    },
    "overlay-open": () => {
      tone(659, 0.05, "sine", 0.003, 0.04, 0.45);
      delayed(35, () => tone(988, 0.04, "sine", 0.003, 0.035, 0.45));
    },
    "overlay-close": () => {
      tone(988, 0.04, "sine", 0.003, 0.035, 0.4);
      delayed(30, () => tone(659, 0.05, "sine", 0.003, 0.04, 0.4));
    },
  },
};

// ═══════════════════════════════════════════════════════════════════════════
// WII — Playful, musical, bouncy.
// Documented notes: Select = G4 (392Hz), Back = D3 (147Hz).
// The Wii uses clear musical intervals (4ths and 5ths) and sine waves
// with a slightly bubbly quality. Focus has a light click followed by
// a subtle "bloop" (rising pitch).
// ═══════════════════════════════════════════════════════════════════════════
const wiiProfile: SoundProfile = {
  id: "wii",
  label: "Wii",
  description: "Musical, bouncy notes",
  sounds: {
    // Focus: light click + subtle rising "bloop"
    focus: () => {
      noise(0.012, 4500, 2, "highpass", 0.3); // click
      sweep(300, 500, 0.04, "sine", 0.25);     // rising bloop
    },
    // Select: G4 (392Hz) — the iconic Wii confirmation zap
    select: () => {
      tone(392, 0.06, "sine", 0.002, 0.05, 0.8);    // G4
      delayed(25, () => tone(784, 0.04, "sine", 0.003, 0.035, 0.3)); // octave shimmer
    },
    // Back: D3 (147Hz) — documented as musical complement to select
    back: () => {
      tone(147, 0.08, "sine", 0.003, 0.07, 0.7);    // D3
    },
    error: () => {
      tone(262, 0.06, "triangle", 0.005, 0.05, 0.5); // C4
      delayed(60, () => tone(220, 0.07, "triangle", 0.005, 0.06, 0.5)); // A3
    },
    // Overlay: ascending musical sequence
    "overlay-open": () => {
      tone(392, 0.045, "sine", 0.003, 0.04, 0.5);    // G4
      delayed(40, () => tone(523, 0.04, "sine", 0.003, 0.035, 0.5));  // C5
      delayed(75, () => tone(659, 0.035, "sine", 0.003, 0.03, 0.45)); // E5
    },
    "overlay-close": () => {
      tone(523, 0.04, "sine", 0.003, 0.035, 0.4);    // C5
      delayed(40, () => tone(392, 0.05, "sine", 0.003, 0.04, 0.4));   // G4
    },
  },
};

// ═══════════════════════════════════════════════════════════════════════════
// PS2 — Smooth, flowing, warm electronic tones.
// The PS2 XMB had distinctive smooth transitions with soft whooshes
// and warm chime confirmations. Gentle sine tones with flowing quality.
// ═══════════════════════════════════════════════════════════════════════════
const ps2Profile: SoundProfile = {
  id: "ps2",
  label: "PS2",
  description: "Smooth, flowing tones",
  sounds: {
    // Focus: soft electronic whoosh + warm tone
    focus: () => {
      sweep(400, 600, 0.03, "sine", 0.5);
      tone(500, 0.015, "sine", 0.002, 0.012, 0.4);
    },
    // Select: warm chime with delayed rising tone
    select: () => {
      tone(600, 0.06, "sine", 0.004, 0.05, 0.7);
      delayed(50, () => tone(800, 0.05, "sine", 0.003, 0.04, 0.6));
    },
    // Back: gentle descending two-tone
    back: () => {
      tone(400, 0.04, "sine", 0.003, 0.035, 0.6);
      delayed(40, () => tone(300, 0.04, "sine", 0.003, 0.035, 0.5));
    },
    error: () => {
      tone(350, 0.08, "sine", 0.005, 0.07, 0.55);
      delayed(50, () => tone(300, 0.1, "sine", 0.005, 0.08, 0.5));
    },
    // Navigate: simple gentle tone
    "overlay-open": () => {
      tone(450, 0.02, "sine", 0.003, 0.015, 0.45);
      delayed(30, () => tone(550, 0.05, "sine", 0.004, 0.04, 0.5));
    },
    "overlay-close": () => {
      tone(550, 0.05, "sine", 0.004, 0.04, 0.45);
      delayed(35, () => tone(450, 0.06, "sine", 0.003, 0.05, 0.4));
    },
  },
};

// ═══════════════════════════════════════════════════════════════════════════
// DS — Soft, clicky, warm tones.
// The Nintendo DS had gentle, pen-tap-like sounds with warm character.
// Combines soft noise clicks with warm higher-pitched tones.
// ═══════════════════════════════════════════════════════════════════════════
const dsProfile: SoundProfile = {
  id: "ds",
  label: "DS",
  description: "Soft clicks, warm chimes",
  sounds: {
    // Focus: gentle pen-tap sound + warm tone
    focus: () => {
      noise(0.01, 5000, 3, "bandpass", 0.35);
      tone(1800, 0.008, "square", 0.001, 0.006, 0.3);
    },
    // Select: two warm ascending tones
    select: () => {
      tone(880, 0.03, "sine", 0.002, 0.025, 0.65);
      delayed(30, () => tone(1100, 0.025, "sine", 0.002, 0.02, 0.55));
    },
    // Back: warm triangle tone
    back: () => {
      tone(600, 0.025, "triangle", 0.003, 0.02, 0.55);
    },
    error: () => {
      tone(660, 0.06, "square", 0.003, 0.05, 0.5);
      delayed(45, () => tone(550, 0.07, "square", 0.003, 0.06, 0.45));
    },
    // Navigate: sharp tone
    "overlay-open": () => {
      tone(1500, 0.012, "square", 0.001, 0.01, 0.4);
      delayed(25, () => tone(1800, 0.02, "square", 0.002, 0.015, 0.4));
    },
    "overlay-close": () => {
      tone(1800, 0.02, "square", 0.002, 0.015, 0.35);
      delayed(30, () => tone(1500, 0.025, "square", 0.001, 0.02, 0.35));
    },
  },
};

// ═══════════════════════════════════════════════════════════════════════════
// DREAMCAST — Bright, digital, melodic.
// The Dreamcast dashboard had a clean, upbeat digital aesthetic.
// Uses bright sine tones with a slightly bell-like character,
// rising arpeggios for confirmations, and crisp digital clicks.
// The "swirl" bootup sound inspires the sweeping quality.
// ═══════════════════════════════════════════════════════════════════════════
const dreamcastProfile: SoundProfile = {
  id: "dreamcast",
  label: "Dreamcast",
  description: "Bright, digital, melodic",
  sounds: {
    // Focus: bright digital blip — crisp sine ping with subtle click
    focus: () => {
      noise(0.008, 6000, 3, "highpass", 0.2);
      tone(1100, 0.02, "sine", 0.001, 0.018, 0.45);
    },
    // Select: rising two-note arpeggio — upbeat Sega feel
    select: () => {
      tone(698, 0.04, "sine", 0.002, 0.035, 0.7);    // F5
      delayed(35, () => tone(880, 0.05, "sine", 0.003, 0.04, 0.65)); // A5
      delayed(65, () => tone(1047, 0.03, "sine", 0.002, 0.025, 0.4)); // C6 shimmer
    },
    // Back: descending digital sweep
    back: () => {
      sweep(900, 500, 0.05, "sine", 0.5);
      tone(500, 0.04, "sine", 0.003, 0.035, 0.45);
    },
    error: () => {
      tone(440, 0.04, "triangle", 0.003, 0.035, 0.5);
      delayed(50, () => tone(370, 0.06, "triangle", 0.003, 0.05, 0.5));
    },
    // Overlay: bright ascending sweep — evokes the Dreamcast swirl
    "overlay-open": () => {
      sweep(400, 900, 0.08, "sine", 0.4);
      delayed(40, () => tone(880, 0.06, "sine", 0.005, 0.05, 0.5));
      delayed(80, () => tone(1047, 0.04, "sine", 0.003, 0.035, 0.4));
    },
    "overlay-close": () => {
      sweep(900, 400, 0.08, "sine", 0.35);
      delayed(40, () => tone(523, 0.06, "sine", 0.005, 0.05, 0.4));
    },
  },
};

// ═══════════════════════════════════════════════════════════════════════════
// GAMECUBE — Playful, bouncy, with iconic startup cube sound feel.
// Bright, ascending three-note selection with cheerful sine waves.
// Descending back sound. Quick, playful focus blip.
// ═══════════════════════════════════════════════════════════════════════════
const gamecubeProfile: SoundProfile = {
  id: "gamecube",
  label: "GameCube",
  description: "Playful, bouncy tones",
  sounds: {
    // Focus: short bright blip with playful character
    focus: () => {
      tone(880, 0.018, "sine", 0.002, 0.015, 0.6);
      tone(1100, 0.015, "triangle", 0.001, 0.012, 0.4);
    },
    // Select: ascending cheerful three-note (G5→B5→D6)
    select: () => {
      tone(784, 0.035, "sine", 0.002, 0.03, 0.8);   // G5
      delayed(35, () => tone(988, 0.03, "sine", 0.002, 0.025, 0.75)); // B5
      delayed(65, () => tone(1175, 0.04, "sine", 0.003, 0.035, 0.7)); // D6
    },
    // Back: descending two-note
    back: () => {
      tone(988, 0.035, "sine", 0.002, 0.03, 0.65);  // B5
      delayed(35, () => tone(659, 0.04, "sine", 0.002, 0.035, 0.55)); // E5
    },
    error: () => {
      tone(550, 0.06, "triangle", 0.003, 0.05, 0.6);
      delayed(50, () => tone(440, 0.07, "triangle", 0.003, 0.06, 0.55));
    },
    "overlay-open": () => {
      tone(659, 0.04, "sine", 0.002, 0.035, 0.6);
      delayed(35, () => tone(880, 0.035, "sine", 0.002, 0.03, 0.6));
      delayed(65, () => tone(1047, 0.03, "sine", 0.003, 0.025, 0.5));
    },
    "overlay-close": () => {
      tone(1047, 0.03, "sine", 0.002, 0.025, 0.5);
      delayed(30, () => tone(659, 0.04, "sine", 0.002, 0.035, 0.5));
    },
  },
};

// ═══════════════════════════════════════════════════════════════════════════
// PSP — Smooth, modern, slightly futuristic.
// Soft electronic tick for focus, clean two-tone rise for select,
// gentle whoosh down for back. Longer envelopes, sine waves.
// ═══════════════════════════════════════════════════════════════════════════
const pspProfile: SoundProfile = {
  id: "psp",
  label: "PSP",
  description: "Smooth, modern electronic",
  sounds: {
    // Focus: soft electronic tick
    focus: () => {
      tone(750, 0.025, "sine", 0.003, 0.02, 0.5);
      noise(0.012, 4000, 2, "highpass", 0.2);
    },
    // Select: clean two-tone rise
    select: () => {
      tone(600, 0.045, "sine", 0.004, 0.04, 0.75);
      delayed(40, () => tone(850, 0.04, "sine", 0.003, 0.035, 0.65));
    },
    // Back: gentle whoosh down
    back: () => {
      sweep(850, 500, 0.055, "sine", 0.5);
      tone(500, 0.03, "sine", 0.003, 0.025, 0.45);
    },
    error: () => {
      tone(400, 0.07, "sine", 0.004, 0.06, 0.55);
      delayed(55, () => tone(350, 0.08, "sine", 0.004, 0.07, 0.5));
    },
    "overlay-open": () => {
      tone(500, 0.04, "sine", 0.003, 0.035, 0.5);
      delayed(40, () => tone(700, 0.05, "sine", 0.004, 0.04, 0.55));
    },
    "overlay-close": () => {
      tone(700, 0.05, "sine", 0.003, 0.04, 0.5);
      delayed(40, () => tone(500, 0.045, "sine", 0.003, 0.04, 0.45));
    },
  },
};

// ═══════════════════════════════════════════════════════════════════════════
// GAME BOY — 8-bit chiptune aesthetic.
// High-pitched square wave blip for focus, classic 8-bit confirm for select
// (C6→E6 square wave, fast), descending square buzz for back.
// Uses ONLY square waves for authentic 8-bit feel.
// ═══════════════════════════════════════════════════════════════════════════
const gameboyProfile: SoundProfile = {
  id: "gameboy",
  label: "Game Boy",
  description: "8-bit chiptune",
  sounds: {
    // Focus: high-pitched square wave blip (very short)
    focus: () => {
      tone(1400, 0.012, "square", 0.001, 0.01, 0.7);
    },
    // Select: classic 8-bit confirm (C6→E6 square wave, fast)
    select: () => {
      tone(1047, 0.025, "square", 0.001, 0.02, 0.8);   // C6
      delayed(25, () => tone(1319, 0.025, "square", 0.001, 0.02, 0.75)); // E6
    },
    // Back: descending square buzz
    back: () => {
      tone(880, 0.02, "square", 0.001, 0.018, 0.7);    // A5
      delayed(20, () => tone(659, 0.025, "square", 0.001, 0.02, 0.6)); // E5
    },
    error: () => {
      tone(784, 0.03, "square", 0.002, 0.025, 0.6);
      delayed(30, () => tone(659, 0.04, "square", 0.002, 0.035, 0.55));
    },
    "overlay-open": () => {
      tone(1047, 0.02, "square", 0.001, 0.018, 0.7);
      delayed(20, () => tone(1319, 0.02, "square", 0.001, 0.018, 0.65));
      delayed(40, () => tone(1568, 0.02, "square", 0.001, 0.018, 0.6)); // G6
    },
    "overlay-close": () => {
      tone(1568, 0.02, "square", 0.001, 0.018, 0.6);
      delayed(20, () => tone(1047, 0.025, "square", 0.001, 0.02, 0.55));
    },
  },
};

// ═══════════════════════════════════════════════════════════════════════════
// N64 — Warm, 3D-era, slightly echoey feel.
// Soft warm ping for focus, three ascending notes with slight reverb
// (layered two sine tones slightly detuned), descending warm tone for back.
// ═══════════════════════════════════════════════════════════════════════════
const n64Profile: SoundProfile = {
  id: "n64",
  label: "N64",
  description: "Warm, 3D-era tones",
  sounds: {
    // Focus: soft warm ping
    focus: () => {
      tone(700, 0.022, "sine", 0.002, 0.018, 0.6);
      tone(705, 0.022, "sine", 0.002, 0.018, 0.3); // detuned layer
    },
    // Select: three ascending notes with slight reverb feel
    select: () => {
      tone(523, 0.04, "sine", 0.003, 0.035, 0.75); // C5
      tone(525, 0.04, "sine", 0.003, 0.035, 0.2);  // detuned layer
      delayed(40, () => {
        tone(659, 0.035, "sine", 0.003, 0.03, 0.7); // E5
        tone(661, 0.035, "sine", 0.003, 0.03, 0.2);
      });
      delayed(75, () => {
        tone(784, 0.04, "sine", 0.003, 0.035, 0.65); // G5
        tone(786, 0.04, "sine", 0.003, 0.035, 0.2);
      });
    },
    // Back: descending warm tone
    back: () => {
      tone(659, 0.035, "sine", 0.003, 0.03, 0.6);
      tone(661, 0.035, "sine", 0.003, 0.03, 0.2);
      delayed(35, () => {
        tone(523, 0.04, "sine", 0.003, 0.035, 0.55);
        tone(525, 0.04, "sine", 0.003, 0.035, 0.2);
      });
    },
    error: () => {
      tone(440, 0.06, "sine", 0.003, 0.05, 0.6);
      delayed(50, () => tone(392, 0.07, "sine", 0.003, 0.06, 0.55));
    },
    "overlay-open": () => {
      tone(523, 0.045, "sine", 0.003, 0.04, 0.6);
      delayed(40, () => tone(659, 0.04, "sine", 0.003, 0.035, 0.6));
      delayed(75, () => tone(784, 0.035, "sine", 0.003, 0.03, 0.55));
    },
    "overlay-close": () => {
      tone(784, 0.04, "sine", 0.003, 0.035, 0.55);
      delayed(35, () => tone(523, 0.045, "sine", 0.003, 0.04, 0.5));
    },
  },
};

// ═══════════════════════════════════════════════════════════════════════════
// PS1 — Clean, minimal, digital.
// Very short high sine ping for focus (similar to PS5 but warmer),
// two clean ascending sine tones for select, single descending tone for back.
// ═══════════════════════════════════════════════════════════════════════════
const ps1Profile: SoundProfile = {
  id: "ps1",
  label: "PS1",
  description: "Clean, minimal digital",
  sounds: {
    // Focus: very short high sine ping (warm variant of PS5)
    focus: () => {
      tone(1000, 0.016, "sine", 0.001, 0.014, 0.55);
    },
    // Select: two clean ascending sine tones
    select: () => {
      tone(880, 0.035, "sine", 0.002, 0.03, 0.75);   // A5
      delayed(30, () => tone(1100, 0.03, "sine", 0.002, 0.025, 0.65)); // C#6
    },
    // Back: single descending tone
    back: () => {
      tone(880, 0.035, "sine", 0.002, 0.03, 0.6);
      delayed(30, () => tone(660, 0.04, "sine", 0.002, 0.035, 0.5));
    },
    error: () => {
      tone(550, 0.05, "sine", 0.003, 0.04, 0.6);
      delayed(45, () => tone(440, 0.06, "sine", 0.003, 0.05, 0.55));
    },
    "overlay-open": () => {
      tone(660, 0.04, "sine", 0.002, 0.035, 0.55);
      delayed(35, () => tone(880, 0.035, "sine", 0.002, 0.03, 0.55));
    },
    "overlay-close": () => {
      tone(880, 0.035, "sine", 0.002, 0.03, 0.5);
      delayed(30, () => tone(660, 0.04, "sine", 0.002, 0.035, 0.45));
    },
  },
};

// ═══════════════════════════════════════════════════════════════════════════
// SNES — 16-bit styled, bright.
// Bright square+sine combo blip for focus, triumphant two-note for select,
// quick descending note for back. Cheerful, upbeat character.
// ═══════════════════════════════════════════════════════════════════════════
const snesProfile: SoundProfile = {
  id: "snes",
  label: "SNES",
  description: "16-bit bright, triumphant",
  sounds: {
    // Focus: bright square+sine combo blip
    focus: () => {
      tone(1200, 0.014, "square", 0.001, 0.012, 0.5);
      tone(1100, 0.014, "sine", 0.001, 0.012, 0.4);
    },
    // Select: triumphant two-note (like getting an item)
    select: () => {
      tone(659, 0.04, "sine", 0.002, 0.035, 0.85);   // E5
      delayed(38, () => tone(988, 0.035, "sine", 0.002, 0.03, 0.75)); // B5
    },
    // Back: quick descending note
    back: () => {
      tone(784, 0.03, "square", 0.001, 0.025, 0.7);  // G5
      delayed(30, () => tone(523, 0.035, "square", 0.001, 0.03, 0.6)); // C5
    },
    error: () => {
      tone(330, 0.05, "square", 0.003, 0.04, 0.6);
      delayed(50, () => tone(262, 0.06, "square", 0.003, 0.05, 0.55));
    },
    "overlay-open": () => {
      tone(523, 0.03, "sine", 0.002, 0.025, 0.7);
      delayed(30, () => tone(659, 0.03, "sine", 0.002, 0.025, 0.65));
      delayed(60, () => tone(784, 0.03, "sine", 0.002, 0.025, 0.6));
    },
    "overlay-close": () => {
      tone(784, 0.03, "sine", 0.002, 0.025, 0.6);
      delayed(30, () => tone(523, 0.035, "sine", 0.002, 0.03, 0.55));
    },
  },
};

// ── Profile Registry ────────────────────────────────────────────────────────

export const soundProfiles: SoundProfile[] = [
  steamProfile,
  switchProfile,
  xboxProfile,
  playstationProfile,
  wiiProfile,
  ps2Profile,
  dsProfile,
  dreamcastProfile,
  gamecubeProfile,
  pspProfile,
  gameboyProfile,
  n64Profile,
  ps1Profile,
  snesProfile,
];

const profileMap = new Map<SoundProfileId, SoundProfile>(
  soundProfiles.map((p) => [p.id, p]),
);

function getActiveProfile(): SoundProfile {
  return profileMap.get(activeProfile) ?? steamProfile;
}

// ── Composable ──────────────────────────────────────────────────────────────

export function useBpAudio() {
  // Read preferences from localStorage on each call
  if (typeof localStorage !== "undefined") {
    const storedEnabled = localStorage.getItem("bpmAudioEnabled");
    if (storedEnabled !== null) {
      enabled = storedEnabled !== "false";
    }
    const storedProfile = localStorage.getItem(
      "bpmAudioProfile",
    ) as SoundProfileId | null;
    if (storedProfile && profileMap.has(storedProfile)) {
      activeProfile = storedProfile;
    }
  }

  return {
    /** Play a sound effect by name using the active profile. */
    play(sound: BpSound) {
      getActiveProfile().sounds[sound]?.();
    },

    /** Get the current enabled state. */
    get enabled() {
      return enabled;
    },

    /** Enable or disable audio feedback globally. */
    setEnabled(val: boolean) {
      enabled = val;
      if (typeof localStorage !== "undefined") {
        localStorage.setItem("bpmAudioEnabled", String(val));
      }
    },

    /** Get the active sound profile ID. */
    get profile() {
      return activeProfile;
    },

    /** Switch the active sound profile. */
    setProfile(id: SoundProfileId) {
      if (profileMap.has(id)) {
        activeProfile = id;
        if (typeof localStorage !== "undefined") {
          localStorage.setItem("bpmAudioProfile", id);
        }
      }
    },

    /** Set the master volume (0-1). */
    setVolume(vol: number) {
      ctx();
      if (masterGain) {
        masterGain.gain.value = Math.max(0, Math.min(1, vol));
      }
    },

    /** Preview a specific sound with the given profile. */
    preview(profileId: SoundProfileId, sound: BpSound = "select") {
      const profile = profileMap.get(profileId);
      if (profile && enabled) {
        profile.sounds[sound]();
      }
    },
  };
}
