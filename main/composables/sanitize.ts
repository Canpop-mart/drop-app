import DOMPurify from "dompurify";

/**
 * Sanitize HTML for use with `v-html` in the desktop client.
 *
 * The client renders user-authored markdown (game descriptions, news) inside a
 * Tauri webview that holds `invoke()` / `__TAURI__`, so any `v-html` sink derived
 * from author content MUST pass through here first — micromark renders markdown
 * but lets raw embedded HTML through untouched. Mirrors the server's
 * `composables/sanitize.ts`. The app is SPA-only (`ssr: false`), so the plain
 * browser build of DOMPurify is enough (no jsdom).
 *
 * Conservative config:
 *   - USE_PROFILES.html — DOMPurify's curated HTML profile (drops <script>,
 *     event handlers, javascript: URIs, <iframe>/<object>/<embed>, etc.).
 *   - ALLOW_DATA_ATTR: false — no data-* attrs (can ferry payload into scripts).
 *   - FORBID_TAGS: ['style'] — no author-controlled stylesheets (CSS exfil).
 */
export function sanitizeHtml(input: string): string {
  if (!input) return "";
  return DOMPurify.sanitize(input, {
    USE_PROFILES: { html: true },
    ALLOW_DATA_ATTR: false,
    FORBID_TAGS: ["style"],
  });
}
