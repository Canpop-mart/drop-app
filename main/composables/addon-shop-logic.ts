/*
Pure logic for the Store's Addons tab, shared by the desktop Store and Big
Picture (composables/addon-shop.ts wires it to the server). Kept free of Vue
and Nuxt so node --test can run it: main/tests/addon-shop.test.ts.

An addon is a service the server admin runs; a player adds or removes it for
themselves. Today that only changes this tab; addon panels elsewhere in Drop
are planned to check it.
*/

export type ShopAddon = {
  name: string;
  title: string;
  summary: string;
  description: string;
  site: string | null;
  added: boolean;
};

export type ShopStatus = "loading" | "error" | "ready";

/** What a row offers, from the shop's shared state. */
export type ShopRowView = {
  /** The row's main button: add it, or start removing it. */
  primary: "add" | "remove";
  /** The inline "remove this?" rows are showing. */
  confirming: boolean;
  /** A request for this row is in flight. */
  busy: boolean;
  /** Any request is in flight, so every row's buttons wait. */
  locked: boolean;
};

const NAME = /^[a-z0-9][a-z0-9-]{0,31}$/;

function str(v: unknown): string {
  return typeof v === "string" ? v : "";
}

/**
 * The server's list, checked. A malformed entry is dropped rather than
 * failing the whole tab; anything that is not a list at all is an error the
 * caller shows as "could not load", never as "no addons".
 */
export function parseShopList(raw: unknown): ShopAddon[] {
  if (!Array.isArray(raw)) throw new Error("addon list is not an array");
  const out: ShopAddon[] = [];
  const seen = new Set<string>();
  for (const entry of raw) {
    if (!entry || typeof entry !== "object") continue;
    const o = entry as Record<string, unknown>;
    const name = str(o.name);
    if (!NAME.test(name) || seen.has(name)) continue;
    seen.add(name);
    const site = str(o.site);
    out.push({
      name,
      title: str(o.title) || name,
      summary: str(o.summary),
      description: str(o.description),
      // Only a web link is ever opened from the shop.
      site: /^https?:\/\//i.test(site) ? site : null,
      added: o.added === true,
    });
  }
  return out;
}

/** A copy of the list with one addon's state changed. */
export function withAdded(
  list: readonly ShopAddon[],
  name: string,
  added: boolean,
): ShopAddon[] {
  return list.map((a) => (a.name === name ? { ...a, added } : a));
}

export function rowView(
  addon: ShopAddon,
  confirming: string | null,
  busy: string | null,
): ShopRowView {
  return {
    primary: addon.added ? "remove" : "add",
    confirming: addon.added && confirming === addon.name,
    busy: busy === addon.name,
    locked: busy !== null,
  };
}
