<script lang="ts">
  import { onMount } from "svelte";
  import { api, type StorageReport } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { bytes } from "$lib/format";
  import Glyph from "./Glyph.svelte";
  import SystemArt from "./SystemArt.svelte";

  let { onchange }: { onchange?: (n: number) => void } = $props();
  let report = $state<StorageReport | null>(null);
  let working = $state(false);
  let details = $state(false);

  async function load() {
    report = await api.storage();
    onchange?.(report.reclaimable);
  }
  onMount(load);

  const groups = $derived(report?.groups ?? []);
  const extras = $derived(groups.flatMap((g) => g.redundant.map((r) => r.path)));
  const max = $derived(Math.max(1, ...groups.map((g) => g.keepSize + g.reclaimable)));

  // Count up to the number. Pixel ticks over in steps, like an old counter; paper eases in.
  let shown = $state(0);
  $effect(() => {
    const target = report?.reclaimable ?? 0;
    const stepped = app.theme === "pixel";
    const start = performance.now();
    let raf = requestAnimationFrame(function step(t) {
      let k = Math.min(1, (t - start) / 900);
      if (stepped) k = Math.ceil(k * 8) / 8;
      shown = target * (stepped ? k : 1 - Math.pow(1 - k, 3));
      if (k < 1) raf = requestAnimationFrame(step);
    });
    return () => cancelAnimationFrame(raf);
  });

  const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

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
    <header class="head">
      <div class="broom"><Glyph name="broom" size={72} /></div>
      <div class="sum">
        {#if groups.length}
          <div class="amount">{bytes(shown)}</div>
          <div class="caption">in extra copies of games you already have</div>
        {:else}
          <div class="amount done">All tidy</div>
          <div class="caption">Every game is stored once.</div>
        {/if}
      </div>
      <div class="acts">
        {#if groups.length}
          <button class="btn big primary" data-nav onclick={reclaim} disabled={working || !extras.length}><Glyph name="broom" size={24} />Tidy up</button>
        {/if}
        {#if report.held > 0}
          <button class="btn big" data-nav onclick={restore} disabled={working}><Glyph name="undo" size={24} />Undo</button>
        {/if}
        {#if groups.length}
          <button class="btn big icon" class:on={details} data-nav onclick={() => (details = !details)} aria-label={details ? "Hide files" : "Show files"} title={details ? "Hide files" : "Show files"}><Glyph name="folder" size={24} /></button>
        {/if}
      </div>
    </header>

    <div class="facts">
      <div class="fact"><span class="k label">Games stored twice</span><span class="v">{groups.length}</span></div>
      <div class="fact"><span class="k label">Extra copies</span><span class="v">{extras.length}</span></div>
      <div class="fact"><span class="k label">Whole library</span><span class="v">{bytes(report.librarySize)}</span></div>
      <div class="fact"><span class="k label">Waiting in Reclaim</span><span class="v">{report.held}</span></div>
    </div>

    <p class="how">
      <Glyph name="info" size={24} />
      <span>Extras move to your Reclaim folder. Nothing is deleted, and Undo puts them back.</span>
    </p>

    {#if groups.length}
      <div class="table">
        <div class="thead label">
          <span>Game</span>
          <span class="legend"><i class="sw keep"></i>Copy you keep<i class="sw extra"></i>Extra copy</span>
          <span class="r">Frees up</span>
        </div>
        {#each groups as g, i (g.key)}
          {@const p = app.platforms.get(g.platform)}
          <div class="pile" style:--i={i}>
            <div class="who">
              <div class="sys"><SystemArt id={g.platform} /></div>
              <div class="names">
                <span class="t" title={g.title}>{g.title}</span>
                <span class="plat label">{p?.name ?? g.platform}</span>
              </div>
            </div>
            <div class="bars" aria-label="{plural(g.redundant.length, 'extra copy', 'extra copies')}">
              <span class="keep" style:--w="{(g.keepSize / max) * 100}%" title="Copy you keep, {bytes(g.keepSize)}"></span>
              {#each g.redundant as r, j (r.path)}
                <span class="extra" style:--w="{(r.size / max) * 100}%" style:--j={j + 1} title="Extra copy, {bytes(r.size)}"></span>
              {/each}
            </div>
            <div class="save">
              <b>{bytes(g.reclaimable)}</b>
              <span class="label">{plural(g.redundant.length, "extra copy", "extra copies")}</span>
            </div>
            {#if details}
              <div class="files">
                {#each g.redundant as r (r.path)}<div class="mono" title={r.path}>{r.path}</div>{/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .tidy {
    height: 100%;
    overflow-y: auto;
    padding: 32px 56px 64px;
  }

  /* ---- Header ---- */
  .head {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 32px;
  }
  .broom {
    width: 112px;
    height: 112px;
    display: grid;
    place-items: center;
  }
  .broom :global(svg) {
    animation: sweep 2.2s ease-in-out infinite;
    transform-origin: 80% 10%;
  }
  :global(:root[data-theme="pixel"]) .broom {
    border: var(--border) solid var(--line);
  }
  :global(:root[data-theme="pixel"]) .broom :global(svg) {
    animation: sweep 1.2s steps(2) infinite;
  }
  :global(:root[data-theme="paper"]) .broom {
    border-radius: 50%;
    background: var(--panel);
    box-shadow: inset 0 0 0 1.5px var(--line);
    color: var(--accent);
  }
  @keyframes sweep {
    50% {
      transform: rotate(-12deg);
    }
  }
  .sum {
    min-width: 0;
  }
  .amount {
    font-family: var(--title);
    font-weight: 800;
    font-size: 72px;
    line-height: 1;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  :global(:root[data-theme="paper"]) .amount {
    color: var(--accent);
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  :global(:root[data-theme="paper"]) .amount.done {
    color: var(--fg);
  }
  .caption {
    margin-top: 10px;
    font-size: 18px;
    color: var(--muted);
  }
  .acts {
    display: flex;
    gap: 12px;
  }

  /* ---- Facts strip ---- */
  .facts {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    margin-top: 32px;
    border-top: var(--border) solid var(--line);
    border-bottom: var(--border) solid var(--line);
  }
  :global(:root[data-theme="paper"]) .facts {
    border-width: 1px;
  }
  .fact {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 16px 24px;
    min-width: 0;
  }
  .fact:first-child {
    padding-left: 0;
  }
  .fact + .fact {
    border-left: 1px solid var(--faint);
  }
  :global(:root[data-theme="pixel"]) .fact + .fact {
    border-left: 3px dotted var(--faint);
  }
  .fact .k {
    color: var(--faint);
    white-space: nowrap;
  }
  .fact .v {
    font-family: var(--title);
    font-weight: 700;
    font-size: 28px;
    line-height: 1.1;
    font-variant-numeric: tabular-nums;
  }

  .how {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 24px 0 32px;
    color: var(--muted);
  }
  .how :global(svg) {
    flex: none;
    color: var(--faint);
  }

  /* ---- Table ---- */
  .thead,
  .pile {
    display: grid;
    grid-template-columns: minmax(260px, 30%) 1fr 132px;
    align-items: center;
    gap: 24px;
  }
  .thead {
    color: var(--faint);
    padding-bottom: 12px;
  }
  .r {
    text-align: right;
  }
  .legend {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .sw {
    width: 24px;
    height: 12px;
    flex: none;
  }
  .sw.extra {
    margin-left: 16px;
  }
  .pile {
    padding: 12px 0;
    border-top: 1px solid color-mix(in srgb, var(--line) 22%, transparent);
    animation: rise 0.4s var(--ease) both;
    animation-delay: calc(var(--i) * 40ms);
  }
  :global(:root[data-theme="pixel"]) .pile {
    border-top: 3px dotted color-mix(in srgb, var(--line) 40%, transparent);
    animation-timing-function: steps(3);
  }
  .who {
    display: flex;
    align-items: center;
    gap: 16px;
    min-width: 0;
  }
  .sys {
    flex: none;
    width: 72px;
    height: 48px;
    position: relative;
  }
  .names {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .t {
    font-family: var(--title);
    font-weight: 700;
    font-size: 18px;
    line-height: 1.2;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .plat {
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bars {
    display: flex;
    gap: 4px;
    height: 24px;
    min-width: 0;
  }
  .bars span {
    width: var(--w);
    min-width: 6px;
    transform-origin: left;
    animation: grow 0.6s var(--ease) both;
    animation-delay: calc(var(--i) * 40ms + var(--j, 0) * 90ms + 150ms);
  }
  :global(:root[data-theme="pixel"]) .bars span {
    animation-timing-function: steps(5);
  }
  :global(:root[data-theme="pixel"]) .keep {
    background: var(--fg);
  }
  :global(:root[data-theme="pixel"]) .extra {
    background: repeating-linear-gradient(90deg, var(--fg) 0 3px, transparent 3px 6px);
    box-shadow: inset 0 0 0 3px var(--fg);
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
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
    white-space: nowrap;
  }
  .save b {
    font-family: var(--title);
    font-size: 20px;
    font-variant-numeric: tabular-nums;
  }
  :global(:root[data-theme="paper"]) .save b {
    color: var(--accent);
  }
  .save .label {
    color: var(--faint);
  }
  .files {
    grid-column: 1 / -1;
    padding-left: 88px;
    color: var(--faint);
  }
  .mono {
    font-family: "Cascadia Mono", Consolas, monospace;
    font-size: 12px;
    line-height: 1.6;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* ---- Buttons: mouse hover and disabled ---- */
  .btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  :global(:root[data-theme="pixel"]) .btn:not(:disabled):hover,
  :global(:root[data-theme="pixel"]) .btn.on {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .btn.primary:not(:disabled):hover {
    outline: var(--border) solid var(--fg);
    outline-offset: 4px;
  }
  :global(:root[data-theme="paper"]) .btn:not(:disabled):hover {
    box-shadow: 3px 3px 0 var(--fg);
    transform: translate(-1px, -1px);
  }
  :global(:root[data-theme="paper"]) .btn.on {
    color: var(--accent);
    border-color: var(--accent);
  }

  @media (max-width: 1200px) {
    .amount {
      font-size: 56px;
    }
    .broom {
      width: 88px;
      height: 88px;
    }
    .thead,
    .pile {
      grid-template-columns: minmax(220px, 34%) 1fr 120px;
    }
  }
</style>
