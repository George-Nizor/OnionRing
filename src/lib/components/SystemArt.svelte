<script lang="ts" module>
  // Hand-made console art, one JSON file per platform id in src/lib/art/systems/
  // (format: src/lib/art/systems/README.md). Pixel theme draws the 48x32 bitmap; paper theme
  // draws the ink illustration. Missing art falls back to the controller icon.
  type Art = { pixel: string[]; ink: string };
  const files = import.meta.glob<Art>("../art/systems/*.json", { eager: true, import: "default" });
  const ART = new Map<string, Art>(Object.entries(files).map(([p, a]) => [p.split("/").pop()!.replace(".json", ""), a]));

  const RECTS = new Map<string, { x: number; y: number; w: number }[]>();
  function rects(id: string, rows: string[]) {
    if (RECTS.has(id)) return RECTS.get(id)!;
    const out: { x: number; y: number; w: number }[] = [];
    rows.forEach((row, y) => {
      let x = 0;
      while (x < row.length) {
        if (row[x] === "#") {
          let w = 1;
          while (row[x + w] === "#") w++;
          out.push({ x, y, w });
          x += w;
        } else x++;
      }
    });
    RECTS.set(id, out);
    return out;
  }
  export const hasSystemArt = (id: string) => ART.has(id);
</script>

<script lang="ts">
  import { app } from "$lib/store.svelte";
  import Glyph from "./Glyph.svelte";

  /** `id` is a platform id (ps2, gc, 3ds, ...). Fills its parent (position it yourself). */
  let { id, label }: { id: string; label?: string } = $props();
  const art = $derived(ART.get(id));
</script>

{#if art && app.theme === "pixel"}
  <svg class="sa" viewBox="0 0 48 32" shape-rendering="crispEdges" preserveAspectRatio="xMidYMid meet" role="img" aria-label={label ?? id}>
    {#each rects(id, art.pixel) as r (`${r.x}-${r.y}`)}
      <rect x={r.x} y={r.y} width={r.w} height="1" fill="currentColor" />
    {/each}
  </svg>
{:else if art}
  <svg class="sa" viewBox="0 0 240 160" preserveAspectRatio="xMidYMid meet" role="img" aria-label={label ?? id}>
    {@html art.ink}
  </svg>
{:else}
  <span class="sa none"><Glyph name="pad" size={48} /></span>
{/if}

<style>
  .sa {
    display: block;
    width: 100%;
    height: 100%;
  }
  .none {
    display: grid;
    place-items: center;
    /* A grid item never shrinks below its content by default; the glyph must fit the slot. */
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    color: var(--faint);
  }
  /* The fallback controller shrinks to fit small slots (the library's 32x24 system icons). */
  .none :global(svg) {
    width: 100%;
    height: 100%;
    max-width: 144px;
    max-height: 144px;
    min-height: 0;
  }
</style>
