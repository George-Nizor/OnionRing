<script lang="ts">
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { api, type StorageReport } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { bytes } from "$lib/format";
  import Icon from "./Icon.svelte";

  let { onchange }: { onchange?: (reclaimable: number) => void } = $props();

  let report = $state<StorageReport | null>(null);
  let picked = $state<Set<string>>(new Set());
  let working = $state(false);

  async function load() {
    report = await api.storage();
    // Archives are clearly redundant; a second unpacked copy is left for the user to decide.
    picked = new Set(report.groups.flatMap((g) => g.redundant.filter((r) => r.reason.startsWith("Archive")).map((r) => r.path)));
    onchange?.(report.reclaimable);
  }
  onMount(load);

  const sizes = $derived(new Map((report?.groups ?? []).flatMap((g) => g.redundant.map((r) => [r.path, r.size] as const))));
  const selectedBytes = $derived([...picked].reduce((s, p) => s + (sizes.get(p) ?? 0), 0));

  function toggle(p: string) {
    const n = new Set(picked);
    if (n.has(p)) n.delete(p);
    else n.add(p);
    picked = n;
  }

  async function reclaim() {
    if (!report || !picked.size) return;
    const ok = await ask(
      `Move ${picked.size} redundant ${picked.size === 1 ? "copy" : "copies"} (${bytes(selectedBytes)}) into ${report.reclaimDir}?\n\nNothing is deleted. You can put everything back with “Restore everything”, or delete the Reclaim folder yourself once you are happy.`,
      { title: "Reclaim space", kind: "warning", okLabel: "Move to Reclaim" },
    );
    if (!ok) return;
    working = true;
    try {
      const [n, b] = await api.reclaim([...picked]);
      app.toast(`Moved ${n} ${n === 1 ? "copy" : "copies"} (${bytes(b)}) to Reclaim`, "ok");
      await load();
    } catch (e) {
      app.toast(String(e), "error", 9000);
    }
    working = false;
  }

  async function restore() {
    working = true;
    try {
      const n = await api.restoreReclaimed();
      app.toast(`Restored ${n} ${n === 1 ? "item" : "items"}`, "ok");
      await load();
    } catch (e) {
      app.toast(String(e), "error", 9000);
    }
    working = false;
  }
</script>

<div class="page">
  <div class="eyebrow">Storage</div>
  <h1>Same games, stored twice</h1>
  <p class="lede">
    OnionRing spotted games kept more than once: the archive they were downloaded in next to the unpacked copy, or a
    second extracted copy. The copy each game actually plays from is never touched.
  </p>

  {#if report}
    <div class="stats">
      <div class="stat big">
        <span class="eyebrow">Reclaimable</span>
        <b class="display">{bytes(report.reclaimable)}</b>
      </div>
      <div class="stat">
        <span class="eyebrow">Library on disk</span>
        <b>{bytes(report.librarySize)}</b>
      </div>
      <div class="stat">
        <span class="eyebrow">Held in Reclaim</span>
        <b>{bytes(report.held)}</b>
      </div>
      <div class="actions">
        <button class="btn primary" onclick={reclaim} disabled={working || !picked.size}>
          <Icon name="disk" size={16} /> Move {bytes(selectedBytes)} to Reclaim
        </button>
        {#if report.held > 0}
          <button class="btn" onclick={restore} disabled={working}><Icon name="undo" size={15} /> Restore everything</button>
          <button class="btn ghost" onclick={() => report && api.reveal(report.reclaimDir)}><Icon name="folder" size={15} /> Open Reclaim</button>
        {/if}
      </div>
    </div>

    {#if report.groups.length === 0}
      <div class="clean card"><Icon name="check" size={20} /> No duplicates. Every game is stored once.</div>
    {/if}

    <div class="groups">
      {#each report.groups as g (g.key)}
        {@const p = app.platforms.get(g.platform)}
        <section class="group card">
          <header>
            <span class="badge" style:--c={p?.color}>{p?.short}</span>
            <b>{g.title}</b>
            <span class="save">{bytes(g.reclaimable)}</span>
          </header>
          <div class="keep">
            <Icon name="check" size={14} />
            <span class="lbl">Plays from</span>
            <span class="path mono" title={g.keep}>{g.keep}</span>
          </div>
          {#each g.redundant as r (r.path)}
            <label class="item">
              <input type="checkbox" checked={picked.has(r.path)} onchange={() => toggle(r.path)} />
              <span class="what">
                <span class="path mono" title={r.path}>{r.path}</span>
                <span class="why">{r.reason}</span>
              </span>
              <span class="size">{bytes(r.size)}</span>
              <button class="btn sm ghost icon" onclick={(e) => { e.preventDefault(); api.reveal(r.path); }} title="Show in folder"><Icon name="folder" size={14} /></button>
            </label>
          {/each}
        </section>
      {/each}
    </div>
  {:else}
    <p class="muted">Measuring…</p>
  {/if}
</div>

<style>
  .stats {
    display: flex;
    align-items: flex-end;
    gap: 40px;
    flex-wrap: wrap;
    margin-bottom: 26px;
  }
  .stat {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .stat b {
    font-size: 18px;
  }
  .stat.big b {
    font-size: 44px;
    font-weight: 650;
    line-height: 1;
    background: var(--ring-linear);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .actions {
    margin-left: auto;
    display: flex;
    gap: 8px;
  }
  .clean {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 18px;
    color: var(--ok);
  }
  .groups {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .group {
    padding: 14px 16px;
    border-radius: 16px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }
  header b {
    flex: 1;
  }
  .save {
    font-weight: 700;
    color: var(--coral);
  }
  .badge {
    width: 36px;
    height: 20px;
    display: grid;
    place-items: center;
    border-radius: 6px;
    font-size: 9.5px;
    font-weight: 800;
    color: color-mix(in srgb, var(--c) 70%, white);
    background: color-mix(in srgb, var(--c) 16%, transparent);
  }
  .keep {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    color: var(--ok);
    font-size: 12.5px;
  }
  .lbl {
    flex: none;
    color: var(--muted);
  }
  .path {
    color: var(--faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px;
    border-radius: 10px;
    cursor: pointer;
  }
  .item:hover {
    background: var(--surface);
  }
  .item input {
    accent-color: var(--coral);
    width: 16px;
    height: 16px;
  }
  .what {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .why {
    font-size: 12px;
    color: var(--muted);
  }
  .size {
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }
  .muted {
    color: var(--faint);
  }
</style>
