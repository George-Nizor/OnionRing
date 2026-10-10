<script lang="ts">
  import { onMount, tick } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type CatalogView, type Emulator, type EmulatorDef, type PortDef, type UpdateInfo } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { focusFirst } from "$lib/nav";
  import { norm } from "$lib/format";
  import Glyph from "./Glyph.svelte";

  let { emulators }: { emulators: Emulator[] } = $props();

  let cat = $state<CatalogView | null>(null);
  let updates = $state<Record<string, UpdateInfo>>({});
  let consoleFilter = $state<string | null>(null);
  let sheet = $state<{ kind: "port"; p: PortDef } | { kind: "emu"; e: EmulatorDef } | null>(null);
  let sheetEl = $state<HTMLElement>();
  const tab = $derived(app.view.tab ?? "ports");

  async function load() {
    cat = await api.catalog();
  }
  onMount(() => {
    load();
    api.checkUpdates().then((u) => (updates = Object.fromEntries(u.map((x) => [x.id, x])))).catch(() => {});
  });
  // Reload the catalog when an install finishes.
  $effect(() => {
    void Object.keys(app.installing).length;
    load();
  });

  const installed = $derived(new Map((cat?.installed ?? []).map((a) => [a.id, a])));
  const consoles = $derived.by(() => {
    const c = new Map<string, number>();
    for (const p of cat?.ports ?? []) for (const o of p.original) c.set(o, (c.get(o) ?? 0) + 1);
    return [...c.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8);
  });
  const ports = $derived(
    (cat?.ports ?? [])
      .filter((p) => !consoleFilter || p.original.includes(consoleFilter))
      .filter((p) => !app.query || norm(`${p.game} ${p.name}`).includes(norm(app.query)))
      .sort((a, b) => Number(installed.has(b.id)) - Number(installed.has(a.id)) || Number(b.featured) - Number(a.featured) || a.game.localeCompare(b.game)),
  );
  const short = (raw: string) => {
    const o = raw.replace(/\s*\(.*?\)\s*/g, " ").trim();
    return (
      ({ "Nintendo 64": "N64", PlayStation: "PS1", "PlayStation 2": "PS2", "PlayStation 4": "PS4", GameCube: "GC", "Super Nintendo": "SNES", "Sega Dreamcast": "DC", "Nintendo Entertainment System": "NES" } as Record<string, string>)[o] ??
      (o.length > 10 ? o.split(" ").map((w) => w[0]).join("").toUpperCase() : o)
    );
  };

  const found = (e: EmulatorDef) => emulators.find((x) => x.id === e.id || x.id.startsWith(`${e.id}-`));

  async function showSheet(s: NonNullable<typeof sheet>) {
    sheet = s;
    await tick();
    focusFirst(sheetEl);
  }
  function install(id: string) {
    app.installing = { ...app.installing, [id]: "Starting" };
    api.installLatest(id).catch(() => {});
  }
  async function fromFile(id: string) {
    const file = await open({ multiple: false, filters: [{ name: "Archives", extensions: ["zip", "7z"] }] });
    if (typeof file === "string") {
      app.installing = { ...app.installing, [id]: "Starting" };
      api.installFromFile(id, file).catch(() => {});
    }
  }
  const pct = (m: string | undefined) => {
    const x = m?.match(/(\d+)%/);
    return x ? Number(x[1]) / 100 : m ? 0.15 : 0;
  };
</script>

