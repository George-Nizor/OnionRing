<script lang="ts">
  import { tick } from "svelte";
  import { api, type Game } from "$lib/api";
  import { app, SORT_LABEL, type Sort } from "$lib/store.svelte";
  import type { PadAction } from "$lib/nav";
  import Glyph from "./Glyph.svelte";
  import LibGlyph from "./LibGlyph.svelte";
  import SystemArt from "./SystemArt.svelte";
  import Tile from "./Tile.svelte";
  import { collections, collectionFilter, filterCollection } from "./collections.svelte";

  collections.start();

  // Same orders as the store's library.
  const SORTS: Record<Sort, (a: Game, b: Game) => number> = {
    title: (a, b) => a.title.localeCompare(b.title),
    recent: (a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || a.title.localeCompare(b.title),
    playtime: (a, b) => b.playtime - a.playtime || a.title.localeCompare(b.title),
    added: (a, b) => b.addedAt - a.addedAt || a.title.localeCompare(b.title),
    size: (a, b) => b.size - a.size,
  };

  const visible = $derived(new Set(app.visibleGames.map((g) => g.id)));
  const favCount = $derived(app.visibleGames.filter((g) => g.favorite).length);
  const colTabs = $derived(
    collections.list.map((c) => ({ id: collectionFilter(c.id), col: c, label: c.name, n: c.gameIds.filter((id) => visible.has(id)).length })),
  );
  const sysTabs = $derived(
    [...app.counts.entries()]
      .map(([id, n]) => ({ id, label: id === "port" ? "PC ports" : (app.platforms.get(id)?.name ?? id), short: id === "port" ? "PC" : (app.platforms.get(id)?.short ?? id), n }))
      .sort((a, b) => a.label.localeCompare(b.label)),
  );
  /** Every filter in sidebar order, for Q/E and the bumpers. */
  const order = $derived(["all", "favorites", ...colTabs.map((t) => t.id), ...sysTabs.map((t) => t.id)]);

  const f = $derived(app.platformFilter);
  const colId = $derived(filterCollection(f));
  const col = $derived(colId === null ? undefined : collections.byId(colId));
  // A deleted collection or a system with no games left falls back to everything.
  const known = $derived(f === "all" || f === "favorites" || (colId !== null ? !!col || !collections.loaded : app.counts.has(f) || !app.loaded));
  $effect(() => {
    if (!known) app.setPlatformFilter("all");
  });

  const games = $derived.by(() => {
    if (colId === null) return app.library;
    if (!col) return [];
    const ids = new Set(col.gameIds);
    return app.visibleGames.filter((g) => ids.has(g.id)).sort(SORTS[app.sort]);
  });
  const sys = $derived(sysTabs.find((t) => t.id === f));
  const heading = $derived(f === "all" ? "All games" : f === "favorites" ? "Favourites" : col ? col.name : (sys?.label ?? ""));

  function cycle(dir: number) {
    const i = Math.max(0, order.indexOf(f));
    app.setPlatformFilter(order[(i + dir + order.length) % order.length]);
  }
  $effect(() => {
    const onpad = (e: Event) => {
      const a = (e as CustomEvent<PadAction>).detail;
      if (a === "lb") cycle(-1);
      if (a === "rb") cycle(1);
    };
    const onkey = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement)?.closest("input")) return;
      if (e.key === "q") cycle(-1);
      if (e.key === "e") cycle(1);
    };
    window.addEventListener("pad", onpad);
    window.addEventListener("keydown", onkey);
    return () => {
      window.removeEventListener("pad", onpad);
      window.removeEventListener("keydown", onkey);
    };
  });

  // ---- new collection ------------------------------------------------------------------------
  let creating = $state(false);
  let newName = $state("");
  let newInput = $state<HTMLInputElement | null>(null);
  async function startCreate() {
    creating = true;
    newName = "";
    await tick();
    newInput?.focus();
  }
  async function create() {
    const name = newName.trim();
    if (!name) {
      creating = false;
      return;
    }
    try {
      const c = await api.createCollection(name);
      creating = false;
      await collections.load();
      app.setPlatformFilter(collectionFilter(c.id));
    } catch (e) {
      app.toast(String(e), "error");
      newInput?.focus();
    }
  }

  // ---- rename / delete -----------------------------------------------------------------------
  let renaming = $state(false);
  let rename = $state("");
  let renameInput = $state<HTMLInputElement | null>(null);
  let confirming = $state(false);
  $effect(() => {
    void f;
    renaming = false;
    confirming = false;
  });
  async function startRename() {
    if (!col) return;
    rename = col.name;
    renaming = true;
    await tick();
    renameInput?.select();
  }
  async function saveRename() {
    if (!col || !renaming) return;
    const name = rename.trim();
    renaming = false;
    if (!name || name === col.name) return;
    try {
      await api.renameCollection(col.id, name);
      await collections.load();
    } catch (e) {
      app.toast(String(e), "error");
    }
  }
  async function remove() {
    if (!col) return;
    const name = col.name;
    try {
      await api.deleteCollection(col.id);
      app.setPlatformFilter("all");
      await collections.load();
      app.toast(`Deleted ${name}. The games are still in your library.`, "ok");
    } catch (e) {
      app.toast(String(e), "error");
    }
  }
  /** Enter saves, Escape cancels without leaving the screen. */
  function keys(e: KeyboardEvent, save: () => void, cancel: () => void) {
    if (e.key === "Enter") {
      e.preventDefault();
      save();
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      cancel();
    }
  }
