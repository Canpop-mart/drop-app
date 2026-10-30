/*
State for the Store's Addons tab, shared by the desktop Store
(components/StoreAddonsTab.vue) and Big Picture
(components/bigpicture/BpmStoreAddons.vue). The pure parts live in
addon-shop-logic.ts.

Each call makes its own state; the two tabs are never mounted together.
A row only changes once the server has agreed; a failed add or remove leaves
it as it was and says so on that row. One request at a time per instance.
*/
import {
  parseShopList,
  withAdded,
  type ShopAddon,
  type ShopStatus,
} from "./addon-shop-logic";
import { useServerApi } from "./use-server-api";

export type ShopActionError = { name: string; kind: "add" | "remove" };

export function useAddonShop() {
  const api = useServerApi();

  const status = ref<ShopStatus>("loading");
  const addons = ref<ShopAddon[]>([]);
  /** Name of the addon with a request in flight. */
  const busy = ref<string | null>(null);
  /** Name of the addon whose "remove this?" rows are open. */
  const confirming = ref<string | null>(null);
  const actionError = ref<ShopActionError | null>(null);

  // A reload that finishes after a newer one must not overwrite it.
  let loadSeq = 0;

  async function load() {
    const seq = ++loadSeq;
    status.value = "loading";
    try {
      const list = parseShopList(await api.addons.list());
      if (seq !== loadSeq) return;
      addons.value = list;
      status.value = "ready";
    } catch (e) {
      if (seq !== loadSeq) return;
      console.error("[ADDONS] could not load the addon list:", e);
      status.value = "error";
    }
  }

  /** Resolves true when the server applied the change. */
  async function setAdded(name: string, add: boolean): Promise<boolean> {
    if (busy.value) return false;
    busy.value = name;
    actionError.value = null;
    try {
      if (add) await api.addons.add(name);
      else await api.addons.remove(name);
      addons.value = withAdded(addons.value, name, add);
      confirming.value = null;
      return true;
    } catch (e) {
      console.error(`[ADDONS] could not ${add ? "add" : "remove"} ${name}:`, e);
      actionError.value = { name, kind: add ? "add" : "remove" };
      return false;
    } finally {
      busy.value = null;
    }
  }

  function askRemove(name: string) {
    if (busy.value) return;
    actionError.value = null;
    confirming.value = name;
  }

  function cancelRemove() {
    confirming.value = null;
  }

  return {
    status,
    addons,
    busy,
    confirming,
    actionError,
    load,
    add: (name: string) => setAdded(name, true),
    remove: (name: string) => setAdded(name, false),
    askRemove,
    cancelRemove,
  };
}
