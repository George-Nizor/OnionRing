// Collections, shared by the library and the game page (kept out of the main store on purpose).
import { listen } from "@tauri-apps/api/event";
import { api, type Collection } from "$lib/api";

class Collections {
  list = $state<Collection[]>([]);
  loaded = $state(false);
  #started = false;

  /** Load once and follow "collections-changed" for the life of the app. Safe to call often. */
  start() {
    if (this.#started) return;
    this.#started = true;
    this.load();
    listen("collections-changed", () => this.load()).catch(() => {});
  }

  async load() {
    try {
      this.list = await api.collections();
    } catch {
      /* keep what we had */
    }
    this.loaded = true;
  }

  byId(id: number) {
    return this.list.find((c) => c.id === id);
  }
}

export const collections = new Collections();

/** Library filter value for a collection. */
export const collectionFilter = (id: number) => `c:${id}`;
export const filterCollection = (f: string) => (f.startsWith("c:") ? Number(f.slice(2)) : null);
