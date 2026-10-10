<script lang="ts">
  import { tick } from "svelte";
  import { app } from "$lib/store.svelte";
  import { focusFirst } from "$lib/nav";
  import Glyph from "./Glyph.svelte";
  import Tile from "./Tile.svelte";

  let input = $state<HTMLInputElement>();
  let results = $state<HTMLElement>();
  $effect(() => {
    if (app.searchOpen) tick().then(() => input?.focus());
  });
</script>

{#if app.searchOpen}
  <div class="search" data-layer>
    <label class="field">
      <Glyph name="search" size={36} />
      <input
        bind:this={input}
        bind:value={app.query}
        data-nav
        placeholder="Type a game"
        spellcheck="false"
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === "ArrowDown") {
            e.preventDefault();
            focusFirst(results);
          }
        }}
      />
      {#if app.query}<span class="n label">{app.searchResults.length}</span>{/if}
    </label>
    <div class="results" bind:this={results}>
      {#each app.searchResults as g, i (g.id)}
        <Tile game={g} width={150} title="always" index={i} />
      {/each}
    </div>
  </div>
{/if}

<style>
  .search {
    position: fixed;
    inset: var(--bar) 0 var(--hints) 0;
    z-index: 35;
    padding: 40px 64px;
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
    margin-bottom: 34px;
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
    flex-wrap: wrap;
    gap: 26px 20px;
  }
</style>
