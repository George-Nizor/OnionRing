// Dev-only: lets the UI run in a normal browser (no Tauri) against a snapshot of a real library,
// for fast design iteration. Create one with:
//   npm run cli -- snapshot <data root>\Cache\ui-snapshot.json
// put VITE_ONIONRING_DATA=<data root> in .env.local, then `npm run dev:ui`.
// Never included in production builds.
//
// Reads come from the snapshot. Commands that change things (collections, launch options,
// favourites, auto-pickup, backups) change an in-memory copy and emit the same events as the
// app, so the UI reacts as it would. Nothing is written to disk.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type {
  AvailableUpdate,
  Backup,
  Bootstrap,
  CatalogView,
  Collection,
  Emulator,
  EmulatorChecks,
  Game,
  PlayTotals,
  Progress,
  SaveSource,
  Session,
  Source,
  StorageReport,
  TopGame,
  WeekTotal,
} from "./api";

/** What `onionring-cli snapshot` writes. Keys added later are optional so older snapshots still load. */
interface Snapshot {
  bootstrap: Bootstrap;
  games: Game[];
  emulators: Emulator[];
  sources: Source[];
  catalog: CatalogView;
  storage: StorageReport;
  pictures?: Record<string, string>;
  checks?: EmulatorChecks[];
  updates?: AvailableUpdate[];
  collections?: Collection[];
  history?: { totals: PlayTotals; top: TopGame[]; sessions: Record<string, Session[]> };
  saveSources?: SaveSource[];
  /** Folder size of each save source, standing in for its archive's size. */
  saveSizes?: Record<string, number>;
  backups?: Backup[];
  autoScan?: boolean;
}

type Args = Record<string, unknown>;

const clone = <T>(v: T): T => structuredClone(v);
const unix = () => Math.floor(Date.now() / 1000);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const DAY = 86_400;

/** Monday 00:00 local time of the week holding `t` (Unix seconds), as the app works it out. */
function weekStart(t: number): number {
  const d = new Date(t * 1000);
  d.setHours(0, 0, 0, 0);
  d.setDate(d.getDate() - ((d.getDay() + 6) % 7));
  return Math.floor(d.getTime() / 1000);
}

/** The core's collection name rules and messages. */
function collectionName(raw: unknown, taken: Collection[], except?: number): string {
  const name = String(raw ?? "").trim();
  if (!name) throw "Give the collection a name.";
  if ([...name].length > 80) throw "That name is too long. Keep it under 80 characters.";
  if (taken.some((c) => c.id !== except && c.name.toLowerCase() === name.toLowerCase()))
    throw `You already have a collection called ${name}.`;
  return name;
}

