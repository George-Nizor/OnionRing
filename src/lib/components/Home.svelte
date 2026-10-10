<script lang="ts">
  import { tick } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, src, type Copy, type Emulator, type Game } from "$lib/api";
  import { app, type Filter, type Sort } from "$lib/store.svelte";
  import { ago, bytes, playtime } from "$lib/format";
  import type { PadAction } from "$lib/gamepad";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import Tile from "./Tile.svelte";

  let { emulators, searchEl }: { emulators: Emulator[]; searchEl?: HTMLInputElement } = $props();

  const game = $derived(app.selected);
  const platform = $derived(game ? app.platforms.get(game.platform) : undefined);
  const emu = $derived(emulators.find((e) => e.id === game?.emulator));
  const choices = $derived(emulators.filter((e) => game && e.platforms.includes(game.platform)));
  const running = $derived(game ? app.running.includes(game.id) : false);
  const grid = $derived(app.view === "grid");

  // ---- backdrop: sharp key art when it is big enough, otherwise a colour wash ----------------
  type Layer = { id: number; url: string | null; wash: string | null; sharp: boolean };
  let layers = $state<Layer[]>([]);
  let sharpIds = $state<Record<string, boolean>>({});

  function probe(url: string) {
    if (url in sharpIds) return;
    const img = new Image();
    img.onload = () => (sharpIds = { ...sharpIds, [url]: img.naturalWidth >= 1200 });
    img.src = url;
  }
  $effect(() => {
    const g = game;
    if (!g) return;
    const hero = src(g.hero);
    // A tiny logo makes a garish wash; games with only a logo get the accent glow instead.
    const wash = src(g.cover ?? g.coverThumb ?? g.hero);
    if (hero) probe(hero);
    if (layers.at(-1)?.id === g.id) return;
    layers = [...layers.slice(-1), { id: g.id, url: hero, wash, sharp: false }];
  });
  const sharp = $derived(!!(game && src(game.hero) && sharpIds[src(game.hero)!]));

  // ---- navigation ---------------------------------------------------------------------------
  let shelf = $state<HTMLDivElement>();
  async function reveal(id: number) {
    await tick();
    shelf?.querySelector<HTMLElement>(`[data-game="${id}"]`)?.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
  }
  function columns(): number {
    if (!grid || !shelf) return 1;
    return getComputedStyle(shelf).gridTemplateColumns.split(" ").length;
  }
  function move(delta: number) {
    const list = app.visible;
    if (!list.length) return;
    const i = list.findIndex((g) => g.id === game?.id);
    const next = list[Math.max(0, Math.min(list.length - 1, (i < 0 ? 0 : i) + delta))];
    app.select(next.id);
    reveal(next.id);
  }
  const filters = $derived<Filter[]>([
    "all",
    ...[...app.counts.keys()].filter((p) => p !== "port").sort().map((p) => `platform:${p}` as Filter),
    ...(app.counts.get("port") ? (["ports"] as Filter[]) : []),
  ]);
  function cycle(dir: number) {
    const i = Math.max(0, filters.indexOf(app.filter));
    app.setFilter(filters[(i + dir + filters.length) % filters.length]);
    if (app.visible[0]) app.select(app.visible[0].id);
  }
  function act(a: PadAction | string) {
    switch (a) {
      case "left": return move(-1);
      case "right": return move(1);
      case "up": return grid ? move(-columns()) : cycle(-1);
      case "down": return grid ? move(columns()) : cycle(1);
      case "a": return game && app.play(game);
      case "y": return game && app.toggleFavorite(game);
      case "lb": return cycle(-1);
      case "rb": return cycle(1);
      case "b": app.query = ""; return;
    }
  }
  $effect(() => {
    const onpad = (e: Event) => app.section === "library" && act((e as CustomEvent<PadAction>).detail);
    window.addEventListener("pad", onpad);
    return () => window.removeEventListener("pad", onpad);
  });
  function onkey(e: KeyboardEvent) {
    if (app.section !== "library") return;
    const typing = (e.target as HTMLElement)?.closest("input, textarea, select");
    if ((e.ctrlKey && e.key.toLowerCase() === "k") || (e.key === "/" && !typing)) {
      e.preventDefault();
      searchEl?.focus();
      return;
    }
    if (typing && !["ArrowDown", "Enter"].includes(e.key)) return;
    const map: Record<string, string> = { ArrowLeft: "left", ArrowRight: "right", ArrowUp: "up", ArrowDown: "down", Enter: "a" };
    if (map[e.key]) {
      e.preventDefault();
      if (typing) (e.target as HTMLElement).blur();
      act(map[e.key]);
    } else if (!typing && e.key.length === 1 && /\S/.test(e.key) && !e.ctrlKey && !e.altKey && !e.metaKey) {
      searchEl?.focus();
    }
  }
  // Wheel scrolls the shelf sideways.
  function onwheel(e: WheelEvent) {
    if (!grid && shelf && Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
      shelf.scrollLeft += e.deltaY;
      e.preventDefault();
    }
  }

  // ---- menu -------------------------------------------------------------------------------
  let menu = $state(false);
  let copies = $state<Copy[]>([]);
  async function toggleMenu() {
    menu = !menu;
    if (menu && game) copies = await api.copies(game.key);
  }
  async function pickCover() {
    if (!game) return;
    const file = await open({ multiple: false, filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }] });
    if (typeof file === "string") {
      await api.setCover(game.id, file);
      await app.reload();
    }
    menu = false;
  }
  async function chooseEmu(id: string) {
    if (!game) return;
    await api.setGameEmulator(game.id, id);
    await app.reload();
    menu = false;
  }
  async function toggleHidden() {
    if (!game) return;
    await api.setFlag(game.id, "hidden", !game.hidden);
    await app.reload();
    menu = false;
  }

  const sorts: [Sort, string][] = [
    ["title", "A–Z"],
    ["recent", "Recently played"],
    ["playtime", "Most played"],
    ["added", "Recently added"],
    ["size", "Largest"],
  ];
  const heading = $derived.by(() => {
    const f = app.filter;
    if (app.query) return `“${app.query}”`;
    if (f === "all") return "Your library";
    if (f === "recent") return "Recently played";
    if (f === "favorites") return "Favourites";
    if (f === "ports") return "Native ports";
    return app.platforms.get(f.slice(9))?.name ?? "Games";
  });