<div class="get">
  <div class="tabs">
    {#each [["ports", "cart", "Ports"], ["emulators", "pad", "Emulators"], ["browser", "globe", "Browser"]] as [id, icon, label] (id)}
      <button class="tab" class:on={tab === id} data-nav onclick={() => { app.view = { ...app.view, tab: id }; sheet = null; }}>
        <Glyph name={icon} size={36} /><span>{label}</span>
      </button>
    {/each}
  </div>

  {#if tab === "ports"}
    <div class="filters">
      <button class="f" class:on={!consoleFilter} data-nav onclick={() => (consoleFilter = null)}>All <em>{cat?.ports.length ?? ""}</em></button>
      {#each consoles as [c, n] (c)}
        <button class="f" class:on={consoleFilter === c} data-nav onclick={() => (consoleFilter = c)} title={c}>{short(c)} <em>{n}</em></button>
      {/each}
    </div>
    <div class="grid">
      {#each ports as p, i (p.id)}
        {@const inst = installed.get(p.id)}
        {@const busy = app.installing[p.id]}
        <button class="cart" data-nav style:--i={Math.min(i, 20)} onclick={() => showSheet({ kind: "port", p })}
          onmouseenter={(e) => (e.currentTarget as HTMLElement).focus({ preventScroll: true })}>
          <span class="top label">{p.original.map(short).join(" ")}</span>
          <span class="name">{p.game}</span>
          <span class="by">{p.name}</span>
          <span class="state">
            {#if busy}
              <span class="prog" style:--p={pct(busy)}></span>
            {:else if inst}
              <Glyph name={updates[p.id]?.available ? "refresh" : "check"} size={24} />
            {:else}
              <Glyph name="get" size={24} />
            {/if}
          </span>
        </button>
      {/each}
    </div>
  {:else if tab === "emulators"}
    <div class="grid emus">
      {#each cat?.emulators ?? [] as e, i (e.id)}
        {@const f = found(e)}
        {@const busy = app.installing[e.id]}
        <button class="cart" data-nav style:--i={i} onclick={() => showSheet({ kind: "emu", e })}
          onmouseenter={(ev) => (ev.currentTarget as HTMLElement).focus({ preventScroll: true })}>
          <span class="top label">{e.platforms.map((p) => app.platforms.get(p)?.short ?? p).join(" ")}</span>
          <span class="name">{e.name}</span>
          <span class="by">{f ? (f.managed ? "Installed by OnionRing" : "Found on this PC") : e.repo ? "Ready to install" : "From its website"}</span>
          <span class="state">
            {#if busy}<span class="prog" style:--p={pct(busy)}></span>
            {:else if f}<Glyph name={f.managed ? "check" : "folder"} size={24} />
            {:else}<Glyph name={e.repo ? "get" : "globe"} size={24} />{/if}
          </span>
        </button>
      {/each}
    </div>
  {:else}
    <div class="empty">
      <Glyph name="globe" size={96} />
      <p class="label">Checking which browser games are safe to add</p>
    </div>
  {/if}
</div>

{#if sheet}
  <div class="scrim" role="presentation" onclick={() => (sheet = null)}></div>
  <aside class="sheet" data-layer bind:this={sheetEl}>
    {#if sheet.kind === "port"}
      {@const p = sheet.p}
      {@const inst = installed.get(p.id)}
      {@const up = updates[p.id]}
      <span class="label">{p.kind} · {p.original.join(", ")}</span>
      <h2>{p.game}</h2>
      <div class="by">{p.name}</div>
      <div class="facts">
        {#if inst}<span><Glyph name="check" size={24} />{inst.version}{inst.source.verified ? ", checked" : ""}</span>{/if}
        {#if p.licence}<span><Glyph name="info" size={24} />{p.licence}</span>{/if}
      </div>
      {#if p.requires}<p class="needs"><Glyph name="alert" size={24} /><span>{p.requires}</span></p>{/if}
      <div class="btns">
        {#if app.installing[p.id]}
          <div class="prog wide" style:--p={pct(app.installing[p.id])}></div>
          <span class="label">{app.installing[p.id]}</span>
        {:else}
          <button class="btn big primary" data-nav onclick={() => install(p.id)}>
            <Glyph name={inst ? "refresh" : "get"} size={24} />{inst ? (up?.available ? `Update to ${up.latest}` : "Reinstall") : "Get"}
          </button>
          <button class="btn big" data-nav onclick={() => fromFile(p.id)}><Glyph name="zip" size={24} />From a zip</button>
          <button class="btn big icon" data-nav onclick={() => openUrl(`https://github.com/${p.repo}`)} aria-label="Project page"><Glyph name="globe" size={24} /></button>
        {/if}
      </div>
    {:else}
      {@const e = sheet.e}
      {@const f = found(e)}
      <span class="label">{e.platforms.map((p) => app.platforms.get(p)?.name ?? p).join(", ")}</span>
      <h2>{e.name}</h2>
      {#if f}<div class="by">{f.managed ? "Installed by OnionRing" : "Found on this PC"}{f.version ? `, version ${f.version}` : ""}</div>{/if}
      {#if e.notes}<p class="needs"><Glyph name="info" size={24} /><span>{e.notes}</span></p>{/if}
      {#if e.requires}<p class="needs"><Glyph name="alert" size={24} /><span>{e.requires}</span></p>{/if}
      <div class="btns">
        {#if app.installing[e.id]}
          <div class="prog wide" style:--p={pct(app.installing[e.id])}></div>
          <span class="label">{app.installing[e.id]}</span>
        {:else if e.repo}
          <button class="btn big primary" data-nav onclick={() => install(e.id)}><Glyph name="get" size={24} />{f?.managed ? "Update" : "Install"}</button>
        {:else if e.homepage}
          <button class="btn big" data-nav onclick={() => openUrl(e.homepage!)}><Glyph name="globe" size={24} />Website</button>
        {/if}
      </div>
    {/if}
  </aside>
{/if}

<style>
  .get {
    height: 100%;
    overflow-y: auto;
    padding: 24px 56px 60px;
  }
  .tabs {
    display: flex;
    gap: 10px;
    margin-bottom: 22px;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 60px;
    padding: 0 20px;
    font-family: var(--title);
    font-size: 22px;
    font-weight: 800;
  }
  :global(:root[data-theme="pixel"]) .tab.on {
    outline: var(--border) solid var(--line);
  }
  :global(:root[data-theme="paper"]) .tab.on {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .tab.on::before {
    content: "";
    position: absolute;
    left: 20px;
    right: 20px;
    bottom: 4px;
    height: 3px;
    background: var(--accent);
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 22px;
  }
  .f {
    height: 38px;
    padding: 0 12px;
    font-family: var(--label);
    font-weight: 700;
    font-size: 14px;
  }
  .f em {
    font-style: normal;
    opacity: 0.6;
    margin-left: 4px;
  }
  :global(:root[data-theme="pixel"]) .f.on {
    outline: var(--border) solid var(--line);
  }
  :global(:root[data-theme="paper"]) .f.on {
    color: var(--accent-2);
    box-shadow: inset 0 -2px 0 var(--accent-2);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
    gap: 16px;
  }
  .cart {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    min-height: 150px;
    padding: 14px 16px;
    text-align: left;
    content-visibility: auto;
    contain-intrinsic-size: 210px 150px;
    animation: rise 0.4s var(--ease) both;
    animation-delay: calc(var(--i) * 18ms);
  }
  .cart .name {
    font-family: var(--title);
    font-weight: 800;
    font-size: 21px;
    line-height: 1.05;
  }
  .cart .by,
  .sheet .by {
    font-family: var(--label);
    font-size: 13px;
    color: var(--faint);
  }
  .cart:focus-visible .by {
    color: inherit;
  }
  .state {
    margin-top: auto;
    align-self: flex-end;
    min-height: 24px;
    display: flex;
    align-items: center;
  }
  :global(:root[data-theme="pixel"]) .cart {
    border: var(--border) solid var(--line);
    animation-timing-function: steps(3);
  }
  :global(:root[data-theme="paper"]) .cart {
    background: var(--panel);
    box-shadow: 0 0 0 1px var(--line);
    transition: transform 0.3s var(--ease), box-shadow 0.3s;
  }
  :global(:root[data-theme="paper"]) .cart:focus-visible {
    transform: translateY(-4px);
    box-shadow:
      0 0 0 2px var(--line),
      5px 5px 0 var(--line);
  }
  :global(:root[data-theme="paper"]) .cart:focus-visible::after {
    display: none;
  }
  :global(:root[data-theme="paper"]) .cart .top {
    color: var(--accent-2);
  }
  /* progress: segmented in pixel, a ruled line in paper */
  .prog {
    width: 90px;
    height: 14px;
    position: relative;
  }
  .prog.wide {
    width: 100%;
    height: 22px;
  }
  .prog::before {
    content: "";
    position: absolute;
    inset: 0;
    width: calc(var(--p) * 100%);
    transition: width 0.4s;
  }
  :global(:root[data-theme="pixel"]) .prog {
    border: 2px solid currentColor;
  }
  :global(:root[data-theme="pixel"]) .prog::before {
    background: repeating-linear-gradient(90deg, currentColor 0 8px, transparent 8px 11px);
    margin: 2px;
  }
  :global(:root[data-theme="paper"]) .prog {
    border-bottom: 1.5px solid currentColor;
  }
  :global(:root[data-theme="paper"]) .prog::before {
    background: var(--accent);
    height: 4px;
    top: auto;
    bottom: -2px;
  }
  .empty {
    display: grid;
    place-items: center;
    gap: 16px;
    padding: 80px 0;
    color: var(--faint);
  }
  .empty :global(svg) {
    animation: spinslow 6s linear infinite;
  }
  :global(:root[data-theme="pixel"]) .empty :global(svg) {
    animation: spinslow 2s steps(8) infinite;
  }
  @keyframes spinslow {
    to {
      transform: rotate(360deg);
    }
  }

  .scrim {
    position: fixed;
    inset: 0;
    z-index: 44;
  }
  :global(:root[data-theme="pixel"]) .scrim {
    background: repeating-linear-gradient(0deg, rgba(0, 0, 0, 0.85) 0 2px, rgba(0, 0, 0, 0.6) 2px 4px);
  }
  :global(:root[data-theme="paper"]) .scrim {
    background: rgba(238, 235, 227, 0.7);
  }
  .sheet {
    position: fixed;
    top: var(--bar);
    right: 0;
    bottom: var(--hints);
    z-index: 45;
    width: min(520px, 92vw);
    padding: 36px 34px;
    background: var(--bg);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  :global(:root[data-theme="pixel"]) .sheet {
    border-left: var(--border) solid var(--line);
    animation: sheetin 0.2s steps(4) both;
  }
  :global(:root[data-theme="paper"]) .sheet {
    background: var(--panel);
    border-left: 2px solid var(--line);
    animation: sheetin 0.35s var(--ease) both;
  }
  @keyframes sheetin {
    from {
      transform: translateX(100%);
    }
  }
  .sheet h2 {
    margin: 0;
    font-family: var(--title);
    font-weight: 800;
    font-size: 40px;
    line-height: 1.02;
  }
  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 24px;
    margin-top: 10px;
  }
  .facts span,
  .needs {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: var(--label);
    font-size: 15px;
  }
  .needs {
    margin: 10px 0 0;
    align-items: flex-start;
  }
  :global(:root[data-theme="paper"]) .needs {
    color: var(--accent);
  }
  .btns {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 24px;
  }
</style>
