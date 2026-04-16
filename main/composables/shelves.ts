/**
 * Shelves composable — manages user collections (shelves) for the BPM library.
 * Wraps the server collection API via server:// protocol.
 */

import { ref } from "vue";
import { serverUrl } from "./use-server-fetch";

export interface Shelf {
  id: string;
  name: string;
  isPublic?: boolean;
  entries: Array<{
    gameId: string;
    game: {
      id: string;
      mName: string;
      mCoverObjectId: string | null;
      mIconObjectId: string | null;
    };
  }>;
}

const shelves = ref<Shelf[]>([]);
const loading = ref(false);

async function apiFetch<T>(path: string, options?: RequestInit): Promise<T> {
  const resp = await fetch(serverUrl(path), {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...(options?.headers ?? {}),
    },
  });
  if (!resp.ok) {
    const text = await resp.text().catch(() => "");
    throw new Error(`API error ${resp.status}: ${text}`);
  }
  const text = await resp.text();
  return text ? JSON.parse(text) : {};
}

export function useShelves() {
  async function fetchShelves() {
    loading.value = true;
    try {
      const data = await apiFetch<Shelf[]>("api/v1/collection");
      // Filter out the default collection (user's main library)
      shelves.value = data.filter((s) => s.name !== "default");
    } catch (e) {
      console.error("[SHELVES] Failed to fetch shelves:", e);
    } finally {
      loading.value = false;
    }
  }

  async function createShelf(name: string): Promise<Shelf | null> {
    try {
      const result = await apiFetch<Shelf>("api/v1/collection", {
        method: "POST",
        body: JSON.stringify({ name }),
      });
      await fetchShelves(); // Refresh
      return result;
    } catch (e) {
      console.error("[SHELVES] Failed to create shelf:", e);
      return null;
    }
  }

  async function deleteShelf(shelfId: string) {
    try {
      await apiFetch(`api/v1/collection/${shelfId}`, {
        method: "DELETE",
      });
      shelves.value = shelves.value.filter((s) => s.id !== shelfId);
    } catch (e) {
      console.error("[SHELVES] Failed to delete shelf:", e);
    }
  }

  async function addToShelf(shelfId: string, gameId: string) {
    try {
      await apiFetch(`api/v1/collection/${shelfId}/entry`, {
        method: "POST",
        body: JSON.stringify({ id: gameId }),
      });
      await fetchShelves(); // Refresh to get updated entries
    } catch (e) {
      console.error("[SHELVES] Failed to add game to shelf:", e);
    }
  }

  async function removeFromShelf(shelfId: string, gameId: string) {
    try {
      await apiFetch(`api/v1/collection/${shelfId}/entry`, {
        method: "DELETE",
        body: JSON.stringify({ id: gameId }),
      });
      // Optimistic update
      const shelf = shelves.value.find((s) => s.id === shelfId);
      if (shelf) {
        shelf.entries = shelf.entries.filter((e) => e.gameId !== gameId);
      }
    } catch (e) {
      console.error("[SHELVES] Failed to remove game from shelf:", e);
    }
  }

  async function toggleShelfVisibility(shelfId: string, isPublic: boolean) {
    try {
      await apiFetch(`api/v1/collection/${shelfId}/visibility`, {
        method: "PUT",
        body: JSON.stringify({ isPublic }),
      });
      await fetchShelves(); // Refresh
    } catch (e) {
      console.error("[SHELVES] Failed to toggle visibility:", e);
    }
  }

  function getShelvesForGame(gameId: string): Shelf[] {
    return shelves.value.filter((s) =>
      s.entries.some((e) => e.gameId === gameId),
    );
  }

  return {
    shelves,
    loading,
    fetchShelves,
    createShelf,
    deleteShelf,
    addToShelf,
    removeFromShelf,
    toggleShelfVisibility,
    getShelvesForGame,
  };
}
