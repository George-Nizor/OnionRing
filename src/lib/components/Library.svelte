<script lang="ts">
  import { app, SORT_LABEL } from "$lib/store.svelte";
  import type { PadAction } from "$lib/nav";
  import Glyph from "./Glyph.svelte";
  import Tile from "./Tile.svelte";

  const tabs = $derived([
    { id: "all", label: "All", icon: "library", n: app.visibleGames.length },
    ...(app.visibleGames.some((g) => g.favorite) ? [{ id: "favorites", label: "", icon: "star", n: app.visibleGames.filter((g) => g.favorite).length }] : []),
    ...[...app.counts.entries()]
      .map(([id, n]) => ({ id, label: id === "port" ? "PC" : (app.platforms.get(id)?.short ?? id).replace("WII", "Wii"), icon: "", n }))
      .sort((a, b) => a.label.localeCompare(b.label)),
  ]);

  function cycle(dir: number) {
    const i = Math.max(0, tabs.findIndex((t) => t.id === app.platformFilter));
    app.setPlatformFilter(tabs[(i + dir + tabs.length) % tabs.length].id);
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
</script>

<div class="lib">
  <div class="strip">
    <div class="tabs">
      {#each tabs as t (t.id)}
        <button class="tab" class:on={app.platformFilter === t.id} data-nav onclick={() => app.setPlatformFilter(t.id)} aria-label="{t.label || 'Favourites'}: {t.n}">
          {#if t.icon}<Glyph name={t.icon} size={24} />{/if}
          {#if t.label}<span>{t.label}</span>{/if}
          <em>{t.n}</em>
        </button>
      {/each}
    </div>
    <button class="btn sort" data-nav onclick={() => app.cycleSort()}>
      <Glyph name="sliders" size={24} />
      {#key app.sort}<span class="s">{SORT_LABEL[app.sort]}</span>{/key}
    </button>
  </div>

  {#key app.platformFilter + app.sort}
    <div class="grid">
      {#each app.library as game, i (game.id)}
        <Tile {game} index={i} width={0} title="always" />
      {/each}
    </div>
  {/key}
</div>

<style>
  .lib {
    height: 100%;
    overflow-y: auto;
    padding: 24px 56px 60px;
  }
  .strip {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    margin-bottom: 26px;
  }
  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    padding: 0 14px;
    font-family: var(--title);
    font-weight: 700;
    font-size: 19px;
  }
  .tab em {
    font-style: normal;
    font-family: var(--label);
    font-size: 12px;
    opacity: 0.65;
  }
  :global(:root[data-theme="pixel"]) .tab {
    border: var(--border) solid transparent;
  }
  :global(:root[data-theme="pixel"]) .tab.on {
    border-color: var(--line);
  }
  :global(:root[data-theme="paper"]) .tab.on {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .tab.on::before {
    content: "";
    position: absolute;
    left: 14px;
    right: 14px;
    bottom: 2px;
    height: 2px;
    background: var(--accent);
  }
  .s {
    display: inline-block;
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
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 30px 24px;
  }
  .grid :global(.tile) {
    width: auto;
  }
</style>
