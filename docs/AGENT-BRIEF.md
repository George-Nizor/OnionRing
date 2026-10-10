# OnionRing: brief for anyone working on it

## The product and the person it's for

OnionRing is a Windows launcher for games the big stores don't sell: emulators and their games,
native PC ports, recomps and decomps, and browser ports. Steam/Epic/GOG are out of scope.
The owner (George) is a PC gamer at a desk, not on a couch: mouse and keyboard first, controller
supported. Not a TV/10-foot UI.

What he has explicitly rejected (do not reintroduce):
- Generic "React app" look, Netflix-style hero layouts, Inter/Inter Tight, rounded floating panels.
- Marketing language ("stunning", "experience", "Same games, stored twice"). Write plain words.
- File paths on screen (only behind an explicit "show files" control).
- Hotkey/controller hint footers. No hint bars anywhere.
- Picture-only tiles where you can't tell what a thing is: names must always be readable.
- Dithered images. Pixel theme is crisp pure black and white.
- Couch/TV features (big-screen mode, guide-button overlays).

What he asked for: charm, function, icons over text, show-not-tell, heavy but purposeful
animation, plain-language prose, two themes that share one layout.

## Themes (src/app.css)

- **pixel**: 1-bit black and white. Pixel fonts (Pixelify Sans for titles/text, Silkscreen for
  labels). 12x12 pixel icons. Square corners, 3px borders. Motion is stepped (`steps()`), like
  sprites: marching-ants focus, typewriter titles, blinking block cursor, blinds wipe between
  screens. Images are crisp greyscale (no dithering).
- **paper**: an e-reader. Ink #141311 on paper #eeebe3, Literata for titles/text, Atkinson
  Hyperlegible for labels. Muted colour accents: --accent #b5452f (red), --accent-2 #2f5d8a
  (blue), --accent-3 #c29433 (mustard). Pictures in soft colour, full colour on focus. Thin line
  icons. Motion: e-ink refresh flash between screens, ink underlines drawing in, a red bookmark
  ribbon dropping onto the focused tile.

Style per theme from a component with `:global(:root[data-theme="pixel"]) .thing { ... }`.
Tokens: --bg --fg --muted --faint --line --panel --hi-bg --hi-fg --accent(-2,-3) --title --text
--label --border --img-filter --img-filter-focus --ease. Shared classes: .btn (.primary .big
.icon), .label, .row, .row-head, .art.

## Code map

- `crates/core/` Rust, all logic. `catalog.rs` (built-in data in `data/*.json`: platforms,
  emulators, ports, browser-games), `scan.rs`, `db.rs` (SQLite), `install.rs`, `launch.rs`,
  `art.rs` (game art), `pictures.rs` (pictures for catalog things), `storage.rs` (duplicates),
  `paths.rs`, `bin/cli.rs` (`npm run cli -- <cmd>`, includes `snapshot`).
- `src-tauri/src/lib.rs` Tauri commands + events. Every command must be listed in
  `generate_handler!`.
- `src/lib/api.ts` typed command bindings. `src/lib/mock.ts` fakes every command for the browser
  preview (reads a snapshot). **A new command needs an api.ts binding AND a mock.ts case.**
- `src/lib/store.svelte.ts` app state (Svelte 5 runes): games, view/history navigation
  (`app.go({ name, gameId?, tab?, item? })`, `app.back()`), theme, search, catalog.
- `src/lib/nav.ts` spatial focus navigation: anything with `data-nav` is reachable by arrow
  keys/controller. Put `data-nav` on every interactive element.
- `src/lib/components/`: `Glyph.svelte` (icons: a 12x12 bitmap for pixel AND a 24x24 line path
  for paper, same name; add both when adding an icon), `Logo.svelte`, `Art.svelte` (game art),
  `Pic.svelte` (lazy pictures for catalog things), `SystemArt.svelte` (hand-drawn console art from
  `src/lib/art/systems/<platform>.json`), `Tile.svelte`, views: `Home`, `Library`, `GamePage`,
  `GetMore`, `Systems`, `Tidy`, `Settings`, `Setup`; shell: `StatusBar`, `Menu`, `Search`,
  `Transition`, `Toasts`; `routes/+page.svelte` mounts views by `app.view.name`.

## Checking your work

- `npm run check` (Svelte/TS, must be 0 errors 0 warnings).
- `cargo clippy --workspace -q -- -D warnings` and `cargo test -q -p onionring-core`.
- The dev server is ALREADY running at http://127.0.0.1:1420 with a mock backend fed by
  `G:\OnionRing\Cache\ui-snapshot.json`. Do not start or stop it. After changing what the backend
  returns, regenerate the snapshot: `npm run cli -- snapshot G:\OnionRing\Cache\ui-snapshot.json`.
- Screenshots exactly as shipped (headless Edge = the app's engine):
  `node scripts/shoot.mjs --views home,library --sizes 1440x900,2048x1152 --themes pixel,paper --out .shots/<yourname>`
  then Read the PNGs. Views: home, home-scrolled, library, game, game-port, get-ports,
  get-port-detail, get-emulators, get-browser, systems, tidy, settings, search, menu, or any
  `app.go` view name. Sizes that matter: 1440x900 (default window), 1536x864 and 2048x1152
  (maximised 1080p/1440p at 125%), 1024x640 (minimum).
- Never commit or push; the lead does that.