</script>

<svelte:window onkeydown={onkey} onclick={(e) => menu && !(e.target as HTMLElement).closest(".menu-wrap") && (menu = false)} />

<div class="home" class:grid style:--c={game ? app.accent(game) : "#ff5d7a"}>
  <!-- Backdrop -->
  <div class="backdrop" aria-hidden="true">
    {#each layers as l (l.id)}
      <div class="layer">
        {#if l.wash}<img class="wash" src={l.wash} alt="" />{:else}<div class="glow"></div>{/if}
        {#if l.url}<img class="key" class:sharp={l.id === game?.id && sharp} src={l.url} alt="" />{/if}
      </div>
    {/each}
    <div class="tint"></div>
    <div class="grain"></div>
  </div>

  <div class="scroll" onwheel={onwheel}>
    <!-- Selected game -->
    <section class="stage">
      {#if game}
        {#key game.id}
          <div class="info">
            <div class="kicker">
              <span class="dot" style:background={app.accent(game)}></span>
              {game.kind === "port" ? "Native PC port" : (platform?.name ?? game.platform)}
              {#if game.region}<span class="sep">·</span>{game.region}{/if}
            </div>
            <h1>{game.title}</h1>
            <div class="meta">
              <span>{game.playtime ? `${playtime(game.playtime)} played` : "Not played yet"}</span>
              {#if game.lastPlayed}<span>Last played {ago(game.lastPlayed).toLowerCase()}</span>{/if}
              <span>{bytes(game.size)}</span>
              {#if game.kind === "rom"}<span>{emu ? `${emu.name}${emu.version ? ` ${emu.version}` : ""}` : "No emulator yet"}</span>{/if}
            </div>
            <div class="actions">
              <button class="play" onclick={() => game && app.play(game)} disabled={game.missing || running}>
                {#if running}
                  <span class="live"></span> Playing
                {:else if game.packed}
                  <Icon name="unpack" size={20} /> Unpack &amp; play
                {:else}
                  <Icon name="play" size={20} /> Play
                {/if}
              </button>
              {#if game.kind === "port"}
                <button class="pill glass" onclick={() => game && api.configurePort(game.id).catch((e) => app.toast(String(e), "error"))}>
                  <Icon name="sliders" size={17} /> Settings
                </button>
              {/if}
              <button class="round glass" class:fav={game.favorite} onclick={() => game && app.toggleFavorite(game)} aria-label="Favourite" title="Favourite">
                <Icon name="star" size={19} />
              </button>
              <div class="menu-wrap">
                <button class="round glass" onclick={toggleMenu} aria-label="More" title="More"><Icon name="more" size={20} stroke={3} /></button>
                {#if menu}
                  <div class="menu">
                    <button onclick={() => { if (game) api.reveal(game.launchPath); menu = false; }}><Icon name="folder" size={16} /> Show in folder</button>
                    <button onclick={pickCover}><Icon name="image" size={16} /> Choose cover image…</button>
                    <button onclick={toggleHidden}><Icon name={game.hidden ? "eye" : "eyeoff"} size={16} /> {game.hidden ? "Show in library" : "Hide from library"}</button>
                    {#if choices.length > 1}
                      <div class="sep">Play with</div>
                      {#each choices as c (c.id)}
                        <button onclick={() => chooseEmu(c.id)}><Icon name={c.id === game.emulator ? "check" : "pad"} size={16} /> {c.name}</button>
                      {/each}
                    {/if}
                    {#if copies.length > 1}
                      <div class="sep">Copies on disk</div>
                      {#each copies as c (c.path)}
                        <div class="copy" title={c.path}>
                          <span class="fmt">{c.archive ? "zip" : c.format}</span>
                          <span class="p">{c.path}</span>
                          {#if c.primary}<Icon name="check" size={14} />{/if}
                        </div>
                      {/each}
                    {/if}
                  </div>
                {/if}
              </div>
            </div>
            {#if game.kind === "rom" && !emu}
              <p class="note"><Icon name="alert" size={15} /> Add an emulator for {platform?.name} in Emulators to play this.</p>
            {:else if emu?.notes}
              <p class="note"><Icon name="sparkle" size={15} /> {emu.notes}</p>
            {/if}
          </div>
          {#if !sharp}
            <div class="poster"><Cover {game} full /></div>
          {/if}
        {/key}
      {:else if app.games.length === 0}
        <div class="info">
          <div class="kicker">Welcome</div>
          <h1>Your library is empty</h1>
          <div class="meta"><span>Point OnionRing at your emulation folder, or grab a port.</span></div>
          <div class="actions">
            <button class="play" onclick={() => (app.section = "settings")}><Icon name="folder" size={18} /> Add a folder</button>
            <button class="pill glass" onclick={() => (app.section = "ports")}><Icon name="box" size={17} /> Browse ports</button>
          </div>
        </div>
      {/if}
    </section>

    <!-- Shelf / grid -->
    <section class="collection">
      <div class="head">
        <h2>{heading} <span>{app.visible.length}</span></h2>
        <div class="tools">
          {#if app.filter !== "recent"}
            <select class="sort" value={app.sort} onchange={(e) => app.setSort((e.target as HTMLSelectElement).value as Sort)}>
              {#each sorts as [k, l] (k)}<option value={k}>{l}</option>{/each}
            </select>
          {/if}
          <div class="toggle">
            <button class:on={!grid} onclick={() => app.setView("shelf")} title="Shelf"><Icon name="library" size={16} /></button>
            <button class:on={grid} onclick={() => app.setView("grid")} title="Grid"><Icon name="grid" size={16} /></button>
          </div>
        </div>
      </div>
      <div class="items" bind:this={shelf}>
        {#each app.visible as g, i (g.id)}
          <Tile game={g} index={i} selected={g.id === game?.id} {grid} />
        {/each}
        {#if app.visible.length === 0 && app.query}
          <p class="none">No games match “{app.query}”.</p>
        {/if}
      </div>
    </section>
  </div>
</div>

<style>
  .home {
    position: absolute;
    inset: 0;
    overflow: hidden;
    isolation: isolate;
  }
  /* ---- backdrop ---- */
  .backdrop {
    position: absolute;
    inset: 0;
    z-index: -1;
    overflow: hidden;
    background: #07060a;
  }
  .layer {
    position: absolute;
    inset: 0;
    background: #07060a;
    animation: fade 0.9s var(--ease) both;
  }
  .wash {
    position: absolute;
    inset: -15%;
    width: 130%;
    height: 130%;
    object-fit: cover;
    filter: blur(70px) saturate(1.5) brightness(0.48);
    transform: translateZ(0);
  }
  .glow {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(55% 60% at 72% 30%, color-mix(in srgb, var(--c) 30%, transparent), transparent 70%),
      radial-gradient(40% 50% at 20% 90%, color-mix(in srgb, var(--c) 12%, transparent), transparent 70%);
  }
  .key {
    position: absolute;
    right: 0;
    top: 0;
    width: 72%;
    height: 78%;
    object-fit: cover;
    opacity: 0.38;
    filter: blur(3px) saturate(1.1);
    mask: radial-gradient(120% 100% at 85% 25%, #000 25%, transparent 72%);
    transition:
      opacity 0.8s,
      filter 0.8s;
  }
  .key.sharp {
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 1;
    filter: none;
    mask: none;
    animation: drift 40s ease-in-out infinite alternate;
  }
  @keyframes drift {
    from {
      transform: scale(1.02);
    }
    to {
      transform: scale(1.09) translate3d(-1%, -1%, 0);
    }
  }
  .tint {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(90deg, rgba(5, 4, 8, 0.88) 0%, rgba(5, 4, 8, 0.55) 38%, rgba(5, 4, 8, 0) 70%),
      linear-gradient(0deg, rgba(5, 4, 8, 0.96) 0%, rgba(5, 4, 8, 0.6) 32%, rgba(5, 4, 8, 0) 60%),
      linear-gradient(180deg, rgba(5, 4, 8, 0.55) 0%, transparent 18%);
  }
  .grain {
    position: absolute;
    inset: 0;
    opacity: 0.06;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='160' height='160'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='.9' numOctaves='2' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E");
    mix-blend-mode: overlay;
    pointer-events: none;
  }

  .scroll {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .grid .scroll {
    overflow-y: auto;
  }

  /* ---- stage ---- */
  .stage {
    flex: 1;
    min-height: 330px;
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 48px;
    padding: 0 64px 34px;
  }
  .grid .stage {
    flex: none;
    height: min(52vh, 480px);
  }
  .info {
    max-width: 760px;
    animation: rise 0.55s var(--ease) both;
  }
  .kicker {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 12.5px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.72);
  }
  .kicker .dot {
    width: 8px;
    height: 8px;
    box-shadow: 0 0 12px currentColor;
  }
  .sep {
    opacity: 0.5;
  }
  h1 {
    margin: 12px 0 0;
    font-family: var(--display);
    font-size: clamp(44px, 4.8vw, 82px);
    font-weight: 750;
    line-height: 0.96;
    letter-spacing: -0.045em;
    text-wrap: balance;
    text-shadow: 0 6px 50px rgba(0, 0, 0, 0.45);
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    row-gap: 6px;
    margin: 18px 0 28px;
    font-size: 15px;
    color: rgba(255, 255, 255, 0.66);
  }
  .meta span + span::before {
    content: "·";
    margin: 0 12px;
    color: rgba(255, 255, 255, 0.35);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .play {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    height: 56px;
    padding: 0 38px 0 30px;
    border-radius: 99px;
    background: #fff;
    color: #0a090d;
    font-size: 17px;
    font-weight: 750;
    letter-spacing: -0.01em;
    box-shadow:
      0 18px 50px -14px rgba(0, 0, 0, 0.8),
      0 0 0 0 rgba(255, 255, 255, 0.4);
    transition:
      transform 0.25s var(--ease),
      box-shadow 0.3s;
  }
  .play:hover:not(:disabled) {
    transform: scale(1.035);
    box-shadow:
      0 22px 60px -14px rgba(0, 0, 0, 0.85),
      0 0 0 6px rgba(255, 255, 255, 0.14),
      0 0 60px -6px color-mix(in srgb, var(--c) 70%, transparent);
  }
  .play:active:not(:disabled) {
    transform: scale(0.98);
  }
  .play:disabled {
    background: rgba(255, 255, 255, 0.7);
    cursor: default;
  }
  .live {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #16a34a;
    animation: blink 1.2s infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.3;
    }
  }
  .pill,
  .round {
    height: 56px;
    border-radius: 99px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 9px;
    font-weight: 650;
    font-size: 15px;
    color: #fff;
    transition:
      background 0.2s,
      transform 0.2s var(--ease);
  }
  .pill {
    padding: 0 24px;
  }
  .round {
    width: 56px;
  }
  .pill:hover,
  .round:hover {
    background: var(--glass-2);
  }
  .round.fav {
    color: var(--gold);
  }
  .round.fav :global(svg) {
    fill: currentColor;
  }
  .note {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 20px 0 0;
    color: rgba(255, 255, 255, 0.6);
    font-size: 13.5px;
  }
  .menu-wrap {
    position: relative;
  }
  .menu {
    position: absolute;
    left: 0;
    bottom: 66px;
    z-index: 40;
    width: 380px;
    max-height: 440px;
    overflow-y: auto;
    padding: 6px;
    border-radius: 16px;
    background: rgba(18, 16, 24, 0.97);
    border: 1px solid var(--line-2);
    box-shadow: 0 40px 80px -24px rgba(0, 0, 0, 0.95);
    animation: rise 0.2s var(--ease);
  }
  .menu button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 11px;
    height: 38px;
    padding: 0 12px;
    border-radius: 10px;
    color: rgba(255, 255, 255, 0.78);
    text-align: left;
  }
  .menu button:hover {
    background: var(--surface-2);
    color: #fff;
  }
  .menu .sep {
    padding: 12px 12px 6px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--faint);
    border-top: 1px solid var(--line);
    margin-top: 6px;
    opacity: 1;
  }
  .copy {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 12px;
    font-size: 12px;
    color: var(--muted);
  }
  .fmt {
    flex: none;
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 5px;
    background: var(--surface-2);
  }
  .copy .p {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
  .poster {
    position: relative;
    flex: none;
    width: clamp(180px, 15vw, 250px);
    aspect-ratio: 3 / 4;
    border-radius: 16px;
    overflow: hidden;
    container-type: inline-size;
    margin-bottom: 6px;
    box-shadow:
      0 50px 90px -30px rgba(0, 0, 0, 0.95),
      0 0 0 1px rgba(255, 255, 255, 0.12);
    animation: rise 0.7s var(--ease) both 0.08s;
  }
  .grid .poster {
    display: none;
  }

  /* ---- collection ---- */
  .collection {
    flex: none;
    padding: 0 0 30px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 64px;
    margin-bottom: 4px;
  }
  .grid .head {
    position: sticky;
    top: 0;
    z-index: 5;
    padding-top: 12px;
    padding-bottom: 12px;
    background: linear-gradient(180deg, rgba(7, 6, 10, 0.92), rgba(7, 6, 10, 0.75) 70%, transparent);
    backdrop-filter: blur(12px);
  }
  h2 {
    margin: 0;
    font-family: var(--display);
    font-size: 17px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  h2 span {
    margin-left: 6px;
    color: rgba(255, 255, 255, 0.4);
    font-weight: 600;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .sort {
    height: 32px;
    padding: 0 30px 0 12px;
    border-radius: 99px;
    border: 1px solid var(--glass-line);
    background: var(--glass)
      url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6'%3E%3Cpath d='M1 1l4 4 4-4' fill='none' stroke='%23fff' stroke-opacity='.6' stroke-width='1.5'/%3E%3C/svg%3E")
      no-repeat right 12px center;
    appearance: none;
    font-size: 13px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.85);
    outline: none;
    cursor: pointer;
  }
  .sort option {
    background: #15121c;
  }
  .toggle {
    display: flex;
    padding: 3px;
    border-radius: 99px;
    background: var(--glass);
    border: 1px solid var(--glass-line);
  }
  .toggle button {
    width: 32px;
    height: 26px;
    display: grid;
    place-items: center;
    border-radius: 99px;
    color: rgba(255, 255, 255, 0.5);
  }
  .toggle button.on {
    background: #fff;
    color: #0a090d;
  }
  .items {
    display: flex;
    gap: 20px;
    overflow-x: auto;
    overflow-y: hidden;
    padding: 30px 64px 26px;
    scroll-padding: 0 64px;
    scrollbar-width: none;
    --tile-w: clamp(132px, 10.5vw, 176px);
  }
  .grid .items {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 34px 22px;
    overflow: visible;
    padding: 18px 64px 60px;
  }
  .none {
    color: var(--muted);
  }

  @media (max-height: 760px) {
    h1 {
      font-size: clamp(36px, 4vw, 60px);
    }
    .meta {
      margin: 12px 0 18px;
    }
  }
</style>
