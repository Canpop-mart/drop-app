import { convertFileSrc } from "@tauri-apps/api/core";

/**
 * Constructs a URL that goes through Tauri's server:// protocol handler,
 * which proxies requests to the Drop server with authentication.
 */
export function serverUrl(path: string): string {
  // convertFileSrc encodes the URI, so we use a placeholder and replace it
  return convertFileSrc("dummyvalue", "server").replace("dummyvalue", path);
}

/**
 * Rewrites image src attributes in HTML to go through the server:// protocol.
 * This handles description HTML that contains relative or absolute server URLs.
 */
export function rewriteDescriptionImages(html: string): string {
  // Replace img src that start with / (relative to server)
  return html.replace(
    /(<img\s[^>]*src=")(\/)([^"]*")/gi,
    (_match, prefix, _slash, rest) => {
      const rewrittenUrl = serverUrl(rest.slice(0, -1));
      return `${prefix}${rewrittenUrl}"`;
    },
  );
}
