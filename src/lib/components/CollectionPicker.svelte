<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api, type Game } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import type { PadAction } from "$lib/nav";
  import Glyph from "./Glyph.svelte";
  import LibGlyph from "./LibGlyph.svelte";
  import { collections } from "./collections.svelte";

  /** Pick the collections a game is in. Sits under (or above) its button; closes on Escape, B or a click outside. */
  let { game, onclose }: { game: Game; onclose: () => void } = $props();

  let panel = $state<HTMLDivElement | null>(null);
  let input = $state<HTMLInputElement | null>(null);
  let up = $state(false);
  let name = $state("");
  let busy = $state(false);

  onMount(() => {
    const r = panel!.getBoundingClientRect();
    up = r.bottom > window.innerHeight - 16 && r.top - r.height - 90 > 70;
    tick().then(() => (panel?.querySelector("[data-nav]") as HTMLElement | null)?.focus());

    const close = () => {
      onclose();
      (document.querySelector(".pickwrap > .btn") as HTMLElement | null)?.focus();
    };
    const onkey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopImmediatePropagation();
      close();
    };
    const onpad = (e: Event) => {
      if ((e as CustomEvent<PadAction>).detail !== "b") return;
      e.stopImmediatePropagation();
      close();
    };
    const ondown = (e: PointerEvent) => {
      if (!(e.target as HTMLElement).closest(".pickwrap")) onclose();
    };
    window.addEventListener("keydown", onkey, true);
    window.addEventListener("pad", onpad, true);
    window.addEventListener("pointerdown", ondown, true);
    return () => {
      window.removeEventListener("keydown", onkey, true);
      window.removeEventListener("pad", onpad, true);
      window.removeEventListener("pointerdown", ondown, true);
    };
  });

  async function toggle(id: number, inIt: boolean) {
    try {
      if (inIt) await api.removeFromCollection(id, game.id);
      else await api.addToCollection(id, game.id);
      await collections.load();
    } catch (e) {
      app.toast(String(e), "error");
    }
  }
  async function create() {
    const n = name.trim();
    if (!n || busy) return;
    busy = true;
    try {
      const c = await api.createCollection(n);
      await api.addToCollection(c.id, game.id);
      name = "";
      await collections.load();
    } catch (e) {
      app.toast(String(e), "error");
      input?.focus();
    }
    busy = false;
  }
</script>

<div class="pick" class:up bind:this={panel} role="dialog" aria-label="Collections for {game.title}">
  <div class="ph label">Collections</div>
  {#if collections.list.length}
    <div class="list">
      {#each collections.list as c (c.id)}
        {@const inIt = c.gameIds.includes(game.id)}
        <button class="opt" class:in={inIt} data-nav onclick={() => toggle(c.id, inIt)} aria-pressed={inIt}>
          <span class="box">{#if inIt}<Glyph name="check" size={12} />{/if}</span>
          <span class="nm">{c.name}</span>
          <em>{c.gameIds.length}</em>
        </button>
      {/each}
    </div>
  {:else}
    <p class="none">Put games together, like "Play next" or "Co-op".</p>
  {/if}
  <div class="add">
    <LibGlyph name="bookmark" size={24} />
    <input
      bind:this={input}
      bind:value={name}
      data-nav
      maxlength="80"
      spellcheck="false"
      placeholder="New collection"
      aria-label="New collection name"
      onkeydown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          create();
        }
      }}
    />
    <button class="go" data-nav onclick={create} disabled={!name.trim() || busy} aria-label="Create and add"><Glyph name="plus" size={24} /></button>
  </div>
</div>

<style>
  .pick {
    position: absolute;
    top: calc(100% + 12px);
    left: 0;
    z-index: 30;
    width: 320px;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: var(--bg);
    animation: drop 0.22s var(--ease) both;
    transform-origin: top left;
  }
  .pick.up {
    top: auto;
    bottom: calc(100% + 12px);
    transform-origin: bottom left;
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: scaleY(0.6);
    }
  }
  :global(:root[data-theme="pixel"]) .pick {
    border: var(--border) solid var(--line);
    box-shadow: 6px 6px 0 var(--fg);
    animation: pxdrop 0.18s steps(3) both;
  }
  @keyframes pxdrop {
    from {
      clip-path: inset(0 0 100% 0);
    }
  }
  :global(:root[data-theme="paper"]) .pick {
    background: var(--panel);
    border: 1.5px solid var(--line);
    box-shadow: 6px 6px 0 var(--line);
  }
  .ph {
    padding: 0 6px;
    color: var(--faint);
  }
  .list {
    display: flex;
    flex-direction: column;
    max-height: 264px;
    overflow-y: auto;
  }
  .opt {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    height: 40px;
    padding: 0 8px;
    text-align: left;
    font-family: var(--title);
    font-size: 16px;
    font-weight: 600;
  }
  .nm {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .opt em {
    font-style: normal;
    font-family: var(--label);
    font-size: 12px;
    color: var(--faint);
  }
  .box {
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
  }
  :global(:root[data-theme="pixel"]) .box {
    border: 2px solid currentColor;
  }
  :global(:root[data-theme="pixel"]) .opt.in .box {
    background: currentColor;
  }
  :global(:root[data-theme="pixel"]) .opt.in .box :global(svg) {
    color: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .opt:focus-visible.in .box :global(svg) {
    color: var(--fg);
  }
  :global(:root[data-theme="pixel"]) .opt:hover:not(:focus-visible) {
    box-shadow: inset 0 0 0 2px var(--fg);
  }
  :global(:root[data-theme="pixel"]) .opt:focus-visible em {
    color: inherit;
  }
  :global(:root[data-theme="paper"]) .box {
    border: 1.5px solid var(--line);
    border-radius: 2px;
  }
  :global(:root[data-theme="paper"]) .opt.in .box {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--panel);
  }
  :global(:root[data-theme="paper"]) .opt:hover,
  :global(:root[data-theme="paper"]) .opt:focus-visible {
    background: color-mix(in srgb, var(--fg) 7%, transparent);
  }
  :global(:root[data-theme="paper"]) .opt:focus-visible::after {
    left: 42px;
    right: 8px;
    bottom: 4px;
    height: 2px;
  }
  .none {
    margin: 0;
    padding: 0 6px;
    font-family: var(--label);
    font-size: 14px;
    line-height: 1.45;
    color: var(--faint);
  }
  .add {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) 40px;
    align-items: center;
    gap: 12px;
    padding: 10px 0 0 8px;
  }
  :global(:root[data-theme="pixel"]) .add {
    border-top: 2px solid var(--line);
  }
  :global(:root[data-theme="paper"]) .add {
    border-top: 1px solid color-mix(in srgb, var(--line) 25%, transparent);
  }
  .add input {
    min-width: 0;
    height: 36px;
    padding: 0 10px;
    background: var(--bg);
    font-family: var(--title);
    font-size: 15px;
  }
  :global(:root[data-theme="pixel"]) .add input {
    border: 2px solid var(--fg);
  }
  :global(:root[data-theme="paper"]) .add input {
    border: 1.5px solid var(--line);
    border-radius: var(--radius);
  }
  .go {
    width: 40px;
    height: 36px;
    display: grid;
    place-items: center;
  }
  .go:disabled {
    opacity: 0.35;
    cursor: default;
  }
  :global(:root[data-theme="pixel"]) .go:not(:disabled):hover {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="paper"]) .go:not(:disabled):hover {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .go:focus-visible::after {
    bottom: -2px;
  }
</style>
