<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type CatalogView, type Emulator } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import Icon from "./Icon.svelte";

  let { emulators, onchange }: { emulators: Emulator[]; onchange: () => void } = $props();

  let cat = $state<CatalogView | null>(null);
  let busy = $state<Record<string, string>>({});
  let editing = $state<Record<string, string>>({});

  onMount(() => {
    api.catalog().then((c) => (cat = c));
    const un = listen<{ id: string; message: string; done: boolean; error: string | null }>("install", (e) => {
      const { id, message, done, error } = e.payload;
      if (done) {
        const { [id]: _, ...rest } = busy;
        busy = rest;
        if (error) app.toast(error, "error", 9000);
        else {
          app.toast("Emulator installed", "ok");
          onchange();
        }
      } else busy = { ...busy, [id]: message };
    });
    return () => un.then((f) => f());
  });

  const have = $derived(new Set(emulators.map((e) => e.id.replace(/-\d+$/, ""))));
  const coveredPlatforms = $derived(new Set(emulators.flatMap((e) => e.platforms)));
  const available = $derived((cat?.emulators ?? []).filter((d) => !have.has(d.id)));

  // Arguments are edited as a single line; quote tokens that contain spaces.
  const join = (args: string[]) => args.map((a) => (/\s/.test(a) ? `"${a}"` : a)).join(" ");
  const split = (s: string) => [...s.matchAll(/"([^"]*)"|(\S+)/g)].map((m) => m[1] ?? m[2]);

  async function saveArgs(e: Emulator) {
    const v = editing[e.id];
    await api.setEmulatorArgs(e.id, v === undefined ? null : split(v));
    const { [e.id]: _, ...rest } = editing;
    editing = rest;
    onchange();
    app.toast(`${e.name} launch options saved`, "ok");
  }
  async function resetArgs(e: Emulator) {
    await api.setEmulatorArgs(e.id, null);
    onchange();
  }
  function install(id: string) {
    busy = { ...busy, [id]: "Starting" };
    api.installLatest(id).catch(() => {});
  }
</script>

<div class="page">
  <div class="eyebrow">Emulators</div>
  <h1>What plays your games</h1>
  <p class="lede">
    Emulators found in your folders are used where they are and are never moved or changed. Missing one? OnionRing can
    install the official build into your data folder and keep it updated.
  </p>

  <h3 class="section">Found on this PC</h3>
  {#if emulators.length === 0}
    <p class="muted">None yet. Add your emulation folder in Settings, or install one below.</p>
  {/if}
  <div class="rows">
    {#each emulators as e (e.id)}
      <div class="row card" class:missing={e.missing}>
        <div class="head">
          <div class="name">
            <b>{e.name}</b>
            {#if e.version}<span class="chip">v{e.version}</span>{/if}
            {#if e.managed}<span class="chip">Installed by OnionRing</span>{/if}
            {#if e.missing}<span class="chip warn">Not found</span>{/if}
          </div>
          <div class="plats">
            {#each e.platforms as pid (pid)}
              {@const p = app.platforms.get(pid)}
              <span class="chip"><span class="dot" style:background={p?.color}></span>{p?.name ?? pid}</span>
            {/each}
          </div>
        </div>
        <button class="path mono" title="Show in folder" onclick={() => api.reveal(e.exe)}>{e.exe}</button>
        <div class="args">
          <span class="eyebrow">Launch options</span>
          <input
            class="field mono"
            value={editing[e.id] ?? join(e.args)}
            oninput={(ev) => (editing = { ...editing, [e.id]: (ev.target as HTMLInputElement).value })}
            spellcheck="false"
          />
          {#if editing[e.id] !== undefined}
            <button class="btn sm primary" onclick={() => saveArgs(e)}>Save</button>
          {:else if join(e.args) !== join(e.defaultArgs)}
            <button class="btn sm" onclick={() => resetArgs(e)}><Icon name="undo" size={14} /> Default</button>
          {/if}
        </div>
        {#if e.notes}<p class="note"><Icon name="sparkle" size={15} /> {e.notes}</p>{/if}
      </div>
    {/each}
  </div>
  <p class="hint"><code>{"{rom}"}</code> is replaced by the game file. Each space-separated part is passed as its own argument, so paths with spaces are safe.</p>

  {#if available.length}
    <h3 class="section">Get more</h3>
    <div class="grid">
      {#each available as d (d.id)}
        {@const needed = d.platforms.some((p) => !coveredPlatforms.has(p))}
        <div class="tile card">
          <div class="tname"><b>{d.name}</b>{#if !needed}<span class="chip">Covered</span>{/if}</div>
          <div class="plats">
            {#each d.platforms as pid (pid)}
              <span class="chip"><span class="dot" style:background={app.platforms.get(pid)?.color}></span>{app.platforms.get(pid)?.short ?? pid}</span>
            {/each}
          </div>
          {#if d.requires}<p class="req">{d.requires}</p>{/if}
          {#if d.notes}<p class="req">{d.notes}</p>{/if}
          <div class="tbtns">
            {#if busy[d.id]}
              <span class="working"><span class="spinner"></span>{busy[d.id]}</span>
            {:else if d.repo}
              <button class="btn sm primary" onclick={() => install(d.id)}><Icon name="download" size={14} /> Install</button>
            {:else if d.homepage}
              <button class="btn sm" onclick={() => openUrl(d.homepage!)}><Icon name="external" size={14} /> Download page</button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .section {
    margin: 30px 0 12px;
    font-size: 15px;
    font-weight: 650;
  }
  .muted {
    color: var(--faint);
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .row {
    padding: 16px 18px;
    border-radius: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .row.missing {
    opacity: 0.6;
  }
  .head {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }
  .name {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 15px;
  }
  .plats {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .chip.warn {
    color: var(--warn);
  }
  .path {
    text-align: left;
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .path:hover {
    color: var(--muted);
    text-decoration: underline;
  }
  .args {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .args .field {
    flex: 1;
    height: 32px;
  }
  .note {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    color: #f5d68a;
    font-size: 13px;
  }
  .hint {
    margin-top: 12px;
    color: var(--faint);
    font-size: 12.5px;
  }
  code {
    color: var(--muted);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 12px;
  }
  .tile {
    padding: 16px;
    border-radius: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .tname {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .req {
    margin: 0;
    font-size: 12.5px;
    color: var(--faint);
  }
  .tbtns {
    margin-top: auto;
  }
  .working {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .spinner {
    width: 13px;
    height: 13px;
    border-radius: 50%;
    border: 2px solid var(--line-2);
    border-top-color: var(--coral);
    animation: rot 0.8s linear infinite;
  }
  @keyframes rot {
    to {
      transform: rotate(360deg);
    }
  }
</style>
