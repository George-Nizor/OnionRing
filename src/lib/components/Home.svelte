<script lang="ts">
  import { api, type BrowserGame, type Emulator, type WeekTotal } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { ago, bytes, playtime } from "$lib/format";
  import Art from "./Art.svelte";
  import Glyph from "./Glyph.svelte";
  import Pic from "./Pic.svelte";
  import SystemArt from "./SystemArt.svelte";
  import Tile from "./Tile.svelte";

  let { emulators, portCount = 0, browser = [] }: { emulators: Emulator[]; portCount?: number; browser?: BrowserGame[] } = $props();
  // Ready to play with nothing to supply first.
  const instant = $derived(browser.filter((b) => !b.needsFiles && b.embed === "window").slice(0, 12));

  const g = $derived(app.spotlight);
  const platform = $derived(g ? app.platforms.get(g.platform) : undefined);
  const emu = $derived(emulators.find((e) => e.id === g?.emulator));
  const running = $derived(g ? app.running.includes(g.id) : false);

  const played = $derived(
    app.visibleGames
      .filter((x) => x.lastPlayed)
      .sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0))
      .slice(0, 12),
  );
  const newest = $derived([...app.visibleGames].sort((a, b) => b.addedAt - a.addedAt || a.title.localeCompare(b.title)).slice(0, 12));
  const systems = $derived(
    [...app.counts.entries()]
      .filter(([id]) => id !== "port")
      .map(([id, n]) => ({ p: app.platforms.get(id)!, n, ok: emulators.some((e) => e.platforms.includes(id) && !e.missing) }))
      .filter((s) => s.p)
      .sort((a, b) => b.n - a.n),
  );
  const ports = $derived(app.visibleGames.filter((x) => x.kind === "port"));
  const short = (id: string) => (id === "port" ? "PC port" : (app.platforms.get(id)?.short ?? ""));

  // This week's play time, with the last 12 weeks as a little bar strip.
  let weeks = $state<WeekTotal[]>([]);
  $effect(() => {
    void app.games;
    api
      .weeklyPlaytime(12)
      .then((w) => (weeks = w))
      .catch(() => (weeks = []));
  });
  const week = $derived(weeks.at(-1));
  const peak = $derived(Math.max(1, ...weeks.map((w) => w.seconds)));
  function hm(s: number) {
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    return h ? `${h}h ${m}m` : `${m}m`;
  }

  const hover = (e: MouseEvent) => (e.currentTarget as HTMLElement).focus({ preventScroll: true });

  // Rows scroll sideways; the arrows in each row's heading page them for mouse users.
  function pager(row: HTMLElement) {
    const block = row.closest<HTMLElement>(".block");
    const update = () => {
      if (!block) return;
      block.dataset.start = String(row.scrollLeft <= 2);
      block.dataset.end = String(row.scrollLeft + row.clientWidth >= row.scrollWidth - 2);
    };
    const ro = new ResizeObserver(update);
    ro.observe(row);
    for (const c of row.children) ro.observe(c);
    row.addEventListener("scroll", update, { passive: true });
    block?.addEventListener("mouseenter", update);
    update();
    return {
      destroy() {
        ro.disconnect();
        row.removeEventListener("scroll", update);
        block?.removeEventListener("mouseenter", update);
      },
    };
  }
  function page(e: MouseEvent, dir: number) {
    const row = (e.currentTarget as HTMLElement).closest(".block")?.querySelector<HTMLElement>(".lane");
    row?.scrollBy({ left: dir * row.clientWidth * 0.8, behavior: "smooth" });
  }
</script>

