<script lang="ts">
  import { onMount } from "svelte";
  import { api, type StorageReport } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { bytes } from "$lib/format";
  import Glyph from "./Glyph.svelte";

  let { onchange }: { onchange?: (n: number) => void } = $props();
  let report = $state<StorageReport | null>(null);
  let working = $state(false);
  let details = $state(false);

  async function load() {
    report = await api.storage();
    onchange?.(report.reclaimable);
  }
  onMount(load);

  const extras = $derived((report?.groups ?? []).flatMap((g) => g.redundant.map((r) => r.path)));
  const max = $derived(Math.max(1, ...(report?.groups ?? []).map((g) => g.keepSize + g.reclaimable)));

  // Count up to the number, for a bit of life.
  let shown = $state(0);
  $effect(() => {
    const target = report?.reclaimable ?? 0;
    const start = performance.now();
    let raf = requestAnimationFrame(function step(t) {
      const k = Math.min(1, (t - start) / 900);
      shown = target * (1 - Math.pow(1 - k, 3));
      if (k < 1) raf = requestAnimationFrame(step);
    });
    return () => cancelAnimationFrame(raf);
  });

  async function reclaim() {
    const { ask } = await import("@tauri-apps/plugin-dialog");
    const ok = await ask(`Move ${bytes(report?.reclaimable ?? 0)} of extra copies into your Reclaim folder?\n\nNothing is deleted. Undo puts everything back.`, {
      title: "Tidy up",
      okLabel: "Move them",
    });
    if (!ok) return;
    working = true;
    try {
      const [n, b] = await api.reclaim(extras);
      app.toast(`Moved ${n} extra copies (${bytes(b)}) to the Reclaim folder`, "ok");
      await load();
    } catch (e) {
      app.toast(String(e), "error", 9000);
    }
    working = false;
  }
  async function restore() {
    working = true;
    try {
      await api.restoreReclaimed();
      app.toast("Everything is back where it was", "ok");
      await load();
    } catch (e) {
      app.toast(String(e), "error", 9000);
    }
    working = false;
  }
</script>

<div class="tidy">
  {#if report}
    <div class="head">
      <Glyph name="broom" size={96} />
      <div>
        <div class="big">{bytes(shown)}</div>
        <div class="label">in extra copies of games you already have</div>
      </div>
      <div class="acts">
        <button class="btn big primary" data-nav onclick={reclaim} disabled={working || !extras.length}><Glyph name="broom" size={24} />Tidy up</button>
        {#if report.held > 0}
          <button class="btn big" data-nav onclick={restore} disabled={working}><Glyph name="undo" size={24} />Undo</button>
        {/if}
        <button class="btn big icon" class:on={details} data-nav onclick={() => (details = !details)} aria-label="Show files"><Glyph name="folder" size={24} /></button>
      </div>
    </div>
    <p class="how label">Extras move to your Reclaim folder, nothing is deleted. Undo puts them back.</p>

    <div class="piles">
      {#each report.groups as g, i (g.key)}
        {@const p = app.platforms.get(g.platform)}
        <div class="pile" style:--i={i}>
          <div class="who">
            <span class="plat label">{p?.short}</span>
            <span class="t">{g.title}</span>
          </div>
          <div class="bars">
            <span class="keep" style:--w="{(g.keepSize / max) * 100}%" title="Kept"></span>
            {#each g.redundant as r, j (r.path)}
              <span class="extra" style:--w="{(r.size / max) * 100}%" style:--j={j} title={r.reason}></span>
            {/each}
          </div>
          <span class="save label">{bytes(g.reclaimable)}</span>
          {#if details}
            <div class="files">
              {#each g.redundant as r (r.path)}<div class="mono" title={r.path}>{r.path}</div>{/each}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .tidy {
    height: 100%;
    overflow-y: auto;
    padding: 30px 56px 60px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 28px;
  }
  .head :global(svg) {
    animation: sweep 2.2s ease-in-out infinite;
    transform-origin: 80% 10%;
  }
  :global(:root[data-theme="pixel"]) .head :global(svg) {
    animation: sweep 1.2s steps(2) infinite;
  }
  @keyframes sweep {
    50% {
      transform: rotate(-12deg);
    }
  }
  .big {
    font-family: var(--title);
    font-weight: 800;
    font-size: 72px;
    line-height: 1;
    font-variant-numeric: tabular-nums;
  }
  :global(:root[data-theme="paper"]) .big {
    color: var(--accent);
  }
  .acts {
    margin-left: auto;
    display: flex;
    gap: 12px;
  }
  .how {
    color: var(--faint);
    margin: 18px 0 30px;
  }
  .piles {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .pile {
    display: grid;
    grid-template-columns: 300px 1fr 90px;
    align-items: center;
    gap: 20px;
    animation: rise 0.4s var(--ease) both;
    animation-delay: calc(var(--i) * 40ms);
  }
  .who {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .t {
    font-family: var(--title);
    font-weight: 700;
    font-size: 18px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .plat {
    flex: none;
    width: 46px;
  }
  .bars {
    display: flex;
    gap: 4px;
    height: 26px;
  }
  .bars span {
    width: var(--w);
    min-width: 6px;
    transform-origin: left;
    animation: grow 0.6s var(--ease) both;
    animation-delay: calc(var(--i) * 40ms + var(--j, 0) * 90ms + 150ms);
  }
  :global(:root[data-theme="pixel"]) .keep {
    background: var(--fg);
  }
  :global(:root[data-theme="pixel"]) .extra {
    border: var(--border) dashed var(--fg);
  }
  :global(:root[data-theme="pixel"]) .bars span {
    animation-timing-function: steps(5);
  }
  :global(:root[data-theme="paper"]) .keep {
    background: var(--fg);
  }
  :global(:root[data-theme="paper"]) .extra {
    background: repeating-linear-gradient(135deg, var(--accent) 0 2px, transparent 2px 7px);
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  @keyframes grow {
    from {
      transform: scaleX(0);
    }
  }
  .save {
    text-align: right;
  }
  .files {
    grid-column: 1 / -1;
    color: var(--faint);
  }
  .mono {
    font-family: "Cascadia Mono", Consolas, monospace;
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  :global(:root[data-theme="pixel"]) .btn.on {
    background: var(--fg);
    color: var(--bg);
  }
</style>
