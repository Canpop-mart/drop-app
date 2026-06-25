/**
 * Render user-authored markdown (game descriptions, news articles) to SAFE HTML
 * for a `v-html` sink — the single client-side renderer for all of them.
 *
 * Pipeline:
 *   1. micromark  — markdown → HTML (full CommonMark, replacing the old hand-
 *      rolled regex renderer that had link/attribute-injection holes).
 *   2. sanitizeHtml (DOMPurify) — strip <script>, event handlers, javascript:
 *      URLs, raw embedded HTML, etc. micromark passes raw HTML through untouched,
 *      so rendering alone is NOT safe in a Tauri webview that holds
 *      invoke()/__TAURI__.
 *   3. rewriteDescriptionImages — point relative `<img src="/...">` at the
 *      server:// proxy. Runs LAST so the proxy URL scheme isn't stripped by the
 *      sanitizer.
 *
 * Pure function, safe to import anywhere. Returns "" on empty input or a
 * micromark parse error (the sink shows an empty body rather than throwing).
 */

import { micromark } from "micromark";
import { sanitizeHtml } from "./sanitize";
import { rewriteDescriptionImages } from "./use-server-fetch";

/** Render a markdown string to sanitized HTML (ready for `v-html`). */
export function renderMarkdown(md: string): string {
  if (!md) return "";
  let html: string;
  try {
    html = micromark(md);
  } catch {
    return "";
  }
  return rewriteDescriptionImages(sanitizeHtml(html));
}
