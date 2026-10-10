import { invoke, convertFileSrc } from "@tauri-apps/api/core";

export interface Platform {
  id: string;
  name: string;
  short: string;
  color: string;
  libretro: string | null;
  extensions: string[];
}

export interface Game {
  id: number;
  key: string;
  title: string;
  platform: string;
  kind: "rom" | "port";
  launchPath: string;
  emulator: string | null;
  appId: string | null;
  serial: string | null;
  region: string | null;
  size: number;
  favorite: boolean;
  hidden: boolean;
  missing: boolean;
  packed: boolean;
  addedAt: number;
  lastPlayed: number | null;
  playtime: number;
  playCount: number;
  cover: string | null;
  coverThumb: string | null;
  hero: string | null;
  logo: string | null;
  copies: number;
  /** Launch options for this game only (one argument per entry; `{rom}`, `{rom_dir}`, `{emu_dir}`). null = the emulator's. */
  userArgs: string[] | null;
}

export interface Copy {
  path: string;
  format: string;
  size: number;
  archive: boolean;
  primary: boolean;
  registered: boolean;
}

export interface Emulator {
  id: string;
  name: string;
  exe: string;
  version: string | null;
  platforms: string[];
  args: string[];
  defaultArgs: string[];
  managed: boolean;
  missing: boolean;
  notes: string | null;
}

export interface Source {
  id: number;
  path: string;
  addedAt: number;
  exists: boolean;
}

export interface EmulatorDef {
  id: string;
  name: string;
  platforms: string[];
  repo: string | null;
  homepage: string | null;
  requires: string | null;
  notes: string | null;
}

export interface PortDef {
  id: string;
  game: string;
  name: string;
  kind: string;
  original: string[];
  repo: string;
  requires: string | null;
  licence: string | null;
  blurb: string | null;
  accent: string | null;
  featured: boolean;
}

export interface InstalledApp {
  id: string;
  name: string;
  kind: string;
  version: string;
  installedAt: number;
  exe: string;
  size: number;
  source: { repo: string | null; tag: string | null; sha256: string; verified: boolean; archive: string };
}

export interface BrowserGame {
  id: string;
  title: string;
  project: string;
  url: string;
  source: string | null;
  status: "official" | "open-source" | "shareware" | "own-files" | "fan-original" | "link" | "custom";
  original: string | null;
  summary: string | null;
  needsFiles: string | null;
  embed: "window" | "link";
  controls: string | null;
  multiplayer: boolean;
}

export type PicKind = "system" | "emulator" | "port" | "browser";

export interface ReleaseDetails {
  tag: string;
  published: string | null;
  asset: string | null;
  size: number | null;
  url: string | null;
}

export interface CatalogView {
  ports: PortDef[];
  emulators: EmulatorDef[];
  browser: BrowserGame[];
  installed: InstalledApp[];
}

export interface UpdateInfo {
  id: string;
  installed: string;
  latest: string;
  published: string | null;
  available: boolean;
  url: string | null;
}

export interface Bootstrap {
  version: string;
  dataRoot: string | null;
  suggestedRoot: string;
  suggestedSources: string[];
  platforms: Platform[];
}

export interface ScanSummary {
  games: number;
  newGames: number;
  emulators: number;
  copies: number;
  missing: number;
  millis: number;
}

export interface Progress {
  stage: string;
  detail: string;
  done: number;
  total: number;
}

export interface StorageReport {
  librarySize: number;
  reclaimable: number;
  reclaimDir: string;
  held: number;
  groups: {
    key: string;
    title: string;
    platform: string;
    keep: string;
    keepSize: number;
    reclaimable: number;
    redundant: { path: string; size: number; format: string; reason: string }[];
  }[];
}

/** One file an emulator needs from the player (BIOS, firmware, keys). */
export interface CheckResult {
  /** What the file is, in plain words ("PS2 BIOS"). */
  what: string;
  ok: boolean;
  /** The emulator works without it. */
  optional: boolean;
  /** One plain sentence on how to get it. */
  help: string;
  /** Where it was found or belongs, when that folder exists. For an "open folder" button (api.openFolder), never shown as text. */
  folder: string | null;
}

