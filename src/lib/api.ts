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
  status: "official" | "open-source" | "shareware" | "own-files" | "fan-original" | "custom";
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
};

export const src = (path: string | null | undefined) => (path ? convertFileSrc(path) : null);
