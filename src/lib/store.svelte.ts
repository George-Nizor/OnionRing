import { listen } from "@tauri-apps/api/event";
import { api, type Bootstrap, type CatalogView, type Game, type Platform, type Progress, type ScanSummary } from "./api";
import { norm } from "./format";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type Theme = "pixel" | "paper";
export type ViewName = "home" | "library" | "game" | "get" | "systems" | "tidy" | "settings" | "stats";
export interface View {
  name: ViewName;
  gameId?: number;
  tab?: string;
  /** A catalog entry to open straight away (from search). */
  item?: string;
}
export type Sort = "title" | "recent" | "playtime" | "added" | "size";

export interface Toast {
  id: number;
  text: string;
  kind: "info" | "ok" | "error";
  /** How long it stays up, in ms. */
  ms: number;
}

const SORTS: Record<Sort, (a: Game, b: Game) => number> = {
  title: (a, b) => a.title.localeCompare(b.title),
  recent: (a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || a.title.localeCompare(b.title),
  playtime: (a, b) => b.playtime - a.playtime || a.title.localeCompare(b.title),
  added: (a, b) => b.addedAt - a.addedAt || a.title.localeCompare(b.title),
  size: (a, b) => b.size - a.size,
};
export const SORT_ORDER: Sort[] = ["title", "recent", "playtime", "added", "size"];
export const SORT_LABEL: Record<Sort, string> = {
  title: "A to Z",
  recent: "Last played",
  playtime: "Most played",
  added: "Newest",
  size: "Biggest",
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
    /* preferences just won't persist */
  }
}

class AppStore {
  boot = $state<Bootstrap | null>(null);
  games = $state<Game[]>([]);
  loaded = $state(false);

  view = $state<View>({ name: "home" });
  history = $state<View[]>([]);
  menuOpen = $state(false);
  /** The top-bar search is in use (its results show while there is a query). */
  searchOpen = $state(false);
  query = $state("");
  /** Bumped to ask the search field to take focus (Ctrl+K, "/", controller Y). */
  searchFocus = $state(0);

  theme = $state<Theme>(load("theme", "pixel"));
  platformFilter = $state<string>(load("platformFilter", "all"));
  sort = $state<Sort>(load("sort", "title"));
  /** The game the home spotlight is showing (follows focus). */
  spotlightId = $state<number | null>(null);

  progress = $state<Progress | null>(null);
  installing = $state<Record<string, string>>({});
  running = $state<number[]>([]);
  toasts = $state<Toast[]>([]);
  accents = $state<Record<string, string>>({});
  catalog = $state<CatalogView | null>(null);

  /** Search across everything: your games, and every port, emulator and browser game. */
  everything = $derived.by(() => {
    const q = norm(this.query);
    const empty = { games: [] as Game[], ports: [] as CatalogView["ports"], emulators: [] as CatalogView["emulators"], browser: [] as CatalogView["browser"] };
    if (!q) return empty;
    const hit = (...s: (string | null | undefined)[]) => norm(s.filter(Boolean).join(" ")).includes(q);
    const c = this.catalog;
    return {
      games: this.visibleGames.filter((g) => hit(g.title, g.serial)).slice(0, 18),
      ports: (c?.ports ?? []).filter((p) => hit(p.game, p.name, p.original.join(" "))).slice(0, 12),
      emulators: (c?.emulators ?? []).filter((e) => hit(e.name, e.platforms.map((p) => this.platforms.get(p)?.name).join(" "))).slice(0, 8),
      browser: (c?.browser ?? []).filter((b) => hit(b.title, b.project, b.original)).slice(0, 12),
    };
  });

  platforms = $derived(new Map<string, Platform>((this.boot?.platforms ?? []).map((p) => [p.id, p])));

  visibleGames = $derived(this.games.filter((g) => !g.hidden));

  counts = $derived.by(() => {
    const c = new Map<string, number>();
    for (const g of this.visibleGames) c.set(g.platform, (c.get(g.platform) ?? 0) + 1);
    return c;
  });

  library = $derived.by(() => {
    let list = this.visibleGames;
    const f = this.platformFilter;
    if (f === "favorites") list = list.filter((g) => g.favorite);
    else if (f !== "all") list = list.filter((g) => g.platform === f);
    return [...list].sort(SORTS[this.sort]);
  });

  /** Most recently played first, then newest additions, for the home "jump back in" row. */
  continueList = $derived.by(() => {
    const played = this.visibleGames.filter((g) => g.lastPlayed).sort(SORTS.recent);
    const rest = this.visibleGames.filter((g) => !g.lastPlayed).sort(SORTS.added);
    return [...played, ...rest].slice(0, 10);
  });

  spotlight = $derived(this.games.find((g) => g.id === this.spotlightId) ?? this.continueList[0] ?? null);
  current = $derived(this.view.gameId ? (this.games.find((g) => g.id === this.view.gameId) ?? null) : null);

