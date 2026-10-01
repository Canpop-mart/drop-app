import { computed, toValue, type MaybeRefOrGetter } from "vue";
import { accentVars, resolveAccentHex } from "./profile-themes";

/**
 * Profile theming for the desktop profile pages: the accent colour that
 * threads through a profile page (name, stats, shelves, cards, buttons,
 * banner).
 *
 * The presets and the rules for a stored `profileTheme` (a preset id or a
 * custom `#rrggbb`) live in profile-themes.ts, shared with Big Picture and the
 * web pages. `useProfileTheme` resolves the stored value to a set of CSS
 * custom properties bound once on a profile-root wrapper; the children then
 * reference them with static Tailwind arbitrary utilities
 * (`text-[color:var(--accent)]`, `ring-[color:var(--accent-border)]`, ...).
 * Never build a class string from the hex: Tailwind's JIT would purge it.
 */

/**
 * Reactive theme for a profile page. Pass the user's `profileTheme` (a ref or
 * getter). Bind `vars` on a root wrapper's `:style`; read `accent` for JS-side
 * needs (e.g. the live edit preview).
 */
export function useProfileTheme(
  theme: MaybeRefOrGetter<string | undefined | null>,
) {
  const accent = computed(() => resolveAccentHex(toValue(theme)));
  const vars = computed(() => accentVars(accent.value));
  return { accent, vars };
}
