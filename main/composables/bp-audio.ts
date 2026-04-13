/**
 * BPM Audio Feedback System
 *
 * Generates UI sounds programmatically using the Web Audio API.
 * No external .wav files needed — all sounds are synthesized.
 * User preference is persisted to localStorage.
 */

// ── Audio Context ────────────────────────────────────────────────────────────

let audioCtx: AudioContext | null = null;
let gainNode: GainNode | null = null;
let enabled = true;

function ensureContext() {
  if (!audioCtx) {
    audioCtx = new AudioContext();
    gainNode = audioCtx.createGain();
    gainNode.gain.value = 0.15; // subtle volume
    gainNode.connect(audioCtx.destination);
  }
  if (audioCtx.state === "suspended") {
    audioCtx.resume();
  }
  return { ctx: audioCtx, gain: gainNode! };
}

// ── Sound Generation ─────────────────────────────────────────────────────────

/**
 * Play a synthesized tone using an oscillator.
 *
 * @param frequency - Hz (e.g. 440 = A4)
 * @param duration - seconds
 * @param type - waveform (sine, square, sawtooth, triangle)
 * @param attack - envelope rise time in seconds
 * @param decay - envelope fall time in seconds (defaults to duration)
 */
function playTone(
  frequency: number,
  duration: number,
  type: OscillatorType = "sine",
  attack = 0.005,
  decay?: number,
) {
  if (!enabled) return;

  const { ctx, gain } = ensureContext();
  const osc = ctx.createOscillator();
  const env = ctx.createGain();

  osc.type = type;
  osc.frequency.value = frequency;

  // Envelope: rise to 1 over attack, then fall to 0 over decay
  env.gain.setValueAtTime(0, ctx.currentTime);
  env.gain.linearRampToValueAtTime(1, ctx.currentTime + attack);
  env.gain.linearRampToValueAtTime(0, ctx.currentTime + (decay ?? duration));

  osc.connect(env);
  env.connect(gain);

  osc.start(ctx.currentTime);
  osc.stop(ctx.currentTime + duration);
}

// ── Sound Library ────────────────────────────────────────────────────────────

export type BpSound =
  | "focus"
  | "select"
  | "back"
  | "error"
  | "overlay-open"
  | "overlay-close";

const sounds: Record<BpSound, () => void> = {
  /**
   * Navigation focus: subtle mid-range beep
   */
  focus: () => playTone(660, 0.04, "sine", 0.003, 0.03),

  /**
   * Selection/confirmation: mid-high pitch with slight attack
   */
  select: () => playTone(880, 0.06, "triangle", 0.005, 0.05),

  /**
   * Back/exit: slightly lower pitch than select
   */
  back: () => playTone(440, 0.06, "triangle", 0.005, 0.05),

  /**
   * Error/boundary: low pitch sawtooth buzz
   */
  error: () => playTone(220, 0.1, "sawtooth", 0.01, 0.08),

  /**
   * Overlay opening: two-note ascending sequence
   */
  "overlay-open": () => {
    playTone(440, 0.1, "sine", 0.01, 0.08);
    setTimeout(() => playTone(660, 0.08, "sine", 0.01, 0.06), 50);
  },

  /**
   * Overlay closing: two-note descending sequence
   */
  "overlay-close": () => {
    playTone(660, 0.08, "sine", 0.005, 0.06);
    setTimeout(() => playTone(440, 0.1, "sine", 0.01, 0.08), 40);
  },
};

// ── Composable ───────────────────────────────────────────────────────────────

export function useBpAudio() {
  // Read preference from localStorage on first call
  const stored =
    typeof localStorage !== "undefined"
      ? localStorage.getItem("bpmAudioEnabled")
      : null;
  if (stored !== null) {
    enabled = stored !== "false";
  }

  return {
    /**
     * Play a sound effect by name.
     */
    play(sound: BpSound) {
      sounds[sound]?.();
    },

    /**
     * Get the current enabled state.
     */
    get enabled() {
      return enabled;
    },

    /**
     * Enable or disable audio feedback globally.
     * Persists to localStorage.
     */
    setEnabled(val: boolean) {
      enabled = val;
      if (typeof localStorage !== "undefined") {
        localStorage.setItem("bpmAudioEnabled", String(val));
      }
    },

    /**
     * Set the master volume (0-1).
     */
    setVolume(vol: number) {
      ensureContext();
      if (gainNode) {
        gainNode.gain.value = Math.max(0, Math.min(1, vol));
      }
    },
  };
}
