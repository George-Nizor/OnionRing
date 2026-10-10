<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type CatalogView, type PortDef, type UpdateInfo } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { bytes, norm } from "$lib/format";
  import Icon from "./Icon.svelte";

  let { onupdates }: { onupdates?: (n: number) => void } = $props();

  let cat = $state<CatalogView | null>(null);
  let updates = $state<Record<string, UpdateInfo>>({});
  let busy = $state<Record<string, string>>({});
  let query = $state("");
  let consoleFilter = $state<string | null>(null);
  let kindFilter = $state<string | null>(null);
  let open_ = $state<PortDef | null>(null);

  async function load() {
    cat = await api.catalog();
  }

  onMount(() => {
    load();
    api.checkUpdates().then((u) => {
      updates = Object.fromEntries(u.map((x) => [x.id, x]));
      onupdates?.(u.filter((x) => x.available).length);
    }).catch(() => {});
    const un = listen<{ id: string; message: string; done: boolean; error: string | null }>("install", (e) => {
      const { id, message, done, error } = e.payload;
      if (done) {
        const { [id]: _, ...rest } = busy;
        busy = rest;
        if (error) app.toast(error, "error", 9000);
        else app.toast("Installed and added to your library", "ok");
        load();
      } else busy = { ...busy, [id]: message };
    });
    return () => un.then((f) => f());
  });

  const installed = $derived(new Map((cat?.installed ?? []).map((a) => [a.id, a])));
  const all = $derived(cat?.ports ?? []);
  const mine = $derived(all.filter((p) => installed.has(p.id)));
  const consoles = $derived.by(() => {
    const c = new Map<string, number>();
    for (const p of all) for (const o of p.original) c.set(o, (c.get(o) ?? 0) + 1);
    return [...c.entries()].sort((a, b) => b[1] - a[1]).slice(0, 10);
  });
  const kinds = $derived([...new Set(all.map((p) => p.kind))].sort());
  const browse = $derived.by(() => {
    const q = norm(query);
    return all.filter(
      (p) =>
        !installed.has(p.id) &&
        (!consoleFilter || p.original.includes(consoleFilter)) &&
        (!kindFilter || p.kind === kindFilter) &&
        (!q || norm(`${p.game} ${p.name}`).includes(q)),
    );
  });

  function install(id: string) {
    busy = { ...busy, [id]: "Starting" };
    api.installLatest(id).catch(() => {});
  }
  async function fromFile(id: string) {
    const file = await open({ multiple: false, filters: [{ name: "Archives", extensions: ["zip", "7z"] }] });
    if (typeof file === "string") {
      busy = { ...busy, [id]: "Starting" };
      api.installFromFile(id, file).catch(() => {});
    }
  }
  const gameFor = (id: string) => app.games.find((x) => x.appId === id);
</script>

