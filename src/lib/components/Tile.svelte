<script lang="ts">
  import type { Game } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";

  let { game, selected = false, index = 0, grid = false }: { game: Game; selected?: boolean; index?: number; grid?: boolean } =
    $props();
  const running = $derived(app.running.includes(game.id));
</script>

<button
  class="tile"
  class:selected
  class:grid
  class:dim={game.missing || game.hidden}
  style:--i={Math.min(index, 20)}
  style:--c={app.accent(game)}
  data-game={game.id}
  onclick={() => app.select(game.id)}
  ondblclick={() => app.play(game)}
  aria-label={game.title}
>
  <div class="frame">
    <Cover {game} />
    {#if running || game.packed || game.missing || game.favorite}
      <div class="flags">
        {#if running}<span class="flag live"><span class="d"></span></span>{/if}
        {#if game.packed}<span class="flag" title="Zipped"><Icon name="unpack" size={13} /></span>{/if}
        {#if game.missing}<span class="flag warn" title="Missing"><Icon name="alert" size={13} /></span>{/if}
        {#if game.favorite}<span class="flag star"><Icon name="star" size={13} /></span>{/if}
      </div>
    {/if}
  </div>
  {#if grid}
    <div class="name">{game.title}</div>
  {/if}
</button>

<style>
  .tile {
    position: relative;
    flex: none;
    width: var(--tile-w, 156px);
    display: flex;
    flex-direction: column;
    gap: 10px;
    text-align: left;
    outline: none;
    animation: rise 0.6s var(--ease) both;
    animation-delay: calc(var(--i) * 28ms);
  }
  .tile.grid {
    width: auto;
    content-visibility: auto;
    contain-intrinsic-size: 170px 270px;
  }
  .frame {
    position: relative;
    aspect-ratio: 3 / 4;
    border-radius: 12px;
    overflow: hidden;
    container-type: inline-size;
    background: #14111b;
    transform-origin: 50% 100%;
    box-shadow: 0 18px 34px -20px rgba(0, 0, 0, 0.95);
    transition:
      transform 0.4s var(--ease),
      box-shadow 0.4s var(--ease),
      filter 0.3s;
  }
  .tile:not(.selected) .frame {
    filter: brightness(0.82) saturate(0.95);
  }
  .tile:hover .frame {
    filter: none;
  }
  /* Selection: a white ring with a slow onion-ring glow behind it. */
  .frame::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.1);
    transition: box-shadow 0.3s;
    pointer-events: none;
  }
  .selected .frame {
    transform: scale(1.1);
    box-shadow:
      0 0 0 3px #fff,
      0 0 0 7px rgba(255, 255, 255, 0.12),
      0 30px 60px -18px rgba(0, 0, 0, 0.95),
      0 0 70px -10px color-mix(in srgb, var(--c) 75%, transparent);
  }
  .tile.grid.selected .frame {
    transform: scale(1.04);
  }
  .tile:focus-visible .frame {
    box-shadow:
      0 0 0 3px var(--coral),
      0 20px 40px -18px rgba(0, 0, 0, 0.9);
  }
  .flags {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    gap: 4px;
  }
  .flag {
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: rgba(6, 5, 9, 0.72);
    backdrop-filter: blur(8px);
    color: #fff;
  }
  .flag.star {
    color: var(--gold);
  }
  .flag.star :global(svg) {
    fill: currentColor;
  }
  .flag.warn {
    color: var(--warn);
  }
  .flag .d {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 10px var(--ok);
  }
  .name {
    font-size: 13.5px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.8);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    padding: 0 2px;
  }
  .selected .name {
    color: #fff;
  }
  .dim {
    opacity: 0.45;
  }
</style>