{#snippet head(icon: string, label: string, count?: number)}
  <div class="head">
    <Glyph name={icon} size={24} />
    <h2 class="label">{label}</h2>
    {#if count}<span class="count label">{count}</span>{/if}
    <span class="pg">
      <button class="pb prev" tabindex="-1" aria-label="Scroll left" onclick={(e) => page(e, -1)}><Glyph name="prev" size={24} /></button>
      <button class="pb next" tabindex="-1" aria-label="Scroll right" onclick={(e) => page(e, 1)}><Glyph name="next" size={24} /></button>
    </span>
  </div>
{/snippet}

<div class="home">
  {#if g}
    <section class="spot">
      {#key g.id}
        <div class="info">
          <div class="kicker">
            <span class="plat label">{short(g.kind === "port" ? "port" : g.platform)}</span>
            {#if platform && g.kind !== "port"}<span class="pname label">{platform.name}</span>{/if}
            {#if g.packed}<span class="tag"><Glyph name="zip" size={24} label="Zipped" /></span>{/if}
            {#if g.favorite}<span class="tag"><Glyph name="star" size={24} label="Favourite" /></span>{/if}
          </div>
          <h1 class="t" class:long={g.title.length > 28}><span class="tt">{g.title}</span><span class="cur" aria-hidden="true"></span></h1>
          <div class="stats">
            <span class="stat"><Glyph name="clock" size={24} label="Played" /><b>{playtime(g.playtime)}</b></span>
            <span class="stat"><Glyph name="disk" size={24} label="Size" /><b>{bytes(g.size)}</b></span>
            {#if g.kind === "rom"}
              <span class="stat"><Glyph name="pad" size={24} label="Emulator" /><b>{emu?.name ?? "No emulator yet"}</b></span>
            {/if}
          </div>
          <div class="actions">
            <button class="btn big primary" data-nav onclick={() => app.play(g)} disabled={g.missing || running}>
              <Glyph name={g.packed ? "zip" : "play"} size={24} />
              {running ? "Playing" : g.packed ? "Unzip" : "Play"}
            </button>
            <button class="btn big icon" data-nav onclick={() => app.openGame(g)} aria-label="Open game page" title="Game page"><Glyph name="info" size={24} /></button>
            <button class="btn big icon" class:fav={g.favorite} data-nav onclick={() => app.toggleFavorite(g)} aria-label="Favourite" title={g.favorite ? "Unfavourite" : "Favourite"}><Glyph name="star" size={24} /></button>
          </div>
        </div>
        <button class="wide" tabindex="-1" onclick={() => app.openGame(g)} aria-label="Open {g.title}"><Art game={g} kind="wide" /></button>
      {/key}
    </section>
  {/if}

  <button class="week" data-nav onclick={() => app.go({ name: "stats" })} onmouseenter={hover} aria-label="Play history">
    <span class="wk-l"><Glyph name="chart" size={24} /><span class="label">This week</span></span>
    <span class="wk-v">{week?.seconds ? hm(week.seconds) : "Nothing played yet"}</span>
    {#if week?.sessions}<span class="wk-s label">{week.sessions} {week.sessions === 1 ? "session" : "sessions"}</span>{/if}
    <span class="bars" aria-hidden="true">
      {#each weeks as w, i (w.start)}
        <span class="b" class:now={i === weeks.length - 1} style:--h={w.seconds / peak}></span>
      {/each}
    </span>
    <span class="wk-go label">Play history<Glyph name="next" size={24} /></span>
  </button>

  {#if played.length}
    <div class="block">
      {@render head("clock", "Played lately")}
      <div class="lane" use:pager>
        {#each played as game, i (game.id)}
          <Tile {game} index={i} meta={ago(game.lastPlayed)} onfocus={() => (app.spotlightId = game.id)} />
        {/each}
      </div>
    </div>
  {/if}

  {#if newest.length}
    <div class="block">
      {@render head("spark", "Newest")}
      <div class="lane" use:pager>
        {#each newest as game, i (game.id)}
          <Tile {game} index={i} meta={short(game.kind === "port" ? "port" : game.platform)} onfocus={() => (app.spotlightId = game.id)} />
        {/each}
      </div>
    </div>
  {/if}

  {#if systems.length}
    <div class="block">
      {@render head("pad", "Systems", systems.length)}
      <div class="lane" use:pager>
        {#each systems as s, i (s.p.id)}
          <button
            class="card"
            data-nav
            style:--i={i}
            onclick={() => {
              app.setPlatformFilter(s.p.id);
              app.go({ name: "library" });
            }}
            onmouseenter={hover}
            aria-label="{s.p.name}, {s.n} games"
          >
            <span class="pic sys"><span class="sa"><SystemArt id={s.p.id} label={s.p.name} /></span></span>
            <span class="cap">
              <span class="nm"><span>{s.p.name}</span></span>
              <span class="meta label">
                {s.n} {s.n === 1 ? "game" : "games"}
                {#if !s.ok}<span class="warn"><Glyph name="alert" size={12} label="No emulator yet" /></span>{/if}
              </span>
            </span>
          </button>
        {/each}
      </div>
    </div>
  {/if}

  <div class="block">
    {@render head("cart", "Ports", ports.length)}
    <div class="lane" use:pager>
      {#each ports as game, i (game.id)}
        <Tile {game} index={i} meta="PC port" onfocus={() => (app.spotlightId = game.id)} />
      {/each}
      <button class="card tall" data-nav style:--i={Math.min(ports.length, 14)} onclick={() => app.go({ name: "get", tab: "ports" })} onmouseenter={hover}>
        <span class="pic more"><Glyph name="plus" size={48} /></span>
        <span class="cap">
          <span class="nm"><span>Get more ports</span></span>
          {#if portCount}<span class="meta label"><span class="ell">{portCount} to choose from</span></span>{/if}
        </span>
      </button>
    </div>
  </div>

  {#if instant.length}
    <div class="block">
      {@render head("globe", "Play in a window, no install")}
      <div class="lane" use:pager>
        {#each instant as b, i (b.id)}
          <button
            class="card"
            data-nav
            style:--i={i}
            onclick={() => api.openBrowserGame(b.id).catch((e) => app.toast(String(e), "error"))}
            onmouseenter={hover}
            aria-label="Play {b.title}"
          >
            <span class="pic web"><Pic kind="browser" id={b.id} icon="globe" /><span class="playme"><Glyph name="play" size={24} /></span></span>
            <span class="cap">
              <span class="nm"><span>{b.title}</span></span>
              <span class="meta label">{b.original?.replace(/\s*\(.*\)/, "") ?? b.project}</span>
            </span>
          </button>
        {/each}
        <button class="card" data-nav style:--i={Math.min(instant.length, 14)} onclick={() => app.go({ name: "get", tab: "browser" })} onmouseenter={hover}>
          <span class="pic more"><Glyph name="plus" size={48} /></span>
          <span class="cap">
            <span class="nm"><span>All browser games</span></span>
            <span class="meta label">{browser.length} in all</span>
          </span>
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  .home {
    /* The spotlight art is bounded by the window height, so rows always get room below it. */
    --spot-h: clamp(200px, 34vh, 420px);
    --card: calc(var(--tile) * 1.5);
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    padding: var(--s6) var(--gutter) var(--s7);
    display: flex;
    flex-direction: column;
    gap: var(--s6);
  }
  /* Nothing on the home page may shrink to fit; it scrolls instead. */
  .home > :global(*) {
    flex: none;
  }

  /* ---- spotlight ---- */
  .spot {
    display: grid;
    grid-template-columns: minmax(0, 1fr) min(52%, calc(var(--spot-h) * 16 / 9));
    gap: clamp(24px, 3vw, 56px);
    align-items: center;
    min-height: var(--spot-h);
  }
  .info {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--s4);
  }
  .kicker {
    display: flex;
    align-items: center;
    gap: var(--s3);
    min-height: 28px;
  }
  .plat {
    padding: 5px 10px;
    line-height: 1;
  }
  .pname {
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tag {
    display: grid;
    place-items: center;
  }
  :global(:root[data-theme="pixel"]) .plat {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="paper"]) .plat {
    border: 1.5px solid var(--accent);
    color: var(--accent);
    border-radius: var(--radius);
  }
  :global(:root[data-theme="paper"]) .tag {
    color: var(--accent-3);
  }
  .t {
    margin: 0;
    font-family: var(--title);
    font-weight: 800;
    font-size: clamp(34px, 3.6vw, 68px);
    line-height: 1.02;
    text-wrap: balance;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    padding-bottom: 0.06em;
  }
  .t.long {
    font-size: clamp(28px, 2.7vw, 52px);
  }
  :global(:root[data-theme="pixel"]) .t {
    font-weight: 700;
    animation: typing 0.45s steps(10) both;
  }
  :global(:root[data-theme="pixel"]) .cur {
    display: inline-block;
    width: 0.45em;
    height: 0.8em;
    margin-left: 0.15em;
    background: var(--fg);
    vertical-align: baseline;
    animation: blink 0.9s steps(1) infinite;
  }
  @keyframes typing {
    from {
      clip-path: inset(0 100% 0 0);
    }
  }
  :global(:root[data-theme="paper"]) .t {
    font-weight: 700;
    letter-spacing: -0.02em;
    animation: rise 0.45s var(--ease) both;
  }
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s2) var(--s5);
    color: var(--muted);
  }
  .stat {
    display: flex;
    align-items: center;
    gap: var(--s2);
    font-family: var(--label);
    font-size: 15px;
  }
  .stat b {
    font-weight: 700;
    color: var(--fg);
  }
  .actions {
    display: flex;
    gap: var(--s3);
    margin-top: var(--s2);
  }
  :global(:root[data-theme="paper"]) .fav {
    color: var(--accent-3);
  }
  .wide {
    position: relative;
    aspect-ratio: 16 / 9;
    width: 100%;
    cursor: pointer;
  }
  :global(:root[data-theme="pixel"]) .wide {
    outline: var(--border) solid var(--line);
    outline-offset: 6px;
    animation: pxshow 0.25s steps(5) both;
  }
  @keyframes pxshow {
    from {
      clip-path: inset(0 0 100% 0);
    }
  }
  :global(:root[data-theme="paper"]) .wide {
    box-shadow:
      0 0 0 1px var(--line),
      10px 10px 0 -1px var(--panel),
      10px 10px 0 0 var(--line);
    animation: rise 0.5s var(--ease) both;
  }
  :global(:root[data-theme="paper"]) .wide :global(.art) {
    filter: var(--img-filter-focus);
  }

  /* ---- this week ---- */
  .week {
    display: flex;
    align-items: center;
    gap: var(--s5);
    height: 52px;
    padding: 0 var(--s4);
    text-align: left;
  }
  :global(:root[data-theme="pixel"]) .week {
    border: var(--border) solid var(--line);
  }
  :global(:root[data-theme="pixel"]) .week:hover {
    background: var(--hi-bg);
    color: var(--hi-fg);
  }
  :global(:root[data-theme="paper"]) .week {
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
    transition: background 0.2s;
  }
  :global(:root[data-theme="paper"]) .week:hover {
    background: var(--panel);
  }
  .wk-l {
    display: flex;
    align-items: center;
    gap: var(--s2);
  }
  .wk-v {
    font-family: var(--title);
    font-weight: 700;
    font-size: 20px;
    white-space: nowrap;
  }
  .wk-s {
    color: var(--faint);
    white-space: nowrap;
  }
  .week:is(:hover, :focus-visible) .wk-s {
    color: inherit;
  }
  .bars {
    flex: 1;
    min-width: 0;
    height: 28px;
    display: flex;
    align-items: flex-end;
    justify-content: flex-end;
    gap: 4px;
  }
  .b {
    width: 8px;
    height: max(2px, calc(var(--h) * 100%));
    background: currentColor;
    opacity: 0.35;
  }
  .b.now {
    opacity: 1;
  }
  :global(:root[data-theme="paper"]) .b.now {
    background: var(--accent);
  }
  :global(:root[data-theme="pixel"]) .b {
    animation: grow 0.3s steps(3) both;
  }
  @keyframes grow {
    from {
      transform: scaleY(0);
      transform-origin: bottom;
    }
  }
  .wk-go {
    display: flex;
    align-items: center;
    gap: var(--s1);
    white-space: nowrap;
  }

  /* ---- rows ---- */
  .block {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--s2);
    height: 32px;
    color: var(--muted);
  }
  .head h2 {
    margin: 0;
    color: var(--fg);
  }
  .count {
    color: var(--faint);
  }
  .pg {
    margin-left: auto;
    display: flex;
    gap: var(--s1);
    opacity: 0;
    transition: opacity 0.2s;
  }
  .block:hover .pg {
    opacity: 1;
  }
  .block:global([data-start="true"][data-end="true"]) .pg {
    visibility: hidden;
  }
  .pb {
    width: 32px;
    height: 32px;
    display: grid;
    place-items: center;
    color: var(--fg);
  }
  :global(:root[data-theme="pixel"]) .pb {
    border: 2px solid var(--line);
  }
  :global(:root[data-theme="pixel"]) .pb:hover {
    background: var(--hi-bg);
    color: var(--hi-fg);
  }
  :global(:root[data-theme="paper"]) .pb {
    border-radius: 50%;
    box-shadow: inset 0 0 0 1px var(--line);
  }
  :global(:root[data-theme="paper"]) .pb:hover {
    background: var(--fg);
    color: var(--panel);
  }
  .block:global([data-start="true"]) .prev,
  .block:global([data-end="true"]) .next {
    opacity: 0.25;
    pointer-events: none;
  }
  .lane {
    display: flex;
    align-items: flex-start;
    gap: var(--s5);
    overflow-x: auto;
    overflow-y: hidden;
    margin: 0 calc(-1 * var(--gutter));
    padding: var(--s4) var(--gutter) var(--s4);
    scroll-padding: 0 var(--gutter);
  }

  /* ---- cards (systems, browser games, "more") ---- */
  .card {
    flex: none;
    width: var(--card);
    display: flex;
    flex-direction: column;
    gap: var(--s3);
    text-align: left;
    animation: rise 0.45s var(--ease) both;
    animation-delay: calc(var(--i, 0) * 30ms);
  }
  .card.tall {
    width: var(--tile);
  }
  .pic {
    position: relative;
    aspect-ratio: 3 / 2;
    display: grid;
    place-items: center;
    overflow: hidden;
  }
  .tall .pic {
    aspect-ratio: 3 / 4;
  }
  .sa {
    display: block;
    width: 72%;
    aspect-ratio: 3 / 2;
  }
  /* Pixel console art stays on whole-pixel multiples of its 48x32 grid. */
  :global(:root[data-theme="pixel"]) .sa {
    width: round(down, calc(100% - 40px), 48px);
  }
  .cap {
    display: flex;
    flex-direction: column;
    gap: var(--s1);
    min-width: 0;
  }
  .nm {
    font-family: var(--text);
    font-size: 15px;
    font-weight: 600;
    line-height: 1.25;
    min-height: 2.5em;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: var(--s2);
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* .meta is a flex row, where text-overflow has no effect: the text carries its own. */
  .ell {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .playme {
    position: absolute;
    left: var(--s2);
    bottom: var(--s2);
    width: 36px;
    height: 36px;
    display: grid;
    place-items: center;
    opacity: 0;
  }
  .card:is(:hover, :focus-visible) .playme {
    opacity: 1;
  }

  /* pixel: framed in white; focus draws the marching ants and inverts the picture */
  :global(:root[data-theme="pixel"]) .card:focus-visible {
    background: none;
    color: var(--fg);
  }
  :global(:root[data-theme="pixel"]) .pic {
    outline: var(--border) solid var(--line);
    outline-offset: calc(-1 * var(--border));
    background: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .pic.more {
    outline-style: dashed;
    color: var(--faint);
  }
  :global(:root[data-theme="pixel"]) .card:is(:hover, :focus-visible) .pic.sys,
  :global(:root[data-theme="pixel"]) .card:is(:hover, :focus-visible) .pic.more {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .card:focus-visible .pic {
    animation: pxgrow 0.16s steps(2) both;
  }
  :global(:root[data-theme="pixel"]) .card:focus-visible .pic::after {
    content: "";
    position: absolute;
    inset: 0;
    background:
      linear-gradient(90deg, var(--fg) 50%, transparent 0) 0 0 / 8px 3px repeat-x,
      linear-gradient(90deg, var(--fg) 50%, transparent 0) 0 100% / 8px 3px repeat-x,
      linear-gradient(0deg, var(--fg) 50%, transparent 0) 0 0 / 3px 8px repeat-y,
      linear-gradient(0deg, var(--fg) 50%, transparent 0) 100% 0 / 3px 8px repeat-y;
    animation: ants 0.5s steps(2) infinite;
    pointer-events: none;
  }
  :global(:root[data-theme="pixel"]) .card:focus-visible .pic.sys::after,
  :global(:root[data-theme="pixel"]) .card:focus-visible .pic.more::after {
    background:
      linear-gradient(90deg, var(--bg) 50%, transparent 0) 0 0 / 8px 3px repeat-x,
      linear-gradient(90deg, var(--bg) 50%, transparent 0) 0 100% / 8px 3px repeat-x,
      linear-gradient(0deg, var(--bg) 50%, transparent 0) 0 0 / 3px 8px repeat-y,
      linear-gradient(0deg, var(--bg) 50%, transparent 0) 100% 0 / 3px 8px repeat-y;
  }
  :global(:root[data-theme="pixel"]) .card:is(:hover, :focus-visible) .nm span {
    background: var(--fg);
    color: var(--bg);
    box-shadow:
      4px 0 0 var(--fg),
      -4px 0 0 var(--fg);
    -webkit-box-decoration-break: clone;
    box-decoration-break: clone;
  }
  :global(:root[data-theme="pixel"]) .playme {
    background: var(--fg);
    color: var(--bg);
  }
  @keyframes pxgrow {
    from {
      transform: scale(0.94);
    }
  }

  /* paper: cards on the page, lifted with an ink shadow on focus */
  :global(:root[data-theme="paper"]) .pic {
    background: var(--panel);
    box-shadow: 0 0 0 1px var(--line);
    transition:
      transform 0.35s var(--ease),
      box-shadow 0.35s var(--ease);
  }
  :global(:root[data-theme="paper"]) .pic.more {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px color-mix(in srgb, var(--line) 45%, transparent);
    color: var(--faint);
  }
  :global(:root[data-theme="paper"]) .card:focus-visible::after {
    display: none;
  }
  :global(:root[data-theme="paper"]) .card:is(:hover, :focus-visible) .pic {
    transform: translateY(-6px);
    box-shadow:
      0 0 0 1.5px var(--line),
      6px 6px 0 var(--line);
    color: var(--fg);
  }
  :global(:root[data-theme="paper"]) .card:is(:hover, :focus-visible) :global(.art) {
    filter: none;
  }
  :global(:root[data-theme="paper"]) .nm span {
    background: linear-gradient(var(--fg), var(--fg)) 0 100% / 0 1.5px no-repeat;
    transition: background-size 0.35s var(--ease);
    -webkit-box-decoration-break: clone;
    box-decoration-break: clone;
  }
  :global(:root[data-theme="paper"]) .card:is(:hover, :focus-visible) .nm span {
    background-size: 100% 1.5px;
  }
  :global(:root[data-theme="paper"]) .card:focus-visible .nm {
    color: var(--accent-2);
  }
  :global(:root[data-theme="paper"]) .card:focus-visible .nm span {
    background-image: linear-gradient(var(--accent-2), var(--accent-2));
  }
  :global(:root[data-theme="paper"]) .warn {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .playme {
    background: var(--fg);
    color: var(--panel);
    border-radius: 50%;
  }
</style>
