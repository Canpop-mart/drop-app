/*
Tidy-ups applied to stored Markdown before it is rendered. Kept free of Vue,
Nuxt and the DOM so node --test can run it: main/tests/markdown-cleanup.test.ts.
*/

/**
 * Drop lines that hold nothing but a bold marker (`**` or `__`).
 *
 * The server's Steam importer turns `<b>...</b>` into `**...**`. Steam pages
 * often wrap an image or a heading in bold; the importer moves those onto
 * their own lines, which strands the two `**` on lines of their own. CommonMark
 * can't pair emphasis across blocks, so they render as literal asterisks.
 * Descriptions imported before the importer was fixed still carry them.
 *
 * A line of three or more `*` is a horizontal rule and is left alone, and
 * code blocks are never touched.
 */
export function dropOrphanEmphasis(md: string): string {
  // Lines inside a fenced code block are code, not formatting, and are kept.
  // Indented code blocks are kept too: only up to 3 leading spaces match.
  let fence: string | null = null;
  return md
    .split(/\r?\n/)
    .filter((line) => {
      const opener = /^ {0,3}(`{3,}|~{3,})/.exec(line);
      if (opener) {
        const marker = opener[1]!;
        if (fence === null) fence = marker;
        else if (marker[0] === fence[0] && marker.length >= fence.length)
          fence = null;
        return true;
      }
      if (fence !== null) return true;
      return !/^ {0,3}(\*\*|__)[ \t]*$/.test(line);
    })
    .join("\n");
}
