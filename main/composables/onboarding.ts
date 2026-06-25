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

import { isDevEnabled } from "./dev-mode";

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

// Steps gated behind dev mode. When dev mode is off these are skipped by
// `activeSteps()`, so `nextRoute`/`prevRoute`/`stepNumber`/`total` all see
// a wizard that's one entry shorter and the user never lands on the page.
const DEV_ONLY_STEPS: ReadonlySet<WizardStep> = new Set(["saves"]);

/**
 * Returns the current sequence of wizard steps, filtered by dev mode.
 * Re-evaluated on each call — `isDevEnabled()` reads from a module-level
 * boolean, not a Vue ref, so toggling dev mode mid-wizard requires a
 * page revisit to take effect (an acceptable edge case).
 */
function activeSteps(): readonly WizardStep[] {
  if (isDevEnabled()) return WIZARD_STEPS;
  return WIZARD_STEPS.filter((s) => !DEV_ONLY_STEPS.has(s));
}

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
    const steps = activeSteps();
    const idx = steps.indexOf(current);
    if (idx < 0 || idx >= steps.length - 1) return "/bigpicture";
    return stepPath(steps[idx + 1]);
  }

  function prevRoute(current: WizardStep): string | null {
    const steps = activeSteps();
    const idx = steps.indexOf(current);
    if (idx <= 0) return null;
    return stepPath(steps[idx - 1]);
  }

  function stepNumber(current: WizardStep): number {
    const steps = activeSteps();
    const idx = steps.indexOf(current);
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
    get total() {
      return activeSteps().length;
    },
  };
}
