/**
 * Lightweight markdown → HTML renderer for game descriptions.
 *
 * Handles a deliberately small subset — headings, bold, italic, links,
 * images, lists, horizontal rules, paragraphs — which is all Drop game
 * descriptions use. Not a general-purpose markdown engine; it does not
 * support tables, code fences, nested lists, etc.
 *
 * Extracted verbatim from `pages/bigpicture/library/[id].vue` so the same
 * renderer can be reused by the desktop game-detail page if it ever needs
 * rich descriptions. Pure function — safe to import anywhere.
 *
 * Relative image URLs (those starting with `/`) are rewritten through the
 * `server://` proxy via `serverUrl()` so media loads with auth headers.
 */

import { serverUrl } from "./use-server-fetch";

/** Render a markdown string to an HTML string (already entity-escaped). */
export function renderMarkdown(md: string): string {
  let html = md
    // Escape HTML entities first so user content can't inject markup.
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    // Headings (### / ## / #)
    .replace(/^### (.+)$/gm, "<h3>$1</h3>")
    .replace(/^## (.+)$/gm, "<h2>$1</h2>")
    .replace(/^# (.+)$/gm, "<h1>$1</h1>")
    // Bold (**text** or __text__)
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/__(.+?)__/g, "<strong>$1</strong>")
    // Italic (*text* or _text_)
    .replace(/\*(.+?)\*/g, "<em>$1</em>")
    .replace(/_(.+?)_/g, "<em>$1</em>")
    // Images ![alt](url) — must come before links so ![...] doesn't match [...]
    .replace(
      /!\[([^\]]*)\]\(([^)]+)\)/g,
      (_m: string, alt: string, url: string) => {
        // Rewrite relative URLs (starting with /) through the server proxy
        const src = url.startsWith("/") ? serverUrl(url.slice(1)) : url;
        return `<img src="${src}" alt="${alt}" class="rounded-lg max-w-full my-2" loading="lazy" />`;
      },
    )
    // Links [text](url)
    .replace(
      /\[([^\]]+)\]\(([^)]+)\)/g,
      '<a href="$2" target="_blank" rel="noopener" class="text-blue-400 hover:underline">$1</a>',
    )
    // Unordered lists (- item or * item)
    .replace(/^[\-\*] (.+)$/gm, "<li>$1</li>")
    // Horizontal rules (--- or ***)
    .replace(/^[\-\*]{3,}$/gm, '<hr class="border-zinc-700 my-4">')
    // Paragraphs (double newlines)
    .replace(/\n\n+/g, "</p><p>")
    // Single newlines → line breaks
    .replace(/\n/g, "<br>");

  // Wrap consecutive list items in a <ul>
  html = html.replace(
    /(<li>.*?<\/li>)+/gs,
    '<ul class="list-disc pl-6 space-y-1">$&</ul>',
  );

  return `<p>${html}</p>`;
}