/** The setup checks of one emulator on the PC. */
export interface EmulatorChecks {
  /** Emulator id as in api.emulators() ("pcsx2", "pcsx2-2"). */
  id: string;
  checks: CheckResult[];
}

/** An update that can be installed. */
export interface AvailableUpdate {
  /** App id when OnionRing installed it, emulator id (as in api.emulators()) otherwise. */
  id: string;
  name: string;
  kind: "port" | "emulator";
  installed: string;
  latest: string;
  /** OnionRing installed it: update with api.installLatest(id). Otherwise the player updates it from `url`. */
  managed: boolean;
  published: string | null;
  url: string | null;
}

/** A list of games the player put together. */
export interface Collection {
  id: number;
  name: string;
  /** Unix seconds. */
  created: number;
  /** Games currently in the library, in the order they were added. */
  gameIds: number[];
}

/** One time a game was played (Unix seconds). */
export interface Session {
  id: number;
  gameId: number;
  started: number;
  ended: number;
  seconds: number;
}

/** Time played in one week, Monday 00:00 local time to the next Monday. */
export interface WeekTotal {
  /** Monday 00:00 local time, Unix seconds. */
  start: number;
  seconds: number;
  sessions: number;
}

export interface PlayTotals {
  seconds: number;
  sessions: number;
  /** Different games played at least once. */
  games: number;
}

/** A game ranked by time played. */
export interface TopGame {
  gameId: number;
  title: string;
  platform: string;
  seconds: number;
  sessions: number;
  /** Unix seconds. */
  lastPlayed: number;
}

/** A folder of saves that a backup covers. */
export interface SaveSource {
  /** Plain name, also the archive's name: "PCSX2", "Dolphin (Wii)", "Bloodborne". */
  name: string;
  path: string;
  /** Names of files and folders left out. */
  skip: string[];
}

export interface BackupItem {
  name: string;
  /** The folder it came from (only behind a "show files" control). */
  from: string;
  /** Archive size in bytes. */
  size: number;
}

export interface Backup {
  /** The folder's name, e.g. "2026-10-10_1532". */
  name: string;
  /** Unix seconds. */
  created: number;
  /** All archives together, in bytes. */
  size: number;
  path: string;
  items: BackupItem[];
  /** Plain-language problems ("Could not save PCSX2: ..."). Everything else was saved. */
  failed: string[];
}

/**
 * Events the backend emits (listen from "@tauri-apps/api/event"):
 * - "progress" (Progress): scans, art, unpacking and backups ("backup", then "done", then "idle").
 * - "library-changed": games changed; reload them.
 * - "auto-scan" (ScanSummary): auto-pickup finished a scan by itself (after "library-changed").
 * - "collections-changed": a collection was created, renamed, deleted or had games added or removed.
 * - "backups-changed": a backup finished.
 * - "game-started" (number), "game-ended" ({ id, seconds }), "install" (see store).
 */