<div class="page">
  <header class="top">
    <div>
      <div class="eyebrow">Ports &amp; recomps</div>
      <h1>Games rebuilt to run natively</h1>
      <p class="lede">
        {all.length} community ports, recompilations and decompilations with Windows builds. OnionRing downloads the official
        release, checks it against the hash the project published, and keeps your saves across updates. You bring your own game
        files.
      </p>
    </div>
  </header>

  {#if mine.length}
    <h2>Installed</h2>
    <div class="mine">
      {#each mine as p (p.id)}
        {@const inst = installed.get(p.id)!}
        {@const up = updates[p.id]}
        {@const g = gameFor(p.id)}
        <article class="big" style:--a={p.accent ?? "#ff5d7a"}>
          <div class="glow"></div>
          <div class="body">
            <div class="kick">{p.kind} · {p.original.join(", ")}</div>
            <div class="game">{p.game}</div>
            <div class="by">{p.name}</div>
            <div class="state">
              {#if busy[p.id]}
                <span class="working"><span class="spinner"></span>{busy[p.id]}</span>
              {:else}
                <span class="ver" class:ok={inst.source.verified}>
                  <Icon name={inst.source.verified ? "shield" : "check"} size={15} />
                  {inst.version} · {bytes(inst.size)}{inst.source.verified ? " · verified" : ""}
                </span>
              {/if}
            </div>
          </div>
          <div class="btns">
            {#if up?.available}
              <button class="btn sm primary" onclick={() => install(p.id)} disabled={!!busy[p.id]}>
                <Icon name="download" size={14} /> Update to {up.latest}
              </button>
            {/if}
            {#if g}
              <button class="btn sm" onclick={() => app.play(g)}><Icon name="play" size={13} /> Play</button>
              <button class="btn sm ghost icon" title="Show in folder" onclick={() => api.reveal(g.launchPath)}><Icon name="folder" size={15} /></button>
            {/if}
            <button class="btn sm ghost icon" title="Details" onclick={() => (open_ = p)}><Icon name="more" size={18} stroke={3} /></button>
          </div>
        </article>
      {/each}
    </div>
  {/if}

  <div class="browse-head">
    <h2>Browse <span>{browse.length}</span></h2>
    <label class="find">
      <Icon name="search" size={16} />
      <input placeholder="Find a game or project" bind:value={query} spellcheck="false" />
    </label>
  </div>
  <div class="filters">
    <button class="f" class:on={!consoleFilter} onclick={() => (consoleFilter = null)}>All consoles</button>
    {#each consoles as [c, n] (c)}
      <button class="f" class:on={consoleFilter === c} onclick={() => (consoleFilter = consoleFilter === c ? null : c)}>{c} <em>{n}</em></button>
    {/each}
  </div>
  <div class="filters">
    <button class="f" class:on={!kindFilter} onclick={() => (kindFilter = null)}>Every kind</button>
    {#each kinds as k (k)}
      <button class="f" class:on={kindFilter === k} onclick={() => (kindFilter = kindFilter === k ? null : k)}>{k}</button>
    {/each}
  </div>

  <div class="grid">
    {#each browse as p (p.id)}
      <article class="tile" style:--a={p.accent ?? "#9d6bff"}>
        <button class="open" onclick={() => (open_ = p)}>
          <div class="kick">{p.original[0] ?? ""}{p.original.length > 1 ? ` +${p.original.length - 1}` : ""}</div>
          <div class="game">{p.game}</div>
          <div class="by">{p.name}</div>
        </button>
        <div class="foot">
          <span class="kind">{p.kind}</span>
          {#if busy[p.id]}
            <span class="working"><span class="spinner"></span>{busy[p.id]}</span>
          {:else}
            <button class="get" onclick={() => install(p.id)}><Icon name="download" size={14} /> Get</button>
          {/if}
        </div>
      </article>
    {/each}
  </div>
</div>

{#if open_}
  {@const p = open_}
  {@const inst = installed.get(p.id)}
  <div class="scrim" role="presentation" onclick={() => (open_ = null)}></div>
  <aside class="sheet" style:--a={p.accent ?? "#9d6bff"}>
    <div class="glow"></div>
    <button class="x" onclick={() => (open_ = null)} aria-label="Close"><Icon name="close" size={18} /></button>
    <div class="kick">{p.kind} · {p.original.join(", ")}</div>
    <h3>{p.game}</h3>
    <div class="by">{p.name}</div>
    {#if p.blurb}<p class="blurb">{p.blurb}</p>{/if}
    {#if p.requires}<p class="req"><Icon name="alert" size={15} /> {p.requires}</p>{/if}
    <dl>
      <dt>Source</dt>
      <dd><button class="link" onclick={() => openUrl(`https://github.com/${p.repo}`)}>github.com/{p.repo} <Icon name="external" size={13} /></button></dd>
      {#if p.licence}<dt>Licence</dt><dd>{p.licence}</dd>{/if}
      {#if inst}
        <dt>Installed</dt><dd>{inst.version}{inst.source.verified ? " (hash verified)" : ""}</dd>
        <dt>Archive</dt><dd class="mono">{inst.source.archive}</dd>
      {/if}
    </dl>
    <div class="sheet-btns">
      {#if busy[p.id]}
        <span class="working"><span class="spinner"></span>{busy[p.id]}</span>
      {:else}
        <button class="btn primary" onclick={() => install(p.id)}><Icon name="download" size={16} /> {inst ? "Reinstall latest" : "Get latest"}</button>
        <button class="btn" onclick={() => fromFile(p.id)}>Install from a downloaded zip…</button>
      {/if}
    </div>
  </aside>
{/if}

<style>
  h2 {
    margin: 34px 0 14px;
    font-family: var(--display);
    font-size: 19px;
    font-weight: 700;
    letter-spacing: -0.015em;
  }
  h2 span {
    color: var(--faint);
    margin-left: 4px;
  }
  .kick {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--a) 55%, white);
    opacity: 0.85;
  }
  .game {
    font-family: var(--display);
    font-weight: 750;
    letter-spacing: -0.03em;
    line-height: 1.05;
  }
  .by {
    color: var(--muted);
    font-size: 13px;
    margin-top: 4px;
  }
  .glow {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(80% 120% at 100% 0%, color-mix(in srgb, var(--a) 32%, transparent), transparent 60%),
      linear-gradient(135deg, color-mix(in srgb, var(--a) 10%, #120f17), #0d0b11);
    z-index: -1;
  }

  .mine {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(440px, 1fr));
    gap: 16px;
  }
  .big {
    position: relative;
    isolation: isolate;
    overflow: hidden;
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    min-height: 190px;
    padding: 24px;
    border-radius: 20px;
    border: 1px solid var(--line-2);
  }
  .big .game {
    font-size: 34px;
    margin-top: 6px;
  }
  .state {
    margin-top: 14px;
    font-size: 13px;
  }
  .ver {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    color: var(--muted);
  }
  .ver.ok {
    color: var(--ok);
  }
  .btns {
    display: flex;
    gap: 6px;
  }

  .browse-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .find {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 300px;
    height: 40px;
    padding: 0 14px;
    border-radius: 99px;
    background: var(--surface);
    border: 1px solid var(--line);
    color: var(--faint);
    margin-top: 20px;
  }
  .find:focus-within {
    border-color: var(--line-2);
  }
  .find input {
    flex: 1;
    border: 0;
    outline: 0;
    background: none;
    color: var(--text);
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 8px;
  }
  .f {
    height: 32px;
    padding: 0 13px;
    border-radius: 99px;
    font-size: 13px;
    font-weight: 600;
    color: var(--muted);
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .f:hover {
    color: var(--text);
  }
  .f.on {
    background: #fff;
    color: #0a090d;
    border-color: #fff;
  }
  .f em {
    font-style: normal;
    opacity: 0.55;
    margin-left: 3px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 12px;
    margin-top: 16px;
  }
  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    border-radius: 16px;
    background: linear-gradient(160deg, color-mix(in srgb, var(--a) 9%, #131018), #0e0c12 70%);
    border: 1px solid var(--line);
    overflow: hidden;
    content-visibility: auto;
    contain-intrinsic-size: 230px 150px;
    transition:
      border-color 0.2s,
      transform 0.25s var(--ease);
  }
  .tile:hover {
    border-color: var(--line-2);
    transform: translateY(-2px);
  }
  .open {
    flex: 1;
    text-align: left;
    padding: 16px 16px 10px;
  }
  .tile .game {
    font-size: 19px;
    margin-top: 6px;
  }
  .tile .by {
    font-size: 12px;
    display: -webkit-box;
    -webkit-line-clamp: 1;
    line-clamp: 1;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px 12px 16px;
  }
  .kind {
    font-size: 11.5px;
    color: var(--faint);
    font-weight: 600;
  }
  .get {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 13px;
    border-radius: 99px;
    background: #fff;
    color: #0a090d;
    font-size: 12.5px;
    font-weight: 700;
    opacity: 0.9;
  }
  .get:hover {
    opacity: 1;
  }
  .working {
    display: inline-flex;
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

  .scrim {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(4px);
    animation: fade 0.2s;
  }
  .sheet {
    position: fixed;
    top: 72px;
    right: 12px;
    bottom: 12px;
    z-index: 61;
    width: min(480px, 92vw);
    padding: 30px 28px;
    border-radius: 22px;
    border: 1px solid var(--line-2);
    overflow-y: auto;
    isolation: isolate;
    box-shadow: 0 40px 80px -20px rgba(0, 0, 0, 0.9);
    animation: slide 0.35s var(--ease);
  }
  @keyframes slide {
    from {
      transform: translateX(30px);
      opacity: 0;
    }
  }
  .x {
    position: absolute;
    top: 14px;
    right: 14px;
    width: 34px;
    height: 34px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: var(--muted);
  }
  .x:hover {
    background: var(--glass);
    color: #fff;
  }
  h3 {
    margin: 8px 0 0;
    font-family: var(--display);
    font-size: 36px;
    font-weight: 750;
    letter-spacing: -0.035em;
    line-height: 1.02;
  }
  .blurb {
    color: var(--muted);
    margin: 18px 0 0;
  }
  .req {
    display: flex;
    gap: 9px;
    margin: 18px 0 0;
    padding: 12px 14px;
    border-radius: 12px;
    background: rgba(251, 191, 36, 0.07);
    border: 1px solid rgba(251, 191, 36, 0.2);
    color: #f5d68a;
    font-size: 13px;
  }
  .req :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  dl {
    display: grid;
    grid-template-columns: 90px 1fr;
    gap: 10px 14px;
    margin: 24px 0;
    font-size: 13px;
  }
  dt {
    color: var(--faint);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .link {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--text);
    text-decoration: underline;
    text-decoration-color: var(--line-2);
    text-underline-offset: 3px;
  }
  .sheet-btns {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
</style>