</script>

<div class="lib">
  <aside class="side">
    <nav class="group">
      <button class="item" class:on={f === "all"} data-nav data-nav-start={f === "all" || undefined} onclick={() => app.setPlatformFilter("all")}>
        <span class="ico"><Glyph name="library" size={24} /></span><span class="nm">All games</span><em>{app.visibleGames.length}</em>
      </button>
      <button class="item" class:on={f === "favorites"} data-nav data-nav-start={f === "favorites" || undefined} onclick={() => app.setPlatformFilter("favorites")}>
        <span class="ico"><Glyph name="star" size={24} /></span><span class="nm">Favourites</span><em>{favCount}</em>
      </button>
    </nav>

    <div class="group">
      <h2 class="gh label">Collections</h2>
      {#each colTabs as t (t.id)}
        <button class="item" class:on={f === t.id} data-nav data-nav-start={f === t.id || undefined} onclick={() => app.setPlatformFilter(t.id)}>
          <span class="ico"><LibGlyph name={f === t.id ? "bookmark-on" : "bookmark"} size={24} /></span><span class="nm">{t.label}</span><em>{t.n}</em>
        </button>
      {/each}
      {#if creating}
        <div class="item new">
          <span class="ico"><LibGlyph name="bookmark" size={24} /></span>
          <input
            bind:this={newInput}
            bind:value={newName}
            data-nav
            maxlength="80"
            spellcheck="false"
            placeholder="Name it"
            aria-label="New collection name"
            onkeydown={(e) => keys(e, create, () => (creating = false))}
            onblur={() => !newName.trim() && (creating = false)}
          />
          <button class="ok" data-nav onclick={create} aria-label="Create collection"><Glyph name="check" size={24} /></button>
        </div>
      {:else}
        <button class="item add" data-nav onclick={startCreate}>
          <span class="ico"><Glyph name="plus" size={24} /></span><span class="nm">New collection</span>
        </button>
      {/if}
    </div>

    {#if sysTabs.length}
      <div class="group">
        <h2 class="gh label">Systems</h2>
        {#each sysTabs as t (t.id)}
          <button class="item sys" class:on={f === t.id} data-nav data-nav-start={f === t.id || undefined} onclick={() => app.setPlatformFilter(t.id)} title={t.label}>
            <span class="ico art"><SystemArt id={t.id} label={t.label} /></span><span class="nm">{t.label}</span><em>{t.n}</em>
          </button>
        {/each}
      </div>
    {/if}
  </aside>

  <section class="main">
    {#key f}
      <header class="head">
        <div class="emblem" class:wide={!!sys} aria-hidden="true">
          {#if sys}<SystemArt id={sys.id} />
          {:else if col}<LibGlyph name="bookmark-on" size={36} />
          {:else}<Glyph name={f === "favorites" ? "star" : "library"} size={36} />{/if}
        </div>
        <div class="titles">
          {#if renaming}
            <input
              class="rename"
              bind:this={renameInput}
              bind:value={rename}
              data-nav
              maxlength="80"
              spellcheck="false"
              aria-label="Collection name"
              onkeydown={(e) => keys(e, saveRename, () => (renaming = false))}
              onblur={saveRename}
            />
          {:else}
            <h1>{heading}</h1>
          {/if}
          <span class="sub label">{games.length} {games.length === 1 ? "game" : "games"}{col ? " in this collection" : ""}</span>
        </div>
        <div class="tools">
          {#if col}
            <button class="btn icon" data-nav onclick={startRename} aria-label="Rename collection" title="Rename"><LibGlyph name="pencil" size={24} /></button>
            <button class="btn icon" class:on={confirming} data-nav onclick={() => (confirming = !confirming)} aria-label="Delete collection" title="Delete"><LibGlyph name="trash" size={24} /></button>
          {/if}
          {#if sys}
            <button
              class="btn icon"
              data-nav
              onclick={() => app.go(sys.id === "port" ? { name: "get", tab: "ports" } : { name: "get", tab: "find", item: sys.label })}
              aria-label="Find more {sys.label}"
              title="Find more"><Glyph name="search" size={24} /></button>
          {/if}
          <button class="btn sort" data-nav onclick={() => app.cycleSort()} title="Change order">
            <Glyph name="sliders" size={24} />
            {#key app.sort}<span class="s">{SORT_LABEL[app.sort]}</span>{/key}
          </button>
        </div>
      </header>
      {#if col && confirming}
        <div class="confirm">
          <LibGlyph name="trash" size={24} />
          <span>Delete <b>{col.name}</b>? The games stay in your library.</span>
          <button class="btn danger" data-nav onclick={remove}>Delete</button>
          <button class="btn" data-nav onclick={() => (confirming = false)}>Keep it</button>
        </div>
      {/if}
    {/key}

    {#if games.length}
      {#key f + app.sort}
        <div class="grid">
          {#each games as game, i (game.id)}
            <Tile {game} index={i} width={0} title="always" />
          {/each}
        </div>
      {/key}
    {:else if app.loaded && (colId === null || collections.loaded)}
      <div class="empty">
        <div class="mark" aria-hidden="true">
          {#if col}<LibGlyph name="bookmark" size={48} />{:else if f === "favorites"}<Glyph name="star" size={48} />{:else}<Glyph name="library" size={48} />{/if}
        </div>
        {#if col}
          <h2>Nothing in {col.name} yet</h2>
          <p>Open a game and press the bookmark next to Play to put it here.</p>
          <button class="btn primary" data-nav onclick={() => app.setPlatformFilter("all")}><Glyph name="library" size={24} />Pick from all games</button>
        {:else if f === "favorites"}
          <h2>No favourites yet</h2>
          <p>Press the star on a game's page and it shows up here.</p>
          <button class="btn primary" data-nav onclick={() => app.setPlatformFilter("all")}><Glyph name="library" size={24} />Pick from all games</button>
        {:else}
          <h2>No games yet</h2>
          <p>Tell OnionRing where your games are and they show up here.</p>
          <button class="btn primary" data-nav onclick={() => app.go({ name: "settings" })}><Glyph name="folder" size={24} />Add a folder</button>
        {/if}
      </div>
    {/if}
  </section>
</div>

<style>
  .lib {
    --side: clamp(236px, 17vw, 292px);
    --pad: clamp(32px, 3.4vw, 64px);
    height: 100%;
    display: grid;
    grid-template-columns: var(--side) minmax(0, 1fr);
  }

  /* ---- sidebar ---- */
  .side {
    overflow-y: auto;
    padding: 28px 12px 40px clamp(14px, 1.4vw, 28px);
    display: flex;
    flex-direction: column;
    gap: 28px;
  }
  :global(:root[data-theme="pixel"]) .side {
    border-right: var(--border) solid var(--line);
  }
  :global(:root[data-theme="paper"]) .side {
    border-right: 1px solid color-mix(in srgb, var(--line) 22%, transparent);
    background: color-mix(in srgb, var(--panel) 55%, transparent);
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .gh {
    margin: 0 0 8px;
    padding: 0 12px;
    color: var(--faint);
    font-weight: 400;
  }
  .item {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    height: 40px;
    padding: 0 12px 0 6px;
    text-align: left;
    font-family: var(--title);
    font-size: 16px;
    font-weight: 600;
    transition: background 0.12s;
  }
  .ico {
    width: 32px;
    height: 24px;
    display: grid;
    place-items: center;
  }
  .ico.art {
    color: currentColor;
  }
  .nm {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .item em {
    font-style: normal;
    font-family: var(--label);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--faint);
    min-width: 2ch;
    text-align: right;
  }
  .item.on em,
  .item:focus-visible em {
    color: inherit;
  }
  .add {
    color: var(--faint);
  }
  .new {
    grid-template-columns: 36px minmax(0, 1fr) 36px;
    padding-right: 2px;
  }
  .new input {
    min-width: 0;
    height: 32px;
    padding: 0 8px;
    background: var(--bg);
    font-family: var(--title);
    font-size: 16px;
  }
  .ok {
    width: 36px;
    height: 36px;
    display: grid;
    place-items: center;
  }

  /* pixel: invert when chosen, a box on hover */
  :global(:root[data-theme="pixel"]) .item:hover:not(.on):not(:focus-visible):not(.new) {
    box-shadow: inset 0 0 0 2px var(--fg);
  }
  :global(:root[data-theme="pixel"]) .item:focus-visible:not(.on) {
    background: none;
    color: var(--fg);
    box-shadow: inset 0 0 0 var(--border) var(--fg);
  }
  :global(:root[data-theme="pixel"]) .item.on:focus-visible::after {
    content: "";
    position: absolute;
    inset: -5px;
    border: 2px dashed var(--fg);
    animation: blink 0.8s steps(1) infinite;
  }
  :global(:root[data-theme="pixel"]) .item {
    font-size: 15px;
  }
  :global(:root[data-theme="pixel"]) .new input {
    border: 2px solid var(--fg);
  }
  :global(:root[data-theme="pixel"]) .ok:focus-visible,
  :global(:root[data-theme="pixel"]) .ok:hover {
    background: var(--fg);
    color: var(--bg);
  }

  /* paper: a red bookmark ribbon on the chosen one, ink tint on hover */
  :global(:root[data-theme="paper"]) .item:hover,
  :global(:root[data-theme="paper"]) .item:focus-visible {
    background: color-mix(in srgb, var(--fg) 7%, transparent);
  }
  :global(:root[data-theme="paper"]) .item.on {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 9%, transparent);
  }
  :global(:root[data-theme="paper"]) .item.on::before {
    content: "";
    position: absolute;
    left: -4px;
    top: 6px;
    bottom: 6px;
    width: 3px;
    background: var(--accent);
    animation: ribbon 0.35s var(--ease) both;
    transform-origin: top;
  }
  @keyframes ribbon {
    from {
      transform: scaleY(0);
    }
  }
  :global(:root[data-theme="paper"]) .item:focus-visible::after {
    left: 48px;
    right: 12px;
    bottom: 5px;
    height: 2px;
  }
  :global(:root[data-theme="paper"]) .ico.art {
    color: var(--fg);
  }
  :global(:root[data-theme="paper"]) .item.on .ico.art {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .new input {
    border: 1.5px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
  }
  :global(:root[data-theme="paper"]) .ok:hover,
  :global(:root[data-theme="paper"]) .ok:focus-visible {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .ok:focus-visible::after {
    display: none;
  }

  /* ---- main ---- */
  .main {
    overflow-y: auto;
    padding: 28px var(--pad) 64px;
  }
  .head {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 20px;
    margin-bottom: 32px;
    min-height: 64px;
  }
  .emblem {
    width: 56px;
    height: 56px;
    display: grid;
    place-items: center;
  }
  .emblem.wide {
    width: 84px;
  }
  .titles {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  h1,
  .rename {
    margin: 0;
    font-family: var(--title);
    font-weight: 800;
    font-size: clamp(30px, 2.6vw, 44px);
    line-height: 1.05;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rename {
    width: 100%;
    max-width: 720px;
    height: 1.25em;
    padding: 0 8px;
    margin-left: -8px;
    background: none;
  }
  :global(:root[data-theme="pixel"]) .rename {
    border: 2px solid var(--fg);
  }
  :global(:root[data-theme="paper"]) .rename {
    border: 0;
    border-bottom: 2px solid var(--accent);
  }
  .sub {
    color: var(--faint);
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .confirm {
    display: flex;
    align-items: center;
    gap: 14px;
    margin: -12px 0 32px;
    padding: 10px 10px 10px 18px;
    font-family: var(--label);
    font-size: 15px;
    animation: rise 0.25s var(--ease) both;
  }
  .confirm span {
    flex: 1;
    min-width: 0;
  }
  :global(:root[data-theme="pixel"]) .confirm {
    border: var(--border) solid var(--line);
    animation-timing-function: steps(3);
  }
  :global(:root[data-theme="paper"]) .confirm {
    border-left: 3px solid var(--accent);
    background: color-mix(in srgb, var(--accent) 8%, var(--panel));
  }
  :global(:root[data-theme="paper"]) .confirm > :global(svg) {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .btn.danger {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--panel);
  }
  :global(:root[data-theme="pixel"]) .btn.danger {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .tools .btn:hover:not(:focus-visible) {
    outline: 2px solid var(--fg);
    outline-offset: 3px;
  }
  :global(:root[data-theme="paper"]) .tools .btn:hover:not(:focus-visible) {
    box-shadow: 3px 3px 0 var(--fg);
    transform: translate(-1px, -1px);
  }
  .s {
    display: inline-block;
    min-width: 7.5em;
    animation: rise 0.3s var(--ease) both;
  }
  :global(:root[data-theme="pixel"]) .s {
    animation: flip 0.2s steps(2) both;
  }
  @keyframes flip {
    from {
      transform: translateY(-100%);
      opacity: 0;
    }
  }

  /* heading motion: typewriter in pixel, an e-ink settle in paper */
  :global(:root[data-theme="pixel"]) h1 {
    animation: type 0.36s steps(9) both;
  }
  @keyframes type {
    from {
      clip-path: inset(0 100% 0 0);
    }
  }
  :global(:root[data-theme="paper"]) .head {
    animation: eink 0.42s steps(3) both;
  }
  @keyframes eink {
    0% {
      opacity: 0.15;
      filter: invert(1);
    }
    40% {
      opacity: 1;
      filter: invert(0.15);
    }
  }
  :global(:root[data-theme="pixel"]) .emblem {
    animation: pop 0.24s steps(3) both;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(clamp(140px, 10.6vw, 188px), 1fr));
    gap: 30px 24px;
  }
  .grid :global(.tile) {
    width: auto;
  }
  /* Names always show, up to two lines, the same height on every tile. */
  .grid :global(.tile .name) {
    white-space: normal;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    line-height: 1.3;
    min-height: 2.6em;
    overflow: hidden;
  }
  .grid :global(.tile .name span) {
    -webkit-box-decoration-break: clone;
    box-decoration-break: clone;
  }

  /* ---- empty ---- */
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
    max-width: 460px;
    padding: 56px 0;
    animation: rise 0.4s var(--ease) both;
  }
  .mark {
    margin-bottom: 8px;
    color: var(--faint);
  }
  :global(:root[data-theme="paper"]) .mark {
    color: var(--accent);
  }
  .empty h2 {
    margin: 0;
    font-family: var(--title);
    font-weight: 800;
    font-size: 26px;
  }
  .empty p {
    margin: 0 0 12px;
    font-family: var(--label);
    font-size: 15px;
    line-height: 1.5;
    color: var(--muted);
  }
  :global(:root[data-theme="pixel"]) .empty p {
    color: var(--faint);
  }
</style>