export const api = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),
  setDataRoot: (path: string) => invoke<void>("set_data_root", { path }),
  sources: () => invoke<Source[]>("sources"),
  addSource: (path: string) => invoke<void>("add_source", { path }),
  removeSource: (id: number) => invoke<void>("remove_source", { id }),
  games: () => invoke<Game[]>("games"),
  copies: (key: string) => invoke<Copy[]>("game_copies", { key }),
  setFlag: (id: number, flag: "favorite" | "hidden", on: boolean) => invoke<void>("set_flag", { id, flag, on }),
  setGameEmulator: (id: number, emulator: string | null) => invoke<void>("set_game_emulator", { id, emulator }),
  setCover: (id: number, path: string) => invoke<void>("set_cover", { id, path }),
  scan: () => invoke<ScanSummary>("scan_library"),
  refreshArt: (force: boolean) => invoke<number>("refresh_art", { force }),
  launch: (id: number) => invoke<void>("launch_game", { id }),
  configurePort: (id: number) => invoke<void>("configure_port", { id }),
  unpack: (id: number) => invoke<ScanSummary>("unpack_game", { id }),
  emulators: () => invoke<Emulator[]>("emulators"),
  setEmulatorArgs: (id: string, args: string[] | null) => invoke<void>("set_emulator_args", { id, args }),
  catalog: () => invoke<CatalogView>("catalog"),
  installLatest: (id: string) => invoke<InstalledApp>("install_latest", { id }),
  installFromFile: (id: string, path: string) => invoke<InstalledApp>("install_from_file", { id, path }),
  checkUpdates: () => invoke<UpdateInfo[]>("check_updates"),
  storage: () => invoke<StorageReport>("storage_report"),
  reclaim: (paths: string[]) => invoke<[number, number]>("reclaim", { paths }),
  restoreReclaimed: () => invoke<number>("restore_reclaimed"),
  reveal: (path: string) => invoke<void>("reveal", { path }),
  openBrowserGame: (id: string) => invoke<void>("open_browser_game", { id }),
  addBrowserGame: (title: string, url: string) => invoke<BrowserGame>("add_browser_game", { title, url }),
  removeBrowserGame: (id: string) => invoke<void>("remove_browser_game", { id }),
  picture: (kind: PicKind, id: string) => invoke<string | null>("picture", { kind, id }),
  releaseDetails: (id: string) => invoke<ReleaseDetails | null>("release_details", { id }),

  /** Launch options for one game; null or [] goes back to the emulator's. Emits "library-changed". */
  setGameArgs: (id: number, args: string[] | null) => invoke<void>("set_game_args", { id, args }),

  /** Files each emulator on the PC needs (BIOS, firmware, keys). Emulators that need nothing are left out. */
  setupChecks: () => invoke<EmulatorChecks[]>("setup_checks"),
  /** Opens a folder in File Explorer (folders only). */
  openFolder: (path: string) => invoke<void>("open_folder", { path }),

  /** Every update available, ports and emulators. Slow the first time (asks GitHub). */
  availableUpdates: () => invoke<AvailableUpdate[]>("available_updates"),

  collections: () => invoke<Collection[]>("collections"),
  createCollection: (name: string) => invoke<Collection>("create_collection", { name }),
  renameCollection: (id: number, name: string) => invoke<void>("rename_collection", { id, name }),
  /** The games stay in the library. */
  deleteCollection: (id: number) => invoke<void>("delete_collection", { id }),
  addToCollection: (collectionId: number, gameId: number) => invoke<void>("add_to_collection", { collectionId, gameId }),
  removeFromCollection: (collectionId: number, gameId: number) =>
    invoke<void>("remove_from_collection", { collectionId, gameId }),

  /** A game's sessions, newest first (default 50). */
  gameSessions: (id: number, limit?: number) => invoke<Session[]>("game_sessions", { id, limit }),
  /** The last `weeks` weeks including this one (default 12), oldest first, empty weeks included. */
  weeklyPlaytime: (weeks?: number) => invoke<WeekTotal[]>("weekly_playtime", { weeks }),
  playTotals: () => invoke<PlayTotals>("play_totals"),
  /** Most played first (default 10). `since` (Unix seconds) counts only sessions started then or later. */
  topGames: (limit?: number, since?: number | null) => invoke<TopGame[]>("top_games", { limit, since }),

  /** The save folders a backup would cover. */
  saveSources: () => invoke<SaveSource[]>("save_sources"),
  /** Zips every save folder into a new backup. Progress events ("backup"); emits "backups-changed". */
  backUpSaves: () => invoke<Backup>("back_up_saves"),
  /** Past backups, newest first. */
  backups: () => invoke<Backup[]>("backups"),

  /** Auto-pickup: scan by itself when games are added to or removed from the library's folders. */
  autoScan: () => invoke<boolean>("auto_scan"),
  setAutoScan: (on: boolean) => invoke<void>("set_auto_scan", { on }),
};

export const src = (path: string | null | undefined) => (path ? convertFileSrc(path) : null);
