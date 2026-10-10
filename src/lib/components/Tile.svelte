<script lang="ts">
  import type { Game } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import Art from "./Art.svelte";
  import Glyph from "./Glyph.svelte";

  let {
    game,
    wide = false,
    width,
    title = "always",
    meta,
    onfocus,
    index = 0,
  }: {
    game: Game;
    wide?: boolean;
    /** px; leave out (or 0) to use the shared --tile size. */
    width?: number;
    /** Names always show; "never" is for places that print the name next to the tile. */
    title?: "always" | "focus" | "never";
    /** A short line under the name (platform, "2 days ago"). */
    meta?: string;
    onfocus?: () => void;
    index?: number;
  } = $props();

  const running = $derived(app.running.includes(game.id));
</script>

<button
  class="tile"
  class:wide
  data-nav
  data-game-id={game.id}
  style:--w={width ? `${width}px` : undefined}
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
      {#if running}<span class="flag live"><Glyph name="play" size={12} label="Playing" /></span>{/if}
      {#if game.packed}<span class="flag"><Glyph name="zip" size={12} label="Zipped" /></span>{/if}
      {#if game.missing}<span class="flag"><Glyph name="alert" size={12} label="Missing" /></span>{/if}
      {#if game.favorite}<span class="flag"><Glyph name="star" size={12} label="Favourite" /></span>{/if}
    </div>
  </div>
  {#if title !== "never"}
    <div class="cap">
      <div class="name"><span>{game.title}</span></div>
      {#if meta}<div class="meta label">{meta}</div>{/if}
    </div>
  {/if}
</button>

<style>
  .tile {
    flex: none;
    width: var(--w, var(--tile, 168px));
    display: flex;
    flex-direction: column;
    gap: var(--s3);
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
  .cap {
    display: flex;
    flex-direction: column;
    gap: var(--s1);
    min-width: 0;
  }
  /* Two lines are always reserved so a row of tiles lines up whatever the names. */
  .name {
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
    overflow-wrap: anywhere;
  }
  .meta {
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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
  :global(:root[data-theme="pixel"]) .name {
    font-weight: 500;
  }
  :global(:root[data-theme="pixel"]) .frame {
    outline: 0 solid var(--fg);
  }
  :global(:root[data-theme="pixel"]) .tile:hover .frame {
    outline: 2px solid var(--faint);
    outline-offset: 4px;
  }
  :global(:root[data-theme="pixel"]) .tile:focus-visible .frame {
    outline: 3px solid var(--fg);
    outline-offset: 4px;
    animation: pxgrow 0.16s steps(2) both;
  }
  :global(:root[data-theme="pixel"]) .tile:focus-visible .frame::after {
    content: "";
    position: absolute;
    inset: -12px;
    background:
      linear-gradient(90deg, var(--fg) 50%, transparent 0) 0 0 / 8px 2px repeat-x,
      linear-gradient(90deg, var(--fg) 50%, transparent 0) 0 100% / 8px 2px repeat-x,
      linear-gradient(0deg, var(--fg) 50%, transparent 0) 0 0 / 2px 8px repeat-y,
      linear-gradient(0deg, var(--fg) 50%, transparent 0) 100% 0 / 2px 8px repeat-y;
    animation: ants 0.5s steps(2) infinite;
    pointer-events: none;
  }
  :global(:root[data-theme="pixel"]) .tile:is(:focus-visible, :hover) .name span {
    background: var(--fg);
    color: var(--bg);
    box-shadow:
      4px 0 0 var(--fg),
      -4px 0 0 var(--fg);
    -webkit-box-decoration-break: clone;
    box-decoration-break: clone;
  }
  :global(:root[data-theme="pixel"]) .tile:focus-visible .meta {
    color: var(--fg);
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
    transition:
      transform 0.35s var(--ease),
      box-shadow 0.35s var(--ease);
  }
  :global(:root[data-theme="paper"]) .tile:focus-visible::after {
    display: none;
  }
  :global(:root[data-theme="paper"]) .tile:is(:focus-visible, :hover) .frame {
    transform: translateY(-6px);
    box-shadow:
      0 0 0 1.5px var(--line),
      6px 6px 0 var(--line);
  }
  :global(:root[data-theme="paper"]) .tile:is(:focus-visible, :hover) :global(.art) {
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
  :global(:root[data-theme="paper"]) .name span {
    background: linear-gradient(var(--fg), var(--fg)) 0 100% / 0 1.5px no-repeat;
    transition: background-size 0.35s var(--ease);
    -webkit-box-decoration-break: clone;
    box-decoration-break: clone;
  }
  :global(:root[data-theme="paper"]) .tile:is(:focus-visible, :hover) .name span {
    background-size: 100% 1.5px;
  }
  :global(:root[data-theme="paper"]) .tile:focus-visible .name {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .tile:focus-visible .name span {
    background-image: linear-gradient(var(--accent), var(--accent));
  }
  :global(:root[data-theme="paper"]) .flag {
    background: var(--panel);
    box-shadow: 0 0 0 1px var(--line);
  }
</style>