export async function installMock(dataRoot: string) {
  const base = dataRoot.replace(/[\\/]+$/, "").toLowerCase();
  // Files under the data root are served by the dev server at /__onionring/ (vite.config.js).
  const url = (p: string) => {
    const lower = p.toLowerCase();
    if (!lower.startsWith(base)) return "";
    return `/__onionring/${p.slice(base.length).replace(/^[\\/]+/, "").split(/[\\/]/).map(encodeURIComponent).join("/")}`;
  };
  const snap: Snapshot = await fetch("/__onionring/Cache/ui-snapshot.json", { cache: "no-store" }).then((r) => r.json());

  // In-memory state the preview can change.
  let collections: Collection[] = clone(snap.collections ?? []);
  let nextCollection = Math.max(0, ...collections.map((c) => c.id)) + 1;
  const backups: Backup[] = clone(snap.backups ?? []);
  let autoScan = snap.autoScan ?? true;
  let backingUp = false;
  const sessions = snap.history?.sessions ?? {};
  const allSessions = () => Object.values(sessions).flat();
  const game = (id: unknown) => snap.games.find((g) => g.id === id);
  const progress = (p: Progress) => emit("progress", p);

  function weekly(weeks: number): WeekTotal[] {
    const n = Math.min(Math.max(1, Math.floor(weeks)), 520);
    const starts: number[] = [];
    for (let i = 0, t = weekStart(unix()); i < n; i++) {
      starts.unshift(t);
      t = weekStart(t - DAY); // the Monday before, safe across daylight saving
    }
    const out = starts.map((start) => ({ start, seconds: 0, sessions: 0 }));
    const end = weekStart(starts[n - 1] + 7 * DAY + DAY / 2); // next Monday 00:00
    for (const s of allSessions()) {
      if (s.started < starts[0] || s.started >= end) continue;
      const i = starts.findLastIndex((w) => w <= s.started);
      out[i].seconds += Math.max(0, s.ended - s.started);
      out[i].sessions += 1;
    }
    return out;
  }

  function topSince(limit: number, since: number): TopGame[] {
    const by = new Map<number, TopGame>();
    for (const s of allSessions()) {
      const g = game(s.gameId);
      if (s.started < since || !g) continue;
      const t = by.get(s.gameId) ?? { gameId: s.gameId, title: g.title, platform: g.platform, seconds: 0, sessions: 0, lastPlayed: 0 };
      t.seconds += Math.max(0, s.ended - s.started);
      t.sessions += 1;
      t.lastPlayed = Math.max(t.lastPlayed, s.started);
      by.set(s.gameId, t);
    }
    return [...by.values()].sort((a, b) => b.seconds - a.seconds || b.lastPlayed - a.lastPlayed).slice(0, limit);
  }

  async function backUp(): Promise<Backup> {
    if (backingUp) throw "A backup is already running.";
    const sources = snap.saveSources ?? [];
    if (!sources.length) throw "There are no saves to back up yet.";
    backingUp = true;
    try {
      for (const [i, s] of sources.entries()) {
        await progress({ stage: "backup", detail: s.name, done: i, total: sources.length });
        await sleep(300);
      }
      await progress({ stage: "done", detail: `${sources.length} saved`, done: sources.length, total: sources.length });
      const created = unix();
      const d = new Date(created * 1000);
      const pad = (n: number) => String(n).padStart(2, "0");
      const stamp = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}_${pad(d.getHours())}${pad(d.getMinutes())}`;
      let name = stamp;
      for (let n = 2; backups.some((b) => b.name === name); n++) name = `${stamp}_${n}`;
      const items = sources.map((s) => ({ name: s.name, from: s.path, size: snap.saveSizes?.[s.path] ?? 0 }));
      const backup: Backup = {
        name,
        created,
        size: items.reduce((sum, i) => sum + i.size, 0),
        path: `${dataRoot.replace(/[\\/]+$/, "")}\\Backups\\${name}`,
        items,
        failed: [],
      };
      backups.unshift(backup);
      return clone(backup);
    } finally {
      backingUp = false;
      await progress({ stage: "idle", detail: "", done: 1, total: 1 });
    }
  }

  mockWindows("main");
  mockIPC(
    async (cmd, raw) => {
      const args = (raw ?? {}) as Args;
      switch (cmd) {
        case "bootstrap": return snap.bootstrap;
        case "games": return clone(snap.games);
        case "emulators": return snap.emulators;
        case "sources": return snap.sources;
        case "catalog": return snap.catalog;
        case "storage_report": return snap.storage;
        case "check_updates": return [];
        case "game_copies": return [];
        case "picture": return snap.pictures?.[`${args.kind}:${args.id}`] ?? null;
        case "release_details": return null;
        case "open_browser_game": {
          const g = snap.catalog.browser?.find((b) => b.id === args.id);
          if (g) window.open(g.url, "_blank");
          return null;
        }
        case "scan_library": return { games: snap.games.length, newGames: 0, emulators: snap.emulators.length, copies: 0, missing: 0, millis: 50 };

        case "set_flag": {
          const g = game(args.id);
          if (g && (args.flag === "favorite" || args.flag === "hidden")) g[args.flag] = Boolean(args.on);
          return null;
        }
        case "anyps5_status":
          return { tool: null, converted: false };
        case "anyps5_convert":
        case "anyps5_forget":
          return null;
        case "set_game_args": {
          const g = game(args.id);
          const list = args.args as string[] | null | undefined;
          if (g) g.userArgs = list?.length ? [...list] : null;
          await emit("library-changed");
          return null;
        }

        // ---- setup checks, updates
        case "setup_checks": return clone(snap.checks ?? []);
        case "open_folder": return null;
        case "available_updates": return clone(snap.updates ?? []);

        // ---- collections
        case "collections":
          return clone(collections).sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) || a.id - b.id);
        case "create_collection": {
          const name = collectionName(args.name, collections);
          const c: Collection = { id: nextCollection++, name, created: unix(), gameIds: [] };
          collections.push(c);
          await emit("collections-changed");
          return clone(c);
        }
        case "rename_collection": {
          const c = collections.find((x) => x.id === args.id);
          const name = collectionName(args.name, collections, args.id as number);
          if (!c) throw "That collection no longer exists.";
          c.name = name;
          await emit("collections-changed");
          return null;
        }
        case "delete_collection":
          collections = collections.filter((c) => c.id !== args.id);
          await emit("collections-changed");
          return null;
        case "add_to_collection": {
          const c = collections.find((x) => x.id === args.collectionId);
          if (!c) throw "That collection no longer exists.";
          if (!game(args.gameId)) throw "That game is no longer in your library.";
          if (!c.gameIds.includes(args.gameId as number)) c.gameIds.push(args.gameId as number);
          await emit("collections-changed");
          return null;
        }
        case "remove_from_collection": {
          const c = collections.find((x) => x.id === args.collectionId);
          if (c) c.gameIds = c.gameIds.filter((id) => id !== args.gameId);
          await emit("collections-changed");
          return null;
        }

        // ---- play history
        case "game_sessions": return clone((sessions[String(args.id)] ?? []).slice(0, (args.limit as number | undefined) ?? 50));
        case "weekly_playtime": return weekly((args.weeks as number | undefined) ?? 12);
        case "play_totals": return clone(snap.history?.totals ?? { seconds: 0, sessions: 0, games: 0 });
        case "top_games": {
          const limit = (args.limit as number | undefined) ?? 10;
          const since = args.since as number | null | undefined;
          return since == null ? clone((snap.history?.top ?? []).slice(0, limit)) : topSince(limit, since);
        }

        // ---- save backups
        case "save_sources": return clone(snap.saveSources ?? []);
        case "backups": return clone(backups);
        case "back_up_saves": {
          const b = await backUp();
          await emit("backups-changed");
          return b;
        }

        // ---- auto-pickup
        case "auto_scan": return autoScan;
        case "set_auto_scan":
          autoScan = Boolean(args.on);
          return null;

        default: return null;
      }
    },
    { shouldMockEvents: true },
  );
  const internals = (window as unknown as { __TAURI_INTERNALS__: { convertFileSrc: (p: string) => string } }).__TAURI_INTERNALS__;
  internals.convertFileSrc = url;
}
