<script lang="ts">
  import { api, type BrowserGame, type Emulator } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { bytes, playtime } from "$lib/format";
  import Art from "./Art.svelte";
  import Glyph from "./Glyph.svelte";
  import Tile from "./Tile.svelte";

  let { emulators, portCount = 0, browser = [] }: { emulators: Emulator[]; portCount?: number; browser?: BrowserGame[] } = $props();
  // Ready to play with nothing to supply first.
  const instant = $derived(browser.filter((b) => !b.needsFiles && b.embed === "window").slice(0, 12));

  const g = $derived(app.spotlight);
  const platform = $derived(g ? app.platforms.get(g.platform) : undefined);
  const emu = $derived(emulators.find((e) => e.id === g?.emulator));
  const running = $derived(g ? app.running.includes(g.id) : false);

  const systems = $derived(
    [...app.counts.entries()]
      .filter(([id]) => id !== "port")
      .map(([id, n]) => ({ p: app.platforms.get(id)!, n, ok: emulators.some((e) => e.platforms.includes(id) && !e.missing) }))
      .filter((s) => s.p)
      .sort((a, b) => b.n - a.n),
  );
  const ports = $derived(app.visibleGames.filter((x) => x.kind === "port"));
</script>

<div class="home">
  {#if g}
    <section class="spot">
      {#key g.id}
        <div class="info">
          <div class="kicker">
            <span class="plat label">{g.kind === "port" ? "PC PORT" : (platform?.short ?? "")}</span>
            {#if g.packed}<span class="tag"><Glyph name="zip" size={24} /></span>{/if}
            {#if g.favorite}<span class="tag"><Glyph name="star" size={24} /></span>{/if}
          </div>
          <h1 class="t"><span>{g.title}</span></h1>
          <div class="stats">
            <span class="stat"><Glyph name="clock" size={24} label="Played" /><b>{g.playtime ? playtime(g.playtime) : "0h"}</b></span>
            <span class="stat"><Glyph name="disk" size={24} label="Size" /><b>{bytes(g.size)}</b></span>
            {#if g.kind === "rom"}
              <span class="stat"><Glyph name="pad" size={24} label="Emulator" /><b>{emu?.name ?? "None yet"}</b></span>
            {/if}
          </div>
          <div class="actions">
            <button class="btn big primary" data-nav onclick={() => app.play(g)} disabled={g.missing || running}>
              <Glyph name={g.packed ? "zip" : "play"} size={24} />
              {running ? "Playing" : g.packed ? "Unzip" : "Play"}
            </button>
            <button class="btn big icon" data-nav onclick={() => app.openGame(g)} aria-label="Open game page"><Glyph name="info" size={24} /></button>
            <button class="btn big icon" data-nav onclick={() => app.toggleFavorite(g)} aria-label="Favourite"><Glyph name="star" size={24} /></button>
          </div>
        </div>
        <div class="wide"><Art game={g} kind="wide" /></div>
      {/key}
    </section>
  {/if}

  <section class="rows">
    {#if app.continueList.length}
      <div class="row-block">
        <div class="row-head"><Glyph name="clock" size={24} /><span class="label">{app.continueList.some((x) => x.lastPlayed) ? "Jump back in" : "Newest"}</span></div>
        <div class="row">
          {#each app.continueList as game, i (game.id)}
            <Tile {game} index={i} width={150} onfocus={() => (app.spotlightId = game.id)} />
          {/each}
        </div>
      </div>
    {/if}

    {#if systems.length}
      <div class="row-block">
        <div class="row-head"><Glyph name="pad" size={24} /><span class="label">Systems</span></div>
        <div class="row">
          {#each systems as s, i (s.p.id)}
            <button
              class="sys"
              data-nav
              style:--i={i}
              onclick={() => { app.setPlatformFilter(s.p.id); app.go({ name: "library" }); }}
              onmouseenter={(e) => (e.currentTarget as HTMLElement).focus({ preventScroll: true })}
              aria-label="{s.p.name}, {s.n} games"
            >
              <span class="big">{s.p.short.replace("WII", "Wii")}</span>
              <span class="meta label"><Glyph name="library" size={12} />{s.n}<Glyph name={s.ok ? "check" : "alert"} size={12} /></span>
            </button>
          {/each}
        </div>
      </div>
    {/if}

    <div class="row-block">
      <div class="row-head"><Glyph name="cart" size={24} /><span class="label">Ports</span></div>
      <div class="row">
        {#each ports as game, i (game.id)}
          <Tile {game} index={i} width={150} onfocus={() => (app.spotlightId = game.id)} />
        {/each}
        <button
          class="more"
          data-nav
          onclick={() => app.go({ name: "get", tab: "ports" })}
          onmouseenter={(e) => (e.currentTarget as HTMLElement).focus({ preventScroll: true })}
          aria-label="Get more ports"
        >
          <Glyph name="plus" size={48} />
          <span class="label">{portCount || ""} more</span>
        </button>
      </div>
    </div>

    {#if instant.length}
      <div class="row-block">
        <div class="row-head"><Glyph name="globe" size={24} /><span class="label">Play in a window, no install</span></div>
        <div class="row">
          {#each instant as b, i (b.id)}
            <button
              class="sys web"
              data-nav
              style:--i={i}
              onclick={() => api.openBrowserGame(b.id).catch((e) => app.toast(String(e), "error"))}
              onmouseenter={(e) => (e.currentTarget as HTMLElement).focus({ preventScroll: true })}
              aria-label="Play {b.title}"
            >
              <span class="wt">{b.title}</span>
              <span class="meta label"><Glyph name="play" size={12} />{b.original?.replace(/\s*\(.*\)/, "") ?? ""}</span>
            </button>
          {/each}
          <button class="more" data-nav onclick={() => app.go({ name: "get", tab: "browser" })} onmouseenter={(e) => (e.currentTarget as HTMLElement).focus({ preventScroll: true })} aria-label="All browser games">
            <Glyph name="plus" size={48} />
            <span class="label">{browser.length} in all</span>
          </button>
        </div>
      </div>
    {/if}
  </section>
</div>

<style>
  .home {
    height: 100%;
    overflow-y: auto;
    padding: 30px 56px 40px;
    display: flex;
    flex-direction: column;
    gap: 30px;
  }
  /* ---- spotlight ---- */
  .spot {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(320px, 46%);
    gap: 48px;
    align-items: end;
    min-height: 300px;
  }
  .info {
    min-width: 0;
  }
  .kicker {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 14px;
  }
  .plat {
    padding: 4px 10px;
    font-size: 14px;
  }
  :global(:root[data-theme="pixel"]) .plat {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="paper"]) .plat {
    border: 1.5px solid var(--line);
    color: var(--accent);
    border-color: var(--accent);
  }
  .t {
    margin: 0;
    font-family: var(--title);
    font-weight: 800;
    font-size: clamp(40px, 4.6vw, 72px);
    line-height: 1;
    text-wrap: balance;
  }
  :global(:root[data-theme="pixel"]) .t span {
    animation: typing 0.5s steps(12) both;
    display: inline-block;
  }
  :global(:root[data-theme="pixel"]) .t::after {
    content: "█";
    font-size: 0.8em;
    margin-left: 8px;
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
    gap: 12px 30px;
    margin: 20px 0 26px;
  }
  .stat {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: var(--label);
    font-size: 17px;
  }
  .stat b {
    font-weight: 700;
  }
  .actions {
    display: flex;
    gap: 12px;
  }
  .wide {
    position: relative;
    aspect-ratio: 16 / 9;
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

  /* ---- rows ---- */
  .rows {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .sys,
  .more {
    flex: none;
    width: 150px;
    height: 150px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    align-items: flex-start;
    padding: 14px;
    text-align: left;
    animation: rise 0.45s var(--ease) both;
    animation-delay: calc(var(--i, 0) * 30ms);
  }
  .sys .big {
    font-family: var(--title);
    font-weight: 800;
    font-size: 44px;
    line-height: 1;
  }
  .sys.web {
    width: 190px;
  }
  .wt {
    font-family: var(--title);
    font-weight: 800;
    font-size: 21px;
    line-height: 1.05;
  }
  .sys .meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 14px;
  }
  .more {
    justify-content: center;
    align-items: center;
    gap: 10px;
  }
  :global(:root[data-theme="pixel"]) .sys,
  :global(:root[data-theme="pixel"]) .more {
    border: var(--border) solid var(--line);
  }
  :global(:root[data-theme="pixel"]) .more {
    border-style: dashed;
  }
  :global(:root[data-theme="pixel"]) .sys:focus-visible,
  :global(:root[data-theme="pixel"]) .more:focus-visible {
    animation: pxgrow2 0.16s steps(2);
  }
  @keyframes pxgrow2 {
    from {
      transform: scale(0.92);
    }
  }
  :global(:root[data-theme="paper"]) .sys,
  :global(:root[data-theme="paper"]) .more {
    background: var(--panel);
    box-shadow: 0 0 0 1px var(--line);
    transition: transform 0.3s var(--ease), box-shadow 0.3s;
  }
  :global(:root[data-theme="paper"]) .more {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px var(--line);
    border: 0;
  }
  :global(:root[data-theme="paper"]) .sys:focus-visible,
  :global(:root[data-theme="paper"]) .more:focus-visible {
    transform: translateY(-6px);
    box-shadow:
      0 0 0 2px var(--line),
      6px 6px 0 var(--line);
  }
  :global(:root[data-theme="paper"]) .sys:focus-visible::after,
  :global(:root[data-theme="paper"]) .more:focus-visible::after {
    display: none;
  }
  :global(:root[data-theme="paper"]) .sys:focus-visible .big {
    color: var(--accent-2);
  }
</style>
