# OnionRing

One launcher for the games the big stores don't sell: your emulators and their games, plus native PC ports, recomps and decomps like bbport (Bloodborne), Ship of Harkinian or Zelda 64: Recompiled.

- **Adopts your setup as it is.** Point it at your emulation folder. It finds the emulators and games where they are and never moves or changes them.
- **Installs ports and emulators for you.** 225 community ports, recomps and decomps with Windows builds, plus the main emulators, are installed from their official GitHub Releases. Each download is checked against the hash the project published. Saves are kept across updates.
- **Small app, your data on your drive.** The installer is a few MB. Your library, artwork, installs and downloads live in a data folder you choose (for example `G:\OnionRing`).
- **Looks like a console.** The selected game's art fills the screen, with a cover shelf, keyboard and controller navigation, and playtime tracking.

OnionRing doesn't provide games, BIOS, firmware or keys. Ports and emulators need your own legally obtained game files.

## Install

Download `OnionRing_x.y.z_x64-setup.exe` from [Releases](https://github.com/George-Nizor/OnionRing/releases) and run it. It installs per-user (no admin rights needed) and updates itself from then on.

On first run you choose:

1. **A data folder.** OnionRing suggests a non-system drive, or an existing `OnionRing` folder it finds.
2. **Folders to adopt.** It suggests common ones such as `X:\Emulation` and `X:\ROMs`.

## Where things live

```text
%LOCALAPPDATA%\OnionRing\        the app (installer, ~5 MB)
%APPDATA%\OnionRing\config.json  remembers which data folder to use

<data folder>\                   e.g. G:\OnionRing
  library.db                     games, emulators, playtime (SQLite)
  Media\                         artwork cache
  Apps\<id>\<version>\           installed ports/emulators (the current and previous version)
  Apps\<id>\current              link to the active version (stable path for shortcuts)
  Apps\<id>\persist\             saves/settings, linked into every version
  Downloads\<id>\<version>\      the original archives (offline reinstall, rollback)
  Reclaim\                       duplicates you moved out of the way (restorable)
  catalog.local.json             optional: your own emulator/port definitions
```

Reinstall the app, point it at the same data folder, and everything comes back.

## Develop

Requirements: Node 22+, Rust (stable, MSVC), and WebView2 (built into Windows 11).

```bash
npm install
npm run app          # the desktop app with hot reload
npm run cli -- scan  # the same core, headless
```

| Command | What it does |
| --- | --- |
| `npm run app` | Desktop app in dev mode |
| `npm run dev:ui` | UI only, in a browser, against a snapshot of your library (see `.env.example`) |
| `npm run check` | Svelte/TypeScript checks |
| `npm run cli -- <cmd>` | `root`, `source add/list`, `scan`, `art`, `games`, `emulators`, `install <id> [--archive file]`, `updates`, `storage`, `snapshot <file>` |
| `npm run ports:refresh` | Rebuild the ports catalog from GitHub Releases |
| `cargo test -p onionring-core` | Core unit tests |

### Layout

```text
crates/core/        all logic: scanning, catalog, installs, art, launching, storage (+ onionring-cli)
  data/             built-in platforms, emulators and ports (JSON, compiled in)
src-tauri/          desktop shell: commands and events over the core
src/                Svelte 5 UI
catalog/            reference research catalog (351 entries) + hand-curated port entries
scripts/            catalog generator
```

### Releasing

Bump `version` in `package.json`, add a CHANGELOG entry, then:

```bash
git tag v0.2.0 && git push --tags
```

The **Release** workflow builds the signed installer and publishes it with `latest.json`. Installed copies offer the update on their next check. The updater signing key lives outside the repo (`~/.tauri/onionring-updater.key`) and in the `TAURI_SIGNING_PRIVATE_KEY` repo secret. Back it up, because without it existing installs can't verify updates.

### Keeping the catalog current

The **Refresh ports catalog** workflow runs weekly. It re-resolves which ports publish Windows builds and opens a PR if anything changed. To add or fix a port by hand, edit `catalog/ports.curated.json` (curated entries win) and run `npm run ports:refresh`.

## Licence

MIT. Ports and emulators installed through OnionRing keep their own licences.
