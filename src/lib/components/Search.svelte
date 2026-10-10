<script lang="ts">
  import { tick } from "svelte";
  import { app } from "$lib/store.svelte";
  import { focusFirst } from "$lib/nav";
  import Glyph from "./Glyph.svelte";
  import Pic from "./Pic.svelte";
  import Tile from "./Tile.svelte";

  let input = $state<HTMLInputElement>();
  let results = $state<HTMLElement>();
  $effect(() => {
    if (app.searchOpen) tick().then(() => input?.focus());
  });
  const r = $derived(app.everything);
  const total = $derived(r.games.length + r.ports.length + r.emulators.length + r.browser.length);
  const installedIds = $derived(new Set((app.catalog?.installed ?? []).map((i) => i.id)));

  function openItem(tab: string, item: string) {
    app.searchOpen = false;
    app.go({ name: "get", tab, item });
  }
</script>

{#if app.searchOpen}
  <div class="search" data-layer>
    <label class="field">
      <Glyph name="search" size={36} />
      <input
        bind:this={input}
        bind:value={app.query}
        data-nav
        placeholder="Search games, ports, emulators"
        spellcheck="false"
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === "ArrowDown") {
            e.preventDefault();
            focusFirst(results);
          }
        }}
      />
      {#if app.query}<span class="n label">{total}</span>{/if}
    </label>

    <div class="results" bind:this={results}>
      {#if r.games.length}
        <section>
          <div class="row-head"><Glyph name="library" size={24} /><span class="label">Your games</span></div>
          <div class="tiles">
            {#each r.games as g, i (g.id)}
              <Tile game={g} width={140} title="always" index={i} />
            {/each}
          </div>
        </section>
      {/if}
      {#each [["ports", "cart", "Ports", r.ports.map((p) => ({ id: p.id, t: p.game, s: p.name, kind: "port" as const }))], ["emulators", "pad", "Emulators", r.emulators.map((e) => ({ id: e.id, t: e.name, s: e.platforms.map((p) => app.platforms.get(p)?.short ?? p).join(" "), kind: "emulator" as const }))], ["browser", "globe", "In a browser window", r.browser.map((b) => ({ id: b.id, t: b.title, s: b.project, kind: "browser" as const }))]] as const as [tab, icon, label, items] (tab)}
        {#if items.length}
          <section>
            <div class="row-head"><Glyph name={icon} size={24} /><span class="label">{label}</span></div>
            <div class="cards">
              {#each items as it, i (it.id)}
                <button class="card" data-nav style:--i={i} onclick={() => openItem(tab, it.id)}>
                  <span class="cp" class:light={it.kind === "emulator"}><Pic kind={it.kind} id={it.id} icon={icon} fit={it.kind === "emulator" ? "contain" : "cover"} /></span>
                  <span class="t">{it.t}</span>
                  <span class="s label">{it.s}</span>
                  {#if installedIds.has(it.id)}<span class="have"><Glyph name="check" size={24} /></span>{/if}
                </button>
              {/each}
            </div>
          </section>
        {/if}
      {/each}
      {#if app.query && total === 0}
        <p class="none label">Nothing matches “{app.query}”.</p>
      {/if}
    </div>
  </div>
{/if}

<style>
  .search {
    position: fixed;
    inset: var(--bar) 0 var(--hints) 0;
    z-index: 35;
    padding: 36px 64px;
    background: var(--bg);
    overflow-y: auto;
  }
  :global(:root[data-theme="pixel"]) .search {
    animation: drop 0.2s steps(4) both;
  }
  :global(:root[data-theme="paper"]) .search {
    animation: fade 0.25s both;
  }
  @keyframes drop {
    from {
      clip-path: inset(0 0 100% 0);
    }
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  .field {
    display: flex;
    align-items: center;
    gap: 20px;
    padding-bottom: 14px;
    margin-bottom: 30px;
  }
  :global(:root[data-theme="pixel"]) .field {
    border-bottom: 4px solid var(--line);
  }
  :global(:root[data-theme="paper"]) .field {
    border-bottom: 2px solid var(--line);
  }
  input {
    flex: 1;
    border: 0;
    background: none;
    font-family: var(--title);
    font-size: 44px;
    font-weight: 700;
    caret-color: var(--fg);
  }
  input::placeholder {
    color: var(--faint);
  }
  input:focus-visible {
    background: none !important;
    color: var(--fg) !important;
  }
  input:focus-visible::after {
    display: none;
  }
  .results {
    display: flex;
    flex-direction: column;
    gap: 30px;
  }
  .tiles {
    display: flex;
    flex-wrap: wrap;
    gap: 24px 18px;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 14px;
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 0 0 12px;
    text-align: left;
    overflow: hidden;
    animation: rise 0.35s var(--ease) both;
    animation-delay: calc(var(--i) * 20ms);
  }
  :global(:root[data-theme="pixel"]) .card {
    border: var(--border) solid var(--line);
  }
  :global(:root[data-theme="paper"]) .card {
    background: var(--panel);
    box-shadow: 0 0 0 1px var(--line);
  }
  :global(:root[data-theme="paper"]) .card:focus-visible {
    box-shadow:
      0 0 0 2px var(--line),
      5px 5px 0 var(--line);
  }
  :global(:root[data-theme="paper"]) .card:focus-visible::after {
    display: none;
  }
  .cp {
    position: relative;
    display: block;
    width: 100%;
    height: 100px;
    margin-bottom: 8px;
    background: var(--panel);
  }
  .cp.light {
    background: #fff;
  }
  .t {
    padding: 0 12px;
    font-family: var(--title);
    font-weight: 800;
    font-size: 19px;
    line-height: 1.05;
  }
  .s {
    padding: 0 12px;
    color: var(--faint);
  }
  .card:focus-visible .s {
    color: inherit;
  }
  .have {
    position: absolute;
    top: 8px;
    right: 8px;
    background: var(--bg);
    color: var(--fg);
    padding: 2px;
  }
  .none {
    color: var(--faint);
  }
</style>
