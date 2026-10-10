// Dev-only: lets the UI run in a normal browser (no Tauri) against a snapshot of a real library,
// for fast design iteration. Create one with:
//   npm run cli -- snapshot <data root>\Cache\ui-snapshot.json
// put VITE_ONIONRING_DATA=<data root> in .env.local, then `npm run dev:ui`.
// Never included in production builds.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

export async function installMock(dataRoot: string) {
  const base = dataRoot.replace(/[\\/]+$/, "").toLowerCase();
  // Files under the data root are served by the dev server at /__onionring/ (vite.config.js).
  const url = (p: string) => {
    const lower = p.toLowerCase();
    if (!lower.startsWith(base)) return "";
    return `/__onionring/${p.slice(base.length).replace(/^[\\/]+/, "").split(/[\\/]/).map(encodeURIComponent).join("/")}`;
  };
  const snap = await fetch("/__onionring/Cache/ui-snapshot.json", { cache: "no-store" }).then((r) => r.json());
  mockWindows("main");
  mockIPC(
    (cmd, args) => {
      switch (cmd) {
        case "bootstrap": return snap.bootstrap;
        case "games": return snap.games;
        case "emulators": return snap.emulators;
        case "sources": return snap.sources;
        case "catalog": return snap.catalog;
        case "storage_report": return snap.storage;
        case "check_updates": return [];
        case "game_copies": return [];
        case "open_browser_game": {
          const g = snap.catalog.browser?.find((b: { id: string }) => b.id === (args as { id: string }).id);
          if (g) window.open(g.url, "_blank");
          return null;
        }
        case "scan_library": return { games: snap.games.length, newGames: 0, emulators: snap.emulators.length, copies: 0, missing: 0, millis: 50 };
        default: return null;
      }
    },
    { shouldMockEvents: true },
  );
  const internals = (window as unknown as { __TAURI_INTERNALS__: { convertFileSrc: (p: string) => string } }).__TAURI_INTERNALS__;
  internals.convertFileSrc = url;
}
