import { listen } from "@tauri-apps/api/event";
import { api, type Bootstrap, type Game, type Platform, type Progress } from "./api";
import { norm } from "./format";

export type Section = "library" | "ports" | "emulators" | "storage" | "settings";
export type Filter = "all" | "recent" | "favorites" | "ports" | `platform:${string}`;
export type Sort = "title" | "recent" | "added" | "playtime" | "size";

export interface Toast {
  id: number;
  text: string;
  kind: "info" | "ok" | "error";
}

const SORTS: Record<Sort, (a: Game, b: Game) => number> = {
  title: (a, b) => a.title.localeCompare(b.title),
  recent: (a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || a.title.localeCompare(b.title),
  added: (a, b) => b.addedAt - a.addedAt || a.title.localeCompare(b.title),
  playtime: (a, b) => b.playtime - a.playtime || a.title.localeCompare(b.title),
  size: (a, b) => b.size - a.size,
};

function load<T>(k: string, fallback: T): T {
  try {
    const v = localStorage.getItem(`onionring.${k}`);
    return v ? (JSON.parse(v) as T) : fallback;
  } catch {
    return fallback;
  }
}
function save(k: string, v: unknown) {
  try {
    localStorage.setItem(`onionring.${k}`, JSON.stringify(v));
  } catch {
    /* storage unavailable: preferences just won't persist */
  }
}

class AppStore {
  boot = $state<Bootstrap | null>(null);
  games = $state<Game[]>([]);
  loaded = $state(false);
  section = $state<Section>("library");
  filter = $state<Filter>(load("filter", "all"));
  sort = $state<Sort>(load("sort", "title"));
  query = $state("");
  selectedId = $state<number | null>(load("selected", null));
  progress = $state<Progress | null>(null);
  running = $state<number[]>([]);
  toasts = $state<Toast[]>([]);
  showHidden = $state(false);
  view = $state<"shelf" | "grid">(load("view", "shelf"));
  /** Accent colours for ports (from the catalog), keyed by app id. */
  accents = $state<Record<string, string>>({});

  accent(g: Game): string {
    return (g.appId && this.accents[g.appId]) || this.platforms.get(g.platform)?.color || "#ff5d7a";
  }

  setView(v: "shelf" | "grid") {
    this.view = v;
    save("view", v);
  }

  platforms = $derived(new Map<string, Platform>((this.boot?.platforms ?? []).map((p) => [p.id, p])));

  counts = $derived.by(() => {
    const c = new Map<string, number>();
    for (const g of this.games) if (!g.hidden) c.set(g.platform, (c.get(g.platform) ?? 0) + 1);
    return c;
  });

  visible = $derived.by(() => {
    const q = norm(this.query);
    let list = this.games.filter((g) => this.showHidden || !g.hidden);
    const f = this.filter;
    if (f === "recent") list = list.filter((g) => g.lastPlayed);
    else if (f === "favorites") list = list.filter((g) => g.favorite);
    else if (f === "ports") list = list.filter((g) => g.kind === "port");
    else if (f.startsWith("platform:")) list = list.filter((g) => g.platform === f.slice(9));
    if (q) list = list.filter((g) => norm(g.title).includes(q) || (g.serial ?? "").toLowerCase().includes(q));
    const sort = f === "recent" ? SORTS.recent : SORTS[this.sort];
    return [...list].sort(sort);
  });

  selected = $derived(this.games.find((g) => g.id === this.selectedId) ?? this.visible[0] ?? null);

  recent = $derived(
    this.games.filter((g) => g.lastPlayed && !g.hidden).sort(SORTS.recent).slice(0, 8),
  );

  async init() {
    this.boot = await api.bootstrap();
    await listen<Progress>("progress", (e) => {
      this.progress = e.payload.stage === "idle" ? null : e.payload;
    });
    await listen("library-changed", () => this.reload());
    await listen<number>("game-started", (e) => {
      this.running = [...this.running, e.payload];
    });
    await listen<{ id: number; seconds: number }>("game-ended", (e) => {
      this.running = this.running.filter((r) => r !== e.payload.id);
    });
    if (this.boot.dataRoot) {
      await this.reload();
      api.catalog()
        .then((c) => (this.accents = Object.fromEntries(c.ports.filter((p) => p.accent).map((p) => [p.id, p.accent!]))))
        .catch(() => {});
    }
    this.loaded = true;
  }

  async reload() {
    try {
      this.games = await api.games();
    } catch (e) {
      this.toast(String(e), "error");
    }
  }

  select(id: number | null) {
    this.selectedId = id;
    save("selected", id);
  }

  setFilter(f: Filter) {
    this.section = "library";
    this.filter = f;
    save("filter", f);
  }

  setSort(s: Sort) {
    this.sort = s;
    save("sort", s);
  }

  toast(text: string, kind: Toast["kind"] = "info", ms = 4200) {
    const id = Date.now() + Math.random();
    this.toasts = [...this.toasts, { id, text, kind }];
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), ms);
  }

  async play(g: Game) {
    if (g.packed) {
      this.toast(`Unpacking ${g.title}…`);
      try {
        await api.unpack(g.id);
        this.toast(`${g.title} is ready to play`, "ok");
      } catch (e) {
        this.toast(String(e), "error", 8000);
      }
      return;
    }
    try {
      await api.launch(g.id);
      this.toast(`Starting ${g.title}`, "ok", 2500);
    } catch (e) {
      this.toast(String(e), "error", 8000);
    }
  }

  async toggleFavorite(g: Game) {
    await api.setFlag(g.id, "favorite", !g.favorite);
    await this.reload();
  }

  async scan() {
    try {
      const s = await api.scan();
      this.toast(
        `Library updated: ${s.games} games${s.newGames ? `, ${s.newGames} new` : ""}, ${s.emulators} emulators`,
        "ok",
      );
    } catch (e) {
      this.toast(String(e), "error", 8000);
    }
  }
}

export const app = new AppStore();
