<script lang="ts" module>
  import { api, src, type PicKind } from "$lib/api";
  // One request per picture for the whole session.
  const cache = new Map<string, Promise<string | null>>();
  function load(kind: PicKind, id: string) {
    const k = `${kind}:${id}`;
    if (!cache.has(k)) cache.set(k, api.picture(kind, id).then((p) => src(p)).catch(() => null));
    return cache.get(k)!;
  }
</script>

<script lang="ts">
  import Glyph from "./Glyph.svelte";
  import SystemArt, { hasSystemArt } from "./SystemArt.svelte";

  /**
   * A fetched picture for a system/emulator/port/browser game; loads when scrolled into view.
   * Without a picture (or with `skip`), it shows the console art of `system`, else a name plate
   * (`plate`), else the icon. Console art is drawn at a whole multiple of its 48x32 grid: set
   * `--sa-w` on a parent (96px, 144px, 192px, 240px) to size it.
   */
  let {
    kind,
    id,
    icon = "library",
    fit = "cover",
    system = null,
    plate = null,
    skip = false,
  }: { kind: PicKind; id: string; icon?: string; fit?: "cover" | "contain"; system?: string | null; plate?: string | null; skip?: boolean } = $props();

  let el = $state<HTMLElement>();
  let url = $state<string | null | undefined>(undefined);
  $effect(() => {
    const node = el;
    const k = kind;
    const i = id;
    if (!node) return;
    if (skip) {
      url = null;
      return;
    }
    url = undefined;
    const io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) {
        io.disconnect();
        load(k, i).then((u) => (url = u));
      }
    });
    io.observe(node);
    return () => io.disconnect();
  });
  const art = $derived(system && hasSystemArt(system) ? system : null);
</script>

<div class="pic" bind:this={el}>
  {#if url}
    <img class="art" class:contain={fit === "contain"} src={url} alt="" decoding="async" />
  {:else if url === null && art}
    <span class="fb"><span class="sa"><SystemArt id={art} /></span></span>
  {:else if url === null}
    <span class="fb plate">
      <Glyph name={icon} size={36} />
      {#if plate}<span class="pt">{plate}</span>{/if}
    </span>
  {:else}
    <span class="ph"><Glyph name={icon} size={36} /></span>
  {/if}
</div>

<style>
  .pic {
    position: absolute;
    inset: 0;
    overflow: hidden;
    display: grid;
    place-items: center;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation: show 0.4s both;
  }
  img.contain {
    object-fit: contain;
    padding: 10%;
  }
  .ph {
    color: var(--faint);
    animation: blink 1.2s steps(2) infinite;
  }
  /* The fallback owns its colours, so it reads the same inside a white logo frame or a focused tile. */
  .fb {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: var(--bg);
    color: var(--fg);
    animation: show 0.4s both;
  }
  .sa {
    display: block;
    width: var(--sa-w, 144px);
    aspect-ratio: 3 / 2;
    max-width: 92%;
    max-height: 92%;
  }
  .plate {
    align-content: center;
    gap: 10px;
    color: var(--faint);
  }
  .pt {
    font-family: var(--label);
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  :global(:root[data-theme="pixel"]) .fb {
    animation-timing-function: steps(3);
  }
  :global(:root[data-theme="paper"]) .fb {
    background: color-mix(in srgb, var(--fg) 5%, var(--bg));
    color: var(--fg);
  }
  :global(:root[data-theme="paper"]) .plate {
    color: var(--faint);
  }
  @keyframes show {
    from {
      opacity: 0;
    }
  }
</style>
