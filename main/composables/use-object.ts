import { convertFileSrc } from "@tauri-apps/api/core";
import { serverUrl } from "~/composables/use-server-fetch";

export const useObject = (id: string) => {
  return convertFileSrc(id, "object");
};

/**
 * Image URL for an object id, for anything that renders game or profile art.
 *
 * Always prefer `object://`. It is the only image path with a cache in front of
 * it (600-entry memory cache plus a disk cache, both in the `remote` crate) and
 * it is concurrency-capped, so a library or Community render reuses art instead
 * of opening a fresh socket per tile. `serverUrl("api/v1/object/...")` routes
 * through `server://`, which has no cache at all, and 38 call sites were using
 * it — that was most of the request storm.
 *
 * The `server://` path survives only as a dev workaround: the `object://`
 * handler is not reachable from the Vite dev origin. It is gated on
 * `import.meta.dev` so it stops shipping to users.
 */
export function objectImageUrl(id?: string | null): string {
  if (!id) return "";
  if (import.meta.dev) return serverUrl(`api/v1/object/${id}`);
  return useObject(id);
}
