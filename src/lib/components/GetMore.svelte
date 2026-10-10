<script lang="ts">
  import { onMount, tick } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type BrowserGame, type CatalogView, type Emulator, type EmulatorDef, type PortDef, type ReleaseDetails, type UpdateInfo } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { focusFirst } from "$lib/nav";
  import { bytes, norm } from "$lib/format";
  import Glyph from "./Glyph.svelte";
  import Pic from "./Pic.svelte";

  let { emulators }: { emulators: Emulator[] } = $props();

  let cat = $state<CatalogView | null>(null);
  let updates = $state<Record<string, UpdateInfo>>({});
  let consoleFilter = $state<string | null>(null);
  let sheet = $state<{ kind: "port"; p: PortDef } | { kind: "emu"; e: EmulatorDef } | { kind: "web"; b: BrowserGame } | { kind: "add" } | null>(null);

  /** What each kind of browser game is, as an icon and two words. */
  const STATUS: Record<BrowserGame["status"], [string, string]> = {
    official: ["check", "Free"],
    "open-source": ["globe", "Open source"],
    shareware: ["info", "Free demo"],
    "own-files": ["folder", "Your files"],
    "fan-original": ["star", "Fan made"],
    custom: ["plus", "Yours"],
  };
  let newTitle = $state("");
  let newUrl = $state("");
  async function addOwn() {
    try {
      const g = await api.addBrowserGame(newTitle, newUrl);
      newTitle = newUrl = "";
      await load();
      app.toast(`Added ${g.title}`, "ok");
      sheet = { kind: "web", b: g };
    } catch (e) {
      app.toast(String(e), "error");
    }
  }
  async function removeOwn(b: BrowserGame) {
    await api.removeBrowserGame(b.id);
    sheet = null;
    await load();
  }
  function playWeb(b: BrowserGame) {
    api.openBrowserGame(b.id).catch((e) => app.toast(String(e), "error"));
  }
  let sheetEl = $state<HTMLElement>();
  const tab = $derived(app.view.tab ?? "ports");

  async function load() {
    cat = await api.catalog();
    // Arriving from search with something to show.
    const item = app.view.item;
    if (item && !sheet) {
      const p = cat.ports.find((x) => x.id === item);
      const e = cat.emulators.find((x) => x.id === item);
      const b = cat.browser.find((x) => x.id === item);
      if (p) showSheet({ kind: "port", p });
      else if (e) showSheet({ kind: "emu", e });
      else if (b) showSheet({ kind: "web", b });
    }
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

  let rel = $state<ReleaseDetails | null>(null);
  let relLoading = $state(false);
  async function showSheet(s: NonNullable<typeof sheet>) {
    sheet = s;
    rel = null;
    const rid = s.kind === "port" ? s.p.id : s.kind === "emu" && s.e.repo ? s.e.id : null;
    if (rid) {
      relLoading = true;
      api.releaseDetails(rid)
        .then((r) => sheet && (rel = r))
        .catch(() => {})
        .finally(() => (relLoading = false));
    }
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
          <span class="cp"><Pic kind="port" id={p.id} icon="cart" /></span>
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
          <span class="cp light"><Pic kind="emulator" id={e.id} icon="pad" fit="contain" /></span>
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
    <div class="grid">
      <button class="cart add" data-nav onclick={() => showSheet({ kind: "add" })}
        onmouseenter={(e) => (e.currentTarget as HTMLElement).focus({ preventScroll: true })}>
        <Glyph name="plus" size={48} />
        <span class="name">Add your own</span>
      </button>
      {#each cat?.browser ?? [] as b, i (b.id)}
        <button class="cart" data-nav style:--i={Math.min(i, 20)} onclick={() => showSheet({ kind: "web", b })}
          onmouseenter={(e) => (e.currentTarget as HTMLElement).focus({ preventScroll: true })}>
          <span class="cp"><Pic kind="browser" id={b.id} icon="globe" /></span>
          <span class="top label">{b.original ?? ""}</span>
          <span class="name">{b.title}</span>
          <span class="by">{b.project}</span>
          <span class="state web">
            <span class="kind label"><Glyph name={STATUS[b.status][0]} size={12} />{STATUS[b.status][1]}</span>
            <Glyph name={b.embed === "window" ? "play" : "globe"} size={24} />
          </span>
        </button>
      {/each}
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
      <div class="hero"><Pic kind="port" id={p.id} icon="cart" /></div>
      <span class="label">{p.kind} · {p.original.join(", ")}</span>
      <h2>{p.game}</h2>
      <div class="by">{p.name}</div>
      {#if p.blurb}<p class="blurb">{p.blurb}</p>{/if}
      {#if p.requires}<p class="needs"><Glyph name="alert" size={24} /><span>{p.requires}</span></p>{/if}
      <dl class="details">
        <dt><Glyph name="globe" size={24} /></dt><dd><button class="link" data-nav onclick={() => openUrl(`https://github.com/${p.repo}`)}>github.com/{p.repo}</button></dd>
        {#if p.licence}<dt><Glyph name="info" size={24} /></dt><dd>{p.licence}</dd>{/if}
        {#if rel}
          <dt><Glyph name="refresh" size={24} /></dt><dd>{rel.tag}{rel.published ? `, ${new Date(rel.published).toLocaleDateString()}` : ""}</dd>
          {#if rel.size}<dt><Glyph name="disk" size={24} /></dt><dd>{bytes(rel.size)} download{rel.asset ? ` (${rel.asset})` : ""}</dd>{/if}
        {:else if relLoading}
          <dt><Glyph name="refresh" size={24} /></dt><dd class="faint">Checking the latest release…</dd>
        {/if}
        {#if inst}<dt><Glyph name="check" size={24} /></dt><dd>{inst.version} installed{inst.source.verified ? ", hash checked" : ""}</dd>{/if}
      </dl>
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
    {:else if sheet.kind === "web"}
      {@const b = sheet.b}
      <div class="hero"><Pic kind="browser" id={b.id} icon="globe" /></div>
      <span class="label">{b.original ?? ""}</span>
      <h2>{b.title}</h2>
      <div class="by">{b.project}</div>
      <div class="facts">
        <span><Glyph name={STATUS[b.status][0]} size={24} />{STATUS[b.status][1]}</span>
        {#if b.multiplayer}<span><Glyph name="pad" size={24} />Online</span>{/if}
        <span><Glyph name={b.embed === "window" ? "play" : "globe"} size={24} />{b.embed === "window" ? "Plays in OnionRing" : "Opens in your browser"}</span>
      </div>
      {#if b.summary}<p class="needs"><Glyph name="info" size={24} /><span>{b.summary}</span></p>{/if}
      {#if b.needsFiles}<p class="needs"><Glyph name="folder" size={24} /><span>{b.needsFiles}</span></p>{/if}
      <dl class="details">
        <dt><Glyph name="globe" size={24} /></dt><dd><button class="link" data-nav onclick={() => openUrl(b.url)}>{new URL(b.url).host}</button></dd>
        {#if b.source}<dt><Glyph name="info" size={24} /></dt><dd><button class="link" data-nav onclick={() => openUrl(b.source!)}>{b.source.replace(/^https?:\/\//, "")}</button></dd>{/if}
        {#if b.controls}<dt><Glyph name="pad" size={24} /></dt><dd>{b.controls.replace("+", " and ").replace("both", "keyboard, mouse or controller")}</dd>{/if}
      </dl>
      <div class="btns">
        <button class="btn big primary" data-nav onclick={() => playWeb(b)}>
          <Glyph name={b.embed === "window" ? "play" : "globe"} size={24} />{b.embed === "window" ? "Play" : "Open"}
        </button>
        {#if b.source}
          <button class="btn big icon" data-nav onclick={() => openUrl(b.source!)} aria-label="Source code"><Glyph name="info" size={24} /></button>
        {/if}
        {#if b.status === "custom"}
          <button class="btn big" data-nav onclick={() => removeOwn(b)}><Glyph name="close" size={24} />Remove</button>
        {/if}
      </div>
    {:else if sheet.kind === "add"}
      <span class="label">Browser game</span>
      <h2>Add your own</h2>
      <label class="in"><span class="label">Name</span><input class="field" data-nav bind:value={newTitle} spellcheck="false" /></label>
      <label class="in"><span class="label">Link</span><input class="field" data-nav bind:value={newUrl} placeholder="https://" spellcheck="false"
        onkeydown={(e) => e.key === "Enter" && addOwn()} /></label>
      <p class="needs"><Glyph name="info" size={24} /><span>Saved only on this PC. It plays in its own window, kept apart from OnionRing.</span></p>
      <div class="btns">
        <button class="btn big primary" data-nav onclick={addOwn} disabled={!newTitle.trim() || !newUrl.trim()}><Glyph name="plus" size={24} />Add</button>
      </div>
    {:else}
      {@const e = sheet.e}
      {@const f = found(e)}
      <div class="hero light"><Pic kind="emulator" id={e.id} icon="pad" fit="contain" /></div>
      <span class="label">{e.platforms.map((p) => app.platforms.get(p)?.name ?? p).join(", ")}</span>
      <h2>{e.name}</h2>
      {#if f}<div class="by">{f.managed ? "Installed by OnionRing" : "Found on this PC"}{f.version ? `, version ${f.version}` : ""}</div>{/if}
      {#if e.notes}<p class="needs"><Glyph name="info" size={24} /><span>{e.notes}</span></p>{/if}
      {#if e.requires}<p class="needs"><Glyph name="alert" size={24} /><span>{e.requires}</span></p>{/if}
      <dl class="details">
        {#if e.repo}<dt><Glyph name="globe" size={24} /></dt><dd><button class="link" data-nav onclick={() => openUrl(`https://github.com/${e.repo}`)}>github.com/{e.repo}</button></dd>{/if}
        {#if e.homepage}<dt><Glyph name="home" size={24} /></dt><dd><button class="link" data-nav onclick={() => openUrl(e.homepage!)}>{new URL(e.homepage).host}</button></dd>{/if}
        {#if rel}
          <dt><Glyph name="refresh" size={24} /></dt><dd>{rel.tag}{rel.published ? `, ${new Date(rel.published).toLocaleDateString()}` : ""}</dd>
          {#if rel.size}<dt><Glyph name="disk" size={24} /></dt><dd>{bytes(rel.size)} download</dd>{/if}
        {:else if relLoading}
          <dt><Glyph name="refresh" size={24} /></dt><dd class="faint">Checking the latest release…</dd>
        {/if}
        {#if f}<dt><Glyph name="folder" size={24} /></dt><dd><button class="link" data-nav onclick={() => api.reveal(f.exe)}>Show on this PC</button></dd>{/if}
      </dl>
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
  .cart.add {
    justify-content: center;
    align-items: center;
    gap: 12px;
  }
  :global(:root[data-theme="pixel"]) .cart.add {
    border-style: dashed;
  }
  :global(:root[data-theme="paper"]) .cart.add {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px var(--line);
  }
  .in {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 10px;
  }
  .field {
    height: 52px;
    padding: 0 14px;
    font-size: 17px;
    background: var(--bg);
    border: var(--border) solid var(--line);
  }
  .cp {
    position: relative;
    display: block;
    width: calc(100% + 32px);
    height: 120px;
    margin: -14px -16px 6px;
    background: var(--panel);
  }
  .cp.light,
  .hero.light {
    background: #fff;
  }
  :global(:root[data-theme="pixel"]) .cp {
    border-bottom: var(--border) solid var(--line);
  }
  :global(:root[data-theme="paper"]) .cp {
    border-bottom: 1px solid var(--line);
  }
  .hero {
    position: relative;
    flex: none;
    height: 240px;
    margin: -36px -34px 14px;
    background: var(--panel);
  }
  :global(:root[data-theme="pixel"]) .hero {
    border-bottom: var(--border) solid var(--line);
  }
  .blurb {
    margin: 6px 0 0;
    font-size: 16px;
    color: var(--muted);
  }
  .details {
    display: grid;
    grid-template-columns: 24px 1fr;
    gap: 12px 14px;
    align-items: center;
    margin: 18px 0 0;
    font-family: var(--label);
    font-size: 15px;
  }
  .details dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .link {
    text-align: left;
    text-decoration: underline;
    text-underline-offset: 4px;
  }
  .faint {
    color: var(--faint);
  }
  .state.web {
    align-self: stretch;
    justify-content: space-between;
  }
  .kind {
    display: flex;
    align-items: center;
    gap: 6px;
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
