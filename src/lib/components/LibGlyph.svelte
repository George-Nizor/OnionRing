<script lang="ts" module>
  // Extra icons for the library and game page (collections), drawn like Glyph.svelte:
  // a 12x12 bitmap for pixel and a 24x24 line path for paper.
  const PX: Record<string, string[]> = {
    bookmark: [".##########.", ".#........#.", ".#........#.", ".#........#.", ".#........#.", ".#........#.", ".#........#.", ".#...##...#.", ".#..#..#..#.", ".#.#....#.#.", ".##......##.", ".#........#."],
    "bookmark-on": [".##########.", ".##########.", ".##########.", ".##########.", ".##########.", ".##########.", ".##########.", ".####..####.", ".###....###.", ".##......##.", ".#........#.", "............"],
    pencil: ["........###.", ".......#####", "......#.###.", ".....#.###..", "....#.###...", "...#.###....", "..#.###.....", ".#.###......", "#.###.......", "###.........", "##..........", "............"],
    trash: ["....####....", "############", "............", ".##########.", ".#..#..#..#.", ".#..#..#..#.", ".#..#..#..#.", ".#..#..#..#.", ".#..#..#..#.", ".#..#..#..#.", ".##########.", "............"],
  };
  const LINE: Record<string, string> = {
    bookmark: "M6.5 3.5h11v17L12 15.8l-5.5 4.7z",
    "bookmark-on": "M6.5 3.5h11v17L12 15.8l-5.5 4.7z",
    pencil: "M15.2 4.8l4 4L8.5 19.5h-4v-4zM12.7 7.3l4 4",
    trash: "M4 6.5h16M9.5 6.5V4h5v2.5M6.2 6.5l1 13.5h9.6l1-13.5M10 10.5v6M14 10.5v6",
  };
  const RECTS = new Map<string, { x: number; y: number; w: number }[]>();
  function rects(name: string) {
    if (RECTS.has(name)) return RECTS.get(name)!;
    const out: { x: number; y: number; w: number }[] = [];
    (PX[name] ?? []).forEach((row, y) => {
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
    RECTS.set(name, out);
    return out;
  }
</script>

<script lang="ts">
  import { app } from "$lib/store.svelte";

  let { name, size = 24, label }: { name: "bookmark" | "bookmark-on" | "pencil" | "trash"; size?: number; label?: string } = $props();
  const px = $derived(Math.max(12, Math.round(size / 12) * 12));
</script>

{#if app.theme === "pixel"}
  <svg class="g" width={px} height={px} viewBox="0 0 12 12" shape-rendering="crispEdges" role={label ? "img" : undefined} aria-label={label} aria-hidden={label ? undefined : "true"}>
    {#each rects(name) as r (`${r.x}-${r.y}`)}
      <rect x={r.x} y={r.y} width={r.w} height="1" fill="currentColor" />
    {/each}
  </svg>
{:else}
  <svg class="g" width={size} height={size} viewBox="0 0 24 24" fill={name === "bookmark-on" ? "currentColor" : "none"} stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" role={label ? "img" : undefined} aria-label={label} aria-hidden={label ? undefined : "true"}>
    <path d={LINE[name]} />
  </svg>
{/if}

<style>
  .g {
    display: block;
    flex: none;
  }
</style>
