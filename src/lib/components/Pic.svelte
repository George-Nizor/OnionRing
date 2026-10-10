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

  /** A fetched picture for a system/emulator/port/browser game; loads when scrolled into view. */
  let { kind, id, icon = "library", fit = "cover" }: { kind: PicKind; id: string; icon?: string; fit?: "cover" | "contain" } = $props();

  let el = $state<HTMLElement>();
  let url = $state<string | null | undefined>(undefined);
  $effect(() => {
    const node = el;
    const k = kind;
    const i = id;
    if (!node) return;
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
</script>

<div class="pic" bind:this={el}>
  {#if url}
    <img class="art" class:contain={fit === "contain"} src={url} alt="" decoding="async" />
  {:else}
    <span class="ph" class:loading={url === undefined}><Glyph name={icon} size={36} /></span>
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
  }
  .ph.loading {
    animation: blink 1.2s steps(2) infinite;
  }
  @keyframes show {
    from {
      opacity: 0;
    }
  }
</style>