  async init() {
    this.applyTheme();
    this.boot = await api.bootstrap();
    await listen<Progress>("progress", (e) => {
      this.progress = e.payload.stage === "idle" ? null : e.payload;
    });
    await listen("library-changed", () => this.reload());
    await listen<ScanSummary>("auto-scan", (e) => {
      const n = e.payload.newGames;
      if (n > 0) this.toast(n === 1 ? "Found 1 new game" : `Found ${n} new games`, "ok");
    });
    await listen<number>("game-started", (e) => (this.running = [...this.running, e.payload]));
    await listen<{ id: number }>("game-ended", (e) => (this.running = this.running.filter((r) => r !== e.payload.id)));
    await listen<{ id: string; message: string; done: boolean; error: string | null }>("install", (e) => {
      const { id, message, done, error } = e.payload;
      if (done) {
        const { [id]: _, ...rest } = this.installing;
        this.installing = rest;
        if (error) this.toast(error, "error", 9000);
        else if (id !== "anyps5") this.toast("Installed. It's in your library.", "ok");
        this.loadCatalog();
      } else this.installing = { ...this.installing, [id]: message };
    });
    if (this.boot.dataRoot) {
      await this.reload();
      this.loadCatalog();
    }
    this.loaded = true;
    this.autoUpdate();
  }

  /** Installed builds keep themselves current from the latest GitHub release: check shortly
   *  after start and every six hours, install quietly, then restart. Never while a game runs. */
  private autoUpdate() {
    if (import.meta.env.DEV || !("__TAURI_INTERNALS__" in window)) return;
    const run = async () => {
      if (this.running.length) return;
      try {
        const u = await check();
        if (!u) return;
        this.toast(`Updating OnionRing to ${u.version}…`, "info", 8000);
        await u.downloadAndInstall();
        await relaunch();
      } catch {
        /* offline or GitHub unreachable: try again next time */
      }
    };
    setTimeout(run, 8000);
    setInterval(run, 6 * 3600 * 1000);
  }

  async loadCatalog() {
    try {
      const c = await api.catalog();
      this.catalog = c;
      this.accents = Object.fromEntries(c.ports.filter((p) => p.accent).map((p) => [p.id, p.accent!]));
    } catch {
      /* catalog unavailable: search just covers the library */
    }
  }

  async reload() {
    try {
      this.games = await api.games();
    } catch (e) {
      this.toast(String(e), "error");
    }
  }

  // ---- navigation -------------------------------------------------------------------------
  go(v: View) {
    this.menuOpen = false;
    this.searchOpen = false;
    const same = this.view.name === v.name && this.view.gameId === v.gameId && this.view.tab === v.tab && this.view.item === v.item;
    if (same) return;
    this.history = [...this.history, this.view].slice(-30);
    this.view = v;
  }
  openSearch() {
    this.menuOpen = false;
    this.searchOpen = true;
    this.searchFocus++;
  }
  closeSearch() {
    this.searchOpen = false;
    this.query = "";
  }
  back(): boolean {
    if (this.searchOpen) {
      this.closeSearch();
      return true;
    }
    if (this.menuOpen) {
      this.menuOpen = false;
      return true;
    }
    const prev = this.history.at(-1);
    if (!prev) return false;
    this.history = this.history.slice(0, -1);
    this.view = prev;
    return true;
  }
  openGame(g: Game) {
    this.go({ name: "game", gameId: g.id });
  }

  setTheme(t: Theme) {
    this.theme = t;
    save("theme", t);
    this.applyTheme();
  }
  applyTheme() {
    document.documentElement.dataset.theme = this.theme;
  }
  setPlatformFilter(f: string) {
    this.platformFilter = f;
    save("platformFilter", f);
  }
  cycleSort() {
    const i = SORT_ORDER.indexOf(this.sort);
    this.sort = SORT_ORDER[(i + 1) % SORT_ORDER.length];
    save("sort", this.sort);
  }

  accent(g: Game): string {
    return (g.appId && this.accents[g.appId]) || "var(--accent)";
  }

  toast(text: string, kind: Toast["kind"] = "info", ms = 4200) {
    const id = Date.now() + Math.random();
    this.toasts = [...this.toasts, { id, text, kind, ms }].slice(-4);
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), ms);
  }

  async play(g: Game) {
    if (g.packed) {
      this.toast(`Unzipping ${g.title}…`);
      try {
        await api.unpack(g.id);
        this.toast(`${g.title} is unzipped and ready`, "ok");
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
      this.toast(`${s.games} games, ${s.emulators} emulators${s.newGames ? `, ${s.newGames} new` : ""}`, "ok");
    } catch (e) {
      this.toast(String(e), "error", 8000);
    }
  }
}

export const app = new AppStore();
