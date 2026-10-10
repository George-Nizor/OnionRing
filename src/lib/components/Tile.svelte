<script lang="ts">
  import type { Game } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import Art from "./Art.svelte";
  import Glyph from "./Glyph.svelte";

  let {
    game,
    wide = false,
    width = 168,
    title = "focus",
    onfocus,
    index = 0,
  }: { game: Game; wide?: boolean; width?: number; title?: "always" | "focus" | "never"; onfocus?: () => void; index?: number } = $props();

  const running = $derived(app.running.includes(game.id));
</script>

<button
  class="tile"
  class:wide
  data-nav
  data-game-id={game.id}
  style:--w="{width}px"
  style:--i={Math.min(index, 14)}
  onclick={() => app.openGame(game)}
  onfocus={() => onfocus?.()}
  onmouseenter={(e) => (e.currentTarget as HTMLElement).focus({ preventScroll: true })}
  aria-label={game.title}
>
  <div class="frame">
    <Art {game} kind={wide ? "wide" : "cover"} />
    <span class="ribbon" aria-hidden="true"></span>
    <div class="flags">
      {#if running}<span class="flag live"><Glyph name="play" size={12} /></span>{/if}
      {#if game.packed}<span class="flag"><Glyph name="zip" size={12} /></span>{/if}
      {#if game.missing}<span class="flag"><Glyph name="alert" size={12} /></span>{/if}
      {#if game.favorite}<span class="flag"><Glyph name="star" size={12} /></span>{/if}
    </div>
  </div>
  {#if title !== "never"}
    <div class="name" class:always={title === "always"}><span>{game.title}</span></div>
  {/if}
</button>

<style>
  .tile {
    flex: none;
    width: var(--w);
    display: flex;
    flex-direction: column;
    gap: 10px;
    text-align: left;
    animation: rise 0.45s var(--ease) both;
    animation-delay: calc(var(--i) * 30ms);
  }
  .frame {
    position: relative;
    aspect-ratio: 3 / 4;
  }
  .wide .frame {
    aspect-ratio: 16 / 9;
  }
  .name {
    font-family: var(--label);
    font-size: 13px;
    font-weight: 700;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    opacity: 0;
    min-height: 1.4em;
  }
  .name.always,
  .tile:focus-visible .name {
    opacity: 1;
  }
  .flags {
    position: absolute;
    top: 6px;
    right: 6px;
    display: flex;
    gap: 4px;
  }
  .flag {
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    background: var(--bg);
    color: var(--fg);
  }

  /* ---- pixel ---- */
  :global(:root[data-theme="pixel"]) .tile:focus-visible {
    background: none;
    color: var(--fg);
  }
  :global(:root[data-theme="pixel"]) .frame {
    outline: 0 solid var(--fg);
  }
  :global(:root[data-theme="pixel"]) .tile:focus-visible .frame {
    outline: 4px solid var(--fg);
    outline-offset: 4px;
    animation: pxgrow 0.16s steps(2) both;
  }
  :global(:root[data-theme="pixel"]) .tile:focus-visible .frame::after {
    content: "";
    position: absolute;
    inset: -14px;
    background:
      linear-gradient(90deg, var(--fg) 50%, transparent 0) 0 0 / 8px 2px repeat-x,
      linear-gradient(90deg, var(--fg) 50%, transparent 0) 0 100% / 8px 2px repeat-x,
      linear-gradient(0deg, var(--fg) 50%, transparent 0) 0 0 / 2px 8px repeat-y,
      linear-gradient(0deg, var(--fg) 50%, transparent 0) 100% 0 / 2px 8px repeat-y;
    animation: ants 0.5s steps(2) infinite;
    pointer-events: none;
  }
  :global(:root[data-theme="pixel"]) .tile:focus-visible .name span {
    background: var(--fg);
    color: var(--bg);
    padding: 2px 6px;
  }
  :global(:root[data-theme="pixel"]) .tile:focus-visible .name span::after {
    content: "█";
    margin-left: 2px;
    animation: blink 0.8s steps(1) infinite;
  }
  :global(:root[data-theme="pixel"]) .ribbon {
    display: none;
  }
  @keyframes pxgrow {
    from {
      transform: scale(0.94);
    }
  }

  /* ---- paper ---- */
  :global(:root[data-theme="paper"]) .frame {
    box-shadow: 0 0 0 1px var(--line);
    transition: transform 0.35s var(--ease);
  }
  :global(:root[data-theme="paper"]) .tile:focus-visible::after {
    display: none;
  }
  :global(:root[data-theme="paper"]) .tile:focus-visible .frame {
    transform: translateY(-6px);
    box-shadow:
      0 0 0 2px var(--line),
      6px 6px 0 var(--line);
  }
  :global(:root[data-theme="paper"]) .tile:focus-visible :global(.art) {
    filter: var(--img-filter-focus);
  }
  :global(:root[data-theme="paper"]) .ribbon {
    position: absolute;
    top: -2px;
    right: 14px;
    width: 18px;
    height: 0;
    background: var(--accent);
    clip-path: polygon(0 0, 100% 0, 100% 100%, 50% 78%, 0 100%);
    transition: height 0.35s var(--ease);
  }
  :global(:root[data-theme="paper"]) .tile:focus-visible .ribbon {
    height: 34px;
  }
  :global(:root[data-theme="paper"]) .name {
    font-family: var(--title);
    font-style: italic;
    font-weight: 600;
    font-size: 15px;
    transition: opacity 0.3s;
  }
  :global(:root[data-theme="paper"]) .flag {
    background: var(--panel);
    box-shadow: 0 0 0 1px var(--line);
  }
</style>
