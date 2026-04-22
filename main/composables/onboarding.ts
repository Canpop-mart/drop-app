/**
 * BPM Onboarding Wizard state.
 *
 * Purely user-triggered — there is no auto-redirect. The user runs it from
 * Settings → Help → "Run setup wizard". We only track completion so the
 * settings entry can surface a "completed" badge / revisit hint.
 *
 * All state is localStorage-backed; re-running is idempotent because each
 * step performs its own "already configured?" check and offers to skip.
 */

const COMPLETED_AT_KEY = "drop:wizard_completed_at";
const STEP_SEEN_PREFIX = "drop:wizard_step_seen:";

export const WIZARD_STEPS = [
  "welcome",
  "profile",
  "storage",
  "platform",
  "saves",
  "retroachievements",
  "navigation",
  "warnings",
  "done",
] as const;

export type WizardStep = (typeof WIZARD_STEPS)[number];

export function useOnboarding() {
  function safeStorage(): Storage | null {
    return typeof localStorage !== "undefined" ? localStorage : null;
  }

  function hasCompleted(): boolean {
    return !!safeStorage()?.getItem(COMPLETED_AT_KEY);
  }

  function completedAt(): Date | null {
    const raw = safeStorage()?.getItem(COMPLETED_AT_KEY);
    if (!raw) return null;
    const d = new Date(raw);
    return isNaN(d.getTime()) ? null : d;
  }

  function markCompleted(): void {
    safeStorage()?.setItem(COMPLETED_AT_KEY, new Date().toISOString());
  }

  function markStepSeen(step: WizardStep): void {
    safeStorage()?.setItem(STEP_SEEN_PREFIX + step, "true");
  }

  function stepSeen(step: WizardStep): boolean {
    return safeStorage()?.getItem(STEP_SEEN_PREFIX + step) === "true";
  }

  function reset(): void {
    const s = safeStorage();
    if (!s) return;
    s.removeItem(COMPLETED_AT_KEY);
    for (const step of WIZARD_STEPS) {
      s.removeItem(STEP_SEEN_PREFIX + step);
    }
  }

  /** First step lives at /welcome (index.vue); subsequent at /welcome/{step}. */
  function stepPath(step: WizardStep): string {
    return step === WIZARD_STEPS[0] ? "/welcome" : "/welcome/" + step;
  }

  function nextRoute(current: WizardStep): string {
    const idx = WIZARD_STEPS.indexOf(current);
    if (idx < 0 || idx >= WIZARD_STEPS.length - 1) return "/bigpicture";
    return stepPath(WIZARD_STEPS[idx + 1]);
  }

  function prevRoute(current: WizardStep): string | null {
    const idx = WIZARD_STEPS.indexOf(current);
    if (idx <= 0) return null;
    return stepPath(WIZARD_STEPS[idx - 1]);
  }

  function stepNumber(current: WizardStep): number {
    const idx = WIZARD_STEPS.indexOf(current);
    return idx < 0 ? 1 : idx + 1;
  }

  return {
    hasCompleted,
    completedAt,
    markCompleted,
    markStepSeen,
    stepSeen,
    reset,
    nextRoute,
    prevRoute,
    stepPath,
    stepNumber,
    total: WIZARD_STEPS.length,
  };
}
