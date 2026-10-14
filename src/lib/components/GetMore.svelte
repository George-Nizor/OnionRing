<script lang="ts">
  import { tick } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type AvailableUpdate, type BrowserGame, type CatalogView, type Emulator, type EmulatorDef, type MashupDef, type PortDef, type ReleaseDetails } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { focusFirst } from "$lib/nav";
  import { bytes, norm } from "$lib/format";
  import Glyph from "./Glyph.svelte";
  import Pic from "./Pic.svelte";

  let { emulators }: { emulators: Emulator[] } = $props();

  let cat = $state<CatalogView | null>(null);
  let updates = $state<AvailableUpdate[] | null>(null);
  let consoleFilter = $state<string | null>(null);
  let sheet = $state<{ kind: "port"; p: PortDef } | { kind: "emu"; e: EmulatorDef } | { kind: "web"; b: BrowserGame } | { kind: "mash"; m: MashupDef } | { kind: "add" } | null>(null);
  let sheetEl = $state<HTMLElement>();
  const tab = $derived(app.view.tab ?? "ports");

  const TABS: [string, string, string][] = [
    ["ports", "cart", "Ports"],
    ["emulators", "pad", "Emulators"],
    ["browser", "globe", "Browser"],
    ["mashups", "mix", "Mashups"],
    ["updates", "refresh", "Updates"],
  ];

  /** What each kind of browser game is, as an icon and two words. */
  const STATUS: Record<BrowserGame["status"], [string, string]> = {
    official: ["check", "Free"],
    "open-source": ["globe", "Open source"],
    shareware: ["info", "Free demo"],
    "own-files": ["folder", "Your files"],
    "fan-original": ["star", "Fan made"],
    link: ["globe", "Web link"],
    custom: ["plus", "Yours"],
  };

  // ---- console names in the catalog -> platform ids (for console art) and short labels
  const clean = (raw: string) => raw.replace(/\s*\(.*?\)\s*/g, " ").trim();
  const PLATFORM: Record<string, string> = {
    playstation: "ps1", "playstation 1": "ps1", "playstation 2": "ps2", "playstation 3": "ps3", "playstation 4": "ps4", "playstation 5": "ps5",
    psp: "psp", "playstation portable": "psp",
    "nintendo 64": "n64", n64: "n64", snes: "snes", "super nintendo": "snes", "super nes": "snes", nes: "nes", famicom: "nes",
    "nintendo entertainment system": "nes", gamecube: "gc", wii: "wii", "wii u": "wiiu", "nintendo switch": "switch", switch: "switch",
    "nintendo ds": "nds", "nintendo 3ds": "3ds", "game boy": "gb", "game boy color": "gbc", "game boy advance": "gba",
    "sega genesis": "genesis", genesis: "genesis", "mega drive": "genesis", "sega mega drive": "genesis", "sega 32x": "genesis",
    dreamcast: "dreamcast", "sega dreamcast": "dreamcast", xbox: "xbox", "xbox 360": "xbox", pc: "port", "ms-dos": "port", dos: "port", windows: "port",
  };
  function platformOf(raw: string | null | undefined): string | null {
    if (!raw) return null;
    const c = clean(raw).toLowerCase();
    if (PLATFORM[c]) return PLATFORM[c];
    for (const p of app.platforms.values()) if (p.name.toLowerCase() === c || p.short.toLowerCase() === c) return p.id;
    return null;
  }
  const SHORT: Record<string, string> = {
    "Nintendo 64": "N64", PlayStation: "PS1", "PlayStation 2": "PS2", "PlayStation 3": "PS3", "PlayStation 4": "PS4", "Super Nintendo": "SNES",
    "Nintendo Entertainment System": "NES", "Nintendo DS": "DS", "Nintendo 3DS": "3DS", "Nintendo Switch": "Switch", "Game Boy Advance": "GBA",
    "Game Boy Color": "GBC", "Sega Genesis": "Genesis", "Sega Dreamcast": "Dreamcast", "Sega Game Gear": "Game Gear", "Sega Saturn": "Saturn", "Sega 32X": "32X",
  };
  const short = (raw: string) => {
    const o = clean(raw);
    return SHORT[o] ?? (o.length > 12 ? o.split(" ").map((w) => w[0]).join("").toUpperCase() : o);
  };
  /** The project's own name, without a trailing "(Game title)" that the card already shows. */
  const projectName = (p: PortDef) => p.name.replace(/\s*\(([^()]*)\)\s*$/, (m, g: string) => (norm(g) === norm(p.game) ? "" : m)).trim() || p.name;
  const day = (iso: string | null | undefined) =>
    iso ? new Date(iso).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" }) : null;

  // ---- data
  async function load() {
    cat = await api.catalog();
    // Arriving from search with something to show.
    const item = app.view.item;
    if (item && !sheet) {
      const p = cat.ports.find((x) => x.id === item);
      const e = cat.emulators.find((x) => x.id === item);
      const b = cat.browser.find((x) => x.id === item);
      const m = cat.mashups.find((x) => x.id === item);
      if (p) showSheet({ kind: "port", p });
      else if (e) showSheet({ kind: "emu", e });
      else if (b) showSheet({ kind: "web", b });
      else if (m) showSheet({ kind: "mash", m });
    }
  }
  const loadUpdates = () => api.availableUpdates().then((u) => (updates = u)).catch(() => (updates = updates ?? []));
  // Load once, then again whenever an install finishes.
  let busyCount = -1;
  $effect(() => {
    const n = Object.keys(app.installing).length;
    if (busyCount === -1 || n < busyCount) {
      load();
      loadUpdates();
    }
    busyCount = n;
  });

  const installed = $derived(new Map((cat?.installed ?? []).map((a) => [a.id, a])));
  const upById = $derived(new Map((updates ?? []).map((u) => [u.id, u])));
  const consoles = $derived.by(() => {
    const c = new Map<string, number>();
    for (const p of cat?.ports ?? []) for (const o of p.original) c.set(o, (c.get(o) ?? 0) + 1);
    return [...c.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8);
  });
  const sorted = $derived(
    [...(cat?.ports ?? [])].sort(
      (a, b) => Number(installed.has(b.id)) - Number(installed.has(a.id)) || Number(b.featured) - Number(a.featured) || a.game.localeCompare(b.game),
    ),
  );
  // Several ports of one game share its picture: only the first card shows it, the rest show the console.
  const picOwner = $derived.by(() => {
    const m = new Map<string, string>();
    for (const p of sorted) if (!m.has(p.game)) m.set(p.game, p.id);
    return m;
  });
  const ports = $derived(
    sorted
      .filter((p) => !consoleFilter || p.original.includes(consoleFilter)),
  );

  const found = (e: EmulatorDef) => emulators.find((x) => x.id === e.id || x.id.startsWith(`${e.id}-`));
  const baseId = (id: string) => id.replace(/-\d+$/, "");
  const defOf = (id: string) => cat?.emulators.find((e) => e.id === baseId(id));
  const emuStatus = (e: EmulatorDef, f: Emulator | undefined) =>
    f ? (f.managed ? "Installed by OnionRing" : "Found on this PC") : e.repo ? "Ready to install" : "From its website";

  // ---- mashups: the game each one is added to, and what it still needs
  const mState = $derived(new Map((cat?.mashupStates ?? []).map((s) => [s.id, s])));
  const mashups = $derived(
    [...(cat?.mashups ?? [])].sort(
      (a, b) => Number(installed.has(b.id)) - Number(installed.has(a.id)) || Number(b.featured) - Number(a.featured) || a.title.localeCompare(b.title),
    ),
  );
  /** Ready to play once installed: the game is there and nothing else is missing. */
  const mReady = (id: string) => {
    const s = mState.get(id);
    return !!s?.host.dir && s.needs.every((n) => n.ok);
  };
  async function chooseHost(hostId: string, name: string) {
    const dir = await open({ directory: true, multiple: false, title: `The folder ${name} is installed in` });
    if (typeof dir !== "string") return;
    try {
      await api.setHostDir(hostId, dir);
      await load();
      app.loadCatalog();
    } catch (e) {
      app.toast(String(e), "error");
    }
  }
  async function forgetHost(hostId: string) {
    await api.setHostDir(hostId, null).catch((e) => app.toast(String(e), "error"));
    await load();
      app.loadCatalog();
  }
  let removing = $state<string | null>(null);
  async function removeMashup(m: MashupDef) {
    removing = m.id;
    try {
      await api.removeMashup(m.id);
      app.toast(m.deploy === "game" ? `Took ${m.name} back out of ${mState.get(m.id)?.host.name ?? "the game"}` : `Removed ${m.name}`, "ok");
      await load();
      app.loadCatalog();
    } catch (e) {
      app.toast(String(e), "error", 12000);
    }
    removing = null;
  }

  /** Where an update's picture comes from. */
  function upArt(u: AvailableUpdate): { kind: "port" | "emulator" | "mashup"; id: string; system: string | null } {
    if (u.kind === "mashup") return { kind: "mashup", id: u.id, system: null };
    if (u.kind === "port") {
      const p = cat?.ports.find((x) => x.id === u.id);
      return { kind: "port", id: u.id, system: platformOf(p?.original[0]) };
    }
    const plat = emulators.find((x) => x.id === u.id)?.platforms[0] ?? defOf(u.id)?.platforms[0] ?? null;
    return { kind: "emulator", id: baseId(u.id), system: plat };
  }
  const managedUps = $derived((updates ?? []).filter((u) => u.managed));
  const otherUps = $derived((updates ?? []).filter((u) => !u.managed));

  // ---- actions
  let rel = $state<ReleaseDetails | null>(null);
  let relLoading = $state(false);
  async function showSheet(s: NonNullable<typeof sheet>) {
    sheet = s;
    rel = null;
    const rid = s.kind === "port" ? s.p.id : s.kind === "mash" ? s.m.id : s.kind === "emu" && s.e.repo ? s.e.id : null;
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
  let updatingAll = $state(false);
  async function updateAll() {
    const list = managedUps.filter((u) => !app.installing[u.id]);
    updatingAll = true;
    app.installing = { ...app.installing, ...Object.fromEntries(list.map((u) => [u.id, "Waiting"])) };
    for (const u of list) {
      app.installing = { ...app.installing, [u.id]: "Starting" };
      await api.installLatest(u.id).catch(() => {});
    }
    updatingAll = false;
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

  let newTitle = $state("");
  let newUrl = $state("");
  async function addOwn() {
    try {
      const g = await api.addBrowserGame(newTitle, newUrl);
      newTitle = newUrl = "";
      await load();
      app.toast(`Added ${g.title}`, "ok");
      showSheet({ kind: "web", b: g });
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
  const siteOf = (u: string) => host(u);
  const hover = (e: MouseEvent) => (e.currentTarget as HTMLElement).focus({ preventScroll: true });
  const host = (u: string) => {
    try {
      return new URL(u).host.replace(/^www\./, "");
    } catch {
      return u;
    }
  };
</script>

<div class="get">
  <nav class="tabs">
    {#each TABS as [id, icon, label] (id)}
      <button class="tab" class:on={tab === id} data-nav onclick={() => { app.view = { ...app.view, tab: id }; sheet = null; }}>
        <Glyph name={icon} size={24} /><span>{label}</span>
        {#if id === "updates" && updates?.length}<span class="badge" aria-label="{updates.length} updates">{updates.length}</span>{/if}
      </button>
    {/each}
  </nav>

  {#if tab === "ports"}
    <div class="bar">
      <div class="filters">
        <button class="f" class:on={!consoleFilter} data-nav onclick={() => (consoleFilter = null)}>All <em>{cat?.ports.length ?? ""}</em></button>
        {#each consoles as [c, n] (c)}
          <button class="f" class:on={consoleFilter === c} data-nav onclick={() => (consoleFilter = c)} title={c}>{short(c)} <em>{n}</em></button>
        {/each}
      </div>
    </div>
    <div class="grid">
      {#each ports as p, i (p.id)}
        {@const inst = installed.get(p.id)}
        {@const busy = app.installing[p.id]}
        <button class="card" data-nav style:--i={Math.min(i, 24)} onclick={() => showSheet({ kind: "port", p })} onmouseenter={hover}>
          <span class="cp">
            <Pic kind="port" id={p.id} icon="cart" system={platformOf(p.original[0])} plate={p.original[0] ? short(p.original[0]) : null} skip={picOwner.get(p.game) !== p.id} />
          </span>
          <span class="body">
            <span class="name">{p.game}</span>
            <span class="by">{projectName(p)}</span>
          </span>
          <span class="foot">
            <span class="chips">{#each p.original.slice(0, 2) as o (o)}<span class="chip">{short(o)}</span>{/each}</span>
            <span class="state" class:hot={inst && upById.has(p.id)}>
              {#if busy}
                <span class="prog" style:--p={pct(busy)}></span>
              {:else if inst && upById.has(p.id)}
                <span class="label">Update</span><Glyph name="refresh" size={24} />
              {:else if inst}
                <Glyph name="check" size={24} />
              {:else}
                <Glyph name="get" size={24} />
              {/if}
            </span>
          </span>
        </button>
      {:else}
        <p class="empty">No ports for that console yet.</p>
      {/each}
    </div>
  {:else if tab === "emulators"}
    <div class="grid">
      {#each cat?.emulators ?? [] as e, i (e.id)}
        {@const f = found(e)}
        {@const busy = app.installing[e.id]}
        {@const up = f ? upById.get(f.id) : undefined}
        <button class="card" data-nav style:--i={i} onclick={() => showSheet({ kind: "emu", e })} onmouseenter={hover}>
          <span class="cp"><Pic kind="emulator" id={e.id} icon="pad" system={e.platforms[0]} skip /></span>
          <span class="body">
            <span class="name">{e.name}</span>
            <span class="by">{emuStatus(e, f)}{f?.version ? ` · ${f.version}` : ""}</span>
          </span>
          <span class="foot">
            <span class="chips">{#each e.platforms.slice(0, 3) as p (p)}<span class="chip">{(app.platforms.get(p)?.short ?? p).replace("WII", "Wii")}</span>{/each}</span>
            <span class="state" class:hot={!!up}>
              {#if busy}<span class="prog" style:--p={pct(busy)}></span>
              {:else if up}<span class="label">Update</span><Glyph name="refresh" size={24} />
              {:else if f}<Glyph name={f.managed ? "check" : "folder"} size={24} />
              {:else}<Glyph name={e.repo ? "get" : "globe"} size={24} />{/if}
            </span>
          </span>
        </button>
      {/each}
    </div>
  {:else if tab === "browser"}
    <div class="grid">
      <button class="card add" data-nav onclick={() => showSheet({ kind: "add" })} onmouseenter={hover}>
        <span class="plus"><Glyph name="plus" size={48} /></span>
        <span class="name">Add your own</span>
        <span class="by">Any game that runs in a browser</span>
      </button>
      {#each cat?.browser ?? [] as b, i (b.id)}
        <button class="card" data-nav style:--i={Math.min(i, 24)} onclick={() => showSheet({ kind: "web", b })} onmouseenter={hover}>
          <span class="cp"><Pic kind="browser" id={b.id} icon="globe" system={platformOf(b.original)} /></span>
          <span class="body">
            <span class="name">{b.title}</span>
            <span class="by">{b.project}</span>
          </span>
          <span class="foot">
            <span class="kind label"><Glyph name={STATUS[b.status][0]} size={12} />{STATUS[b.status][1]}</span>
            <span class="state"><Glyph name={b.embed === "window" ? "play" : "globe"} size={24} /></span>
          </span>
        </button>
      {/each}
    </div>
  {:else if tab === "mashups"}
    <div class="grid">
      {#each mashups as m, i (m.id)}
        {@const inst = installed.get(m.id)}
        {@const busy = app.installing[m.id]}
        {@const s = mState.get(m.id)}
        <button class="card" data-nav style:--i={Math.min(i, 24)} onclick={() => showSheet({ kind: "mash", m })} onmouseenter={hover}>
          <span class="cp"><Pic kind="mashup" id={m.id} icon="mix" system={null} /></span>
          <span class="body">
            <span class="name">{m.title}</span>
            <span class="by">{m.name}{m.author ? ` · ${m.author}` : ""}</span>
          </span>
          <span class="foot">
            <span class="kind label host" class:faint={!s?.host.dir}><Glyph name={s?.host.dir ? "check" : "alert"} size={12} /><span>{s?.host.name ?? m.host}</span></span>
            <span class="state" class:hot={inst && upById.has(m.id)}>
              {#if busy}
                <span class="prog" style:--p={pct(busy)}></span>
              {:else if inst && upById.has(m.id)}
                <span class="label">Update</span><Glyph name="refresh" size={24} />
              {:else if inst}
                <Glyph name={mReady(m.id) ? "check" : "alert"} size={24} />
              {:else}
                <Glyph name="get" size={24} />
              {/if}
            </span>
          </span>
        </button>
      {:else}
        <p class="empty">No mashups yet.</p>
      {/each}
    </div>
  {:else}
    <!-- Updates -->
    {#if updates === null}
      <div class="done checking">
        <span class="spin"><Glyph name="refresh" size={48} /></span>
        <h2>Checking for new versions</h2>
      </div>
    {:else if !updates.length}
      <div class="done">
        {#if app.theme === "pixel"}
          <span class="tick px"><Glyph name="check" size={96} /></span>
        {:else}
          <svg class="tick ink" viewBox="0 0 120 120" aria-hidden="true">
            <circle class="ring" cx="60" cy="60" r="52" pathLength="1" />
            <path class="mark" d="M36 62 L53 78 L86 42" pathLength="1" />
          </svg>
        {/if}
        <h2>Everything is up to date</h2>
        <p>New versions of your emulators, ports and mashups show up here.</p>
      </div>
    {:else}
      <div class="uphead">
        <h2>{updates.length} new {updates.length === 1 ? "version" : "versions"}</h2>
        {#if managedUps.length}
          <button class="btn primary" data-nav onclick={updateAll} disabled={updatingAll}>
            <Glyph name="refresh" size={24} />{managedUps.length > 1 ? `Update all ${managedUps.length}` : "Update"}
          </button>
        {/if}
      </div>
      <div class="ups">
        {#each [...managedUps, ...otherUps] as u, i (u.id)}
          {@const a = upArt(u)}
          {@const busy = app.installing[u.id]}
          {@const def = u.managed ? null : defOf(u.id)}
          <article class="up" style:--i={i}>
            <span class="thumb">
              <Pic kind={a.kind} id={a.id} icon={a.kind === "port" ? "cart" : a.kind === "mashup" ? "mix" : "pad"} system={a.system} skip={a.kind === "emulator"} />
            </span>
            <div class="who">
              <span class="label">{u.kind === "port" ? "Port" : u.kind === "mashup" ? "Mashup" : "Emulator"}{u.managed ? "" : " · on this PC"}</span>
              <h3>{u.name}</h3>
            </div>
            <div class="vers">
              <span class="old">{u.installed}</span>
              <span class="arrow"><Glyph name="back" size={24} /></span>
              <span class="new">{u.latest}</span>
              {#if day(u.published)}<span class="when">{day(u.published)}</span>{/if}
            </div>
            <div class="acts">
              {#if busy}
                <span class="prog" style:--p={pct(busy)}></span>
              {:else if u.managed}
                <button class="btn primary" data-nav onclick={() => install(u.id)}><Glyph name="refresh" size={24} />Update</button>
              {/if}
            </div>
            {#if !u.managed}
              <div class="why">
                <Glyph name="info" size={24} />
                <p>You set this copy up yourself, so OnionRing leaves it alone. {def?.repo ? "Install a fresh copy that OnionRing keeps up to date, or get the new version from its download page." : "Get the new version from its download page."}</p>
                <div class="whybtns">
                  {#if def?.repo && !busy}
                    <button class="btn" data-nav onclick={() => install(def.id)}><Glyph name="get" size={24} />Install fresh copy</button>
                  {/if}
                  {#if u.url}
                    <button class="btn" data-nav onclick={() => openUrl(u.url!)}><Glyph name="globe" size={24} />Download page</button>
                  {/if}
                </div>
              </div>
            {/if}
          </article>
        {/each}
      </div>
    {/if}
  {/if}
</div>

{#if sheet}
  <div class="scrim" role="presentation" onclick={() => (sheet = null)}></div>
  <aside class="sheet" data-layer bind:this={sheetEl}>
    {#if sheet.kind === "port"}
      {@const p = sheet.p}
      {@const inst = installed.get(p.id)}
      {@const up = upById.get(p.id)}
      <div class="hero"><Pic kind="port" id={p.id} icon="cart" system={platformOf(p.original[0])} /></div>
      <div class="sbody">
        <div class="kicker"><span class="chip">{p.kind}</span>{#each p.original as o (o)}<span class="chip">{clean(o)}</span>{/each}</div>
        <h2>{p.game}</h2>
        <div class="sub">{projectName(p)}</div>
        {#if p.blurb}<p class="blurb">{p.blurb}</p>{/if}
        {#if p.requires}<div class="callout"><Glyph name="alert" size={24} /><div><span class="label">You will need</span><p>{p.requires}</p></div></div>{/if}
        <dl class="facts">
          <div><dt><Glyph name="globe" size={24} />Source</dt><dd><button class="link" data-nav onclick={() => openUrl(`https://github.com/${p.repo}`)}>github.com/{p.repo}</button></dd></div>
          {#if p.licence}<div><dt><Glyph name="info" size={24} />Licence</dt><dd>{p.licence}</dd></div>{/if}
          {#if rel}
            <div><dt><Glyph name="refresh" size={24} />Latest</dt><dd>{rel.tag}{rel.published ? ` · ${day(rel.published)}` : ""}</dd></div>
            {#if rel.size}<div><dt><Glyph name="disk" size={24} />Size</dt><dd>{bytes(rel.size)} download</dd></div>{/if}
          {:else if relLoading}
            <div><dt><Glyph name="refresh" size={24} />Latest</dt><dd class="faint">Checking…</dd></div>
          {/if}
          {#if inst}<div><dt><Glyph name="check" size={24} />Installed</dt><dd>{inst.version}{inst.source.verified ? " · download checked" : ""}</dd></div>{/if}
        </dl>
      </div>
      <div class="actions">
        {#if app.installing[p.id]}
          <div class="progress"><div class="prog wide" style:--p={pct(app.installing[p.id])}></div><span class="label">{app.installing[p.id]}</span></div>
        {:else}
          <button class="btn big primary" data-nav onclick={() => install(p.id)}>
            <Glyph name={inst ? "refresh" : "get"} size={24} />{inst ? (up ? `Update to ${up.latest}` : "Reinstall") : "Get"}
          </button>
          <button class="btn big" data-nav onclick={() => fromFile(p.id)}><Glyph name="zip" size={24} />From a zip</button>
        {/if}
      </div>
    {:else if sheet.kind === "mash"}
      {@const m = sheet.m}
      {@const inst = installed.get(m.id)}
      {@const up = upById.get(m.id)}
      {@const s = mState.get(m.id)}
      {@const host = s?.host}
      <div class="hero"><Pic kind="mashup" id={m.id} icon="mix" system={null} /></div>
      <div class="sbody">
        <div class="kicker">
          <span class="chip"><Glyph name="mix" size={12} />Mashup</span>
          <span class="chip">{host?.name ?? m.host}</span>
          <span class="chip">{m.brings}</span>
        </div>
        <h2>{m.title}</h2>
        <div class="sub">{m.name}{m.author ? ` by ${m.author}` : ""}</div>
        {#if m.blurb}<p class="blurb">{m.blurb}</p>{/if}

        {#if host && !host.dir}
          <div class="callout">
            <Glyph name="alert" size={24} />
            <div>
              <span class="label">{host.name} isn't installed</span>
              <p>{m.name} is added to your own copy of {host.name}.{host.steam ? " Install it through Steam, or show OnionRing where it is." : " Show OnionRing where it is."}</p>
              <div class="whybtns">
                {#if host.steam}<button class="btn" data-nav onclick={() => openUrl(`https://store.steampowered.com/app/${host.steam}`)}><Glyph name="globe" size={24} />Steam page</button>{/if}
                <button class="btn" data-nav onclick={() => chooseHost(host.id, host.name)}><Glyph name="folder" size={24} />Choose its folder</button>
              </div>
            </div>
          </div>
        {/if}
        {#if s?.needs.length}
          <ul class="needs">
            {#each s.needs as n (n.what)}
              <li class:ok={n.ok}>
                <Glyph name={n.ok ? "check" : "alert"} size={24} />
                <div><b>{n.what}</b>{#if !n.ok}<span>{n.help}</span>{/if}</div>
                {#if !n.ok && n.url}<button class="btn" data-nav onclick={() => openUrl(n.url!)}><Glyph name="globe" size={24} />Get it</button>{/if}
              </li>
            {/each}
          </ul>
        {/if}
        {#if m.requires}<div class="callout"><Glyph name="alert" size={24} /><div><span class="label">You will also need</span><p>{m.requires}</p></div></div>{/if}

        <dl class="facts">
          {#if host?.dir}
            <div><dt><Glyph name="pad" size={24} />Your game</dt><dd>
              <button class="link" data-nav onclick={() => api.openFolder(host.dir!).catch((e) => app.toast(String(e), "error"))}>{host.chosen ? "In the folder you chose" : "Found in Steam"}</button>
              {#if host.chosen}<button class="link" data-nav onclick={() => forgetHost(host.id)}>Use Steam's</button>{:else}<button class="link" data-nav onclick={() => chooseHost(host.id, host.name)}>Choose another</button>{/if}
            </dd></div>
          {/if}
          <div><dt><Glyph name="folder" size={24} />Goes</dt><dd>{m.deploy === "game" ? `Into your ${host?.name ?? "game"} folder. Remove takes every file back out.` : "In its own folder. Your game files aren't changed."}</dd></div>
          <div><dt><Glyph name="globe" size={24} />Source</dt><dd><button class="link" data-nav onclick={() => openUrl(`https://github.com/${m.repo}`)}>github.com/{m.repo}</button></dd></div>
          {#if m.page}<div><dt><Glyph name="info" size={24} />Mod page</dt><dd><button class="link" data-nav onclick={() => openUrl(m.page!)}>{siteOf(m.page)}</button></dd></div>{/if}
          {#if m.licence}<div><dt><Glyph name="info" size={24} />Licence</dt><dd>{m.licence}</dd></div>{/if}
          {#if rel}
            <div><dt><Glyph name="refresh" size={24} />Latest</dt><dd>{rel.tag}{rel.published ? ` · ${day(rel.published)}` : ""}</dd></div>
            {#if rel.size}<div><dt><Glyph name="disk" size={24} />Size</dt><dd>{bytes(rel.size)} download</dd></div>{/if}
          {:else if relLoading}
            <div><dt><Glyph name="refresh" size={24} />Latest</dt><dd class="faint">Checking…</dd></div>
          {/if}
          {#if inst}<div><dt><Glyph name="check" size={24} />Installed</dt><dd>{inst.version}{inst.source.verified ? " · download checked" : ""}</dd></div>{/if}
        </dl>
      </div>
      <div class="actions">
        {#if app.installing[m.id]}
          <div class="progress"><div class="prog wide" style:--p={pct(app.installing[m.id])}></div><span class="label">{app.installing[m.id]}</span></div>
        {:else}
          <button class="btn big primary" data-nav onclick={() => install(m.id)} disabled={m.deploy === "game" && !host?.dir}>
            <Glyph name={inst ? "refresh" : "get"} size={24} />{inst ? (up ? `Update to ${up.latest}` : "Reinstall") : "Get"}
          </button>
          {#if inst}
            <button class="btn big" data-nav onclick={() => removeMashup(m)} disabled={removing === m.id}><Glyph name="close" size={24} />{removing === m.id ? "Removing…" : "Remove"}</button>
          {:else}
            <button class="btn big" data-nav onclick={() => fromFile(m.id)} disabled={m.deploy === "game" && !host?.dir}><Glyph name="zip" size={24} />From a zip</button>
          {/if}
        {/if}
      </div>
    {:else if sheet.kind === "web"}
      {@const b = sheet.b}
      <div class="hero"><Pic kind="browser" id={b.id} icon="globe" system={platformOf(b.original)} /></div>
      <div class="sbody">
        <div class="kicker">
          <span class="chip"><Glyph name={STATUS[b.status][0]} size={12} />{STATUS[b.status][1]}</span>
          {#if b.original}<span class="chip">{b.original}</span>{/if}
          {#if b.multiplayer}<span class="chip"><Glyph name="pad" size={12} />Online</span>{/if}
        </div>
        <h2>{b.title}</h2>
        <div class="sub">{b.project}</div>
        {#if b.summary}<p class="blurb">{b.summary}</p>{/if}
        {#if b.needsFiles}<div class="callout"><Glyph name="folder" size={24} /><div><span class="label">You will need</span><p>{b.needsFiles}</p></div></div>{/if}
        <dl class="facts">
          <div><dt><Glyph name={b.embed === "window" ? "play" : "globe"} size={24} />Plays</dt><dd>{b.embed === "window" ? "In its own OnionRing window" : "In your web browser"}</dd></div>
          <div><dt><Glyph name="globe" size={24} />Website</dt><dd><button class="link" data-nav onclick={() => openUrl(b.url)}>{host(b.url)}</button></dd></div>
          {#if b.source}<div><dt><Glyph name="info" size={24} />Source</dt><dd><button class="link" data-nav onclick={() => openUrl(b.source!)}>{b.source.replace(/^https?:\/\/(www\.)?/, "")}</button></dd></div>{/if}
          {#if b.controls}<div><dt><Glyph name="pad" size={24} />Controls</dt><dd>{b.controls.replace("+", " and ").replace("both", "keyboard, mouse or controller")}</dd></div>{/if}
        </dl>
      </div>
      <div class="actions">
        <button class="btn big primary" data-nav onclick={() => playWeb(b)}>
          <Glyph name={b.embed === "window" ? "play" : "globe"} size={24} />{b.embed === "window" ? "Play" : "Open"}
        </button>
        {#if b.status === "custom"}
          <button class="btn big" data-nav onclick={() => removeOwn(b)}><Glyph name="close" size={24} />Remove</button>
        {/if}
      </div>
    {:else if sheet.kind === "add"}
      <div class="sbody pad">
        <div class="kicker"><span class="chip"><Glyph name="globe" size={12} />Browser game</span></div>
        <h2>Add your own</h2>
        <label class="in"><span class="label">Name</span><input class="field" data-nav bind:value={newTitle} spellcheck="false" /></label>
        <label class="in"><span class="label">Link</span><input class="field" data-nav bind:value={newUrl} placeholder="https://" spellcheck="false"
          onkeydown={(e) => e.key === "Enter" && addOwn()} /></label>
        <div class="callout quiet"><Glyph name="info" size={24} /><div><p>Saved only on this PC. It plays in its own window, kept apart from OnionRing.</p></div></div>
      </div>
      <div class="actions">
        <button class="btn big primary" data-nav onclick={addOwn} disabled={!newTitle.trim() || !newUrl.trim()}><Glyph name="plus" size={24} />Add</button>
      </div>
    {:else}
      {@const e = sheet.e}
      {@const f = found(e)}
      {@const up = f ? upById.get(f.id) : undefined}
      <div class="hero"><Pic kind="emulator" id={e.id} icon="pad" system={e.platforms[0]} skip /></div>
      <div class="sbody">
        <div class="kicker">{#each e.platforms as p (p)}<span class="chip">{app.platforms.get(p)?.name ?? p}</span>{/each}</div>
        <h2>{e.name}</h2>
        <div class="sub">{emuStatus(e, f)}{f?.version ? `, version ${f.version}` : ""}</div>
        {#if up}
          <div class="callout hot"><Glyph name="refresh" size={24} /><div><span class="label">Version {up.latest} is out</span>
            <p>{up.managed ? "OnionRing can update it for you." : "You set this copy up yourself, so OnionRing leaves it alone. Install a fresh copy, or get the new version from its download page."}</p></div></div>
        {/if}
        {#if e.notes}<div class="callout quiet"><Glyph name="info" size={24} /><div><p>{e.notes}</p></div></div>{/if}
        {#if e.requires}<div class="callout"><Glyph name="alert" size={24} /><div><span class="label">You will need</span><p>{e.requires}</p></div></div>{/if}
        <dl class="facts">
          {#if e.repo}<div><dt><Glyph name="globe" size={24} />Source</dt><dd><button class="link" data-nav onclick={() => openUrl(`https://github.com/${e.repo}`)}>github.com/{e.repo}</button></dd></div>{/if}
          {#if e.homepage}<div><dt><Glyph name="home" size={24} />Website</dt><dd><button class="link" data-nav onclick={() => openUrl(e.homepage!)}>{host(e.homepage)}</button></dd></div>{/if}
          {#if rel}
            <div><dt><Glyph name="refresh" size={24} />Latest</dt><dd>{rel.tag}{rel.published ? ` · ${day(rel.published)}` : ""}</dd></div>
            {#if rel.size}<div><dt><Glyph name="disk" size={24} />Size</dt><dd>{bytes(rel.size)} download</dd></div>{/if}
          {:else if relLoading}
            <div><dt><Glyph name="refresh" size={24} />Latest</dt><dd class="faint">Checking…</dd></div>
          {/if}
          {#if f}<div><dt><Glyph name="folder" size={24} />This PC</dt><dd><button class="link" data-nav onclick={() => api.reveal(f.exe)}>Show in its folder</button></dd></div>{/if}
        </dl>
      </div>
      <div class="actions">
        {#if app.installing[e.id]}
          <div class="progress"><div class="prog wide" style:--p={pct(app.installing[e.id])}></div><span class="label">{app.installing[e.id]}</span></div>
        {:else if e.repo}
          <button class="btn big primary" data-nav onclick={() => install(e.id)}><Glyph name={f ? "refresh" : "get"} size={24} />{f?.managed ? "Update" : f ? "Install fresh copy" : "Install"}</button>
          {#if up?.url && !up.managed}<button class="btn big" data-nav onclick={() => openUrl(up.url!)}><Glyph name="globe" size={24} />Download page</button>{/if}
        {:else if e.homepage}
          <button class="btn big" data-nav onclick={() => openUrl(e.homepage!)}><Glyph name="globe" size={24} />Website</button>
        {/if}
      </div>
    {/if}
    <button class="btn icon close" data-nav onclick={() => (sheet = null)} aria-label="Close"><Glyph name="close" size={24} /></button>
  </aside>
{/if}

<style>
  .get {
    height: 100%;
    overflow-y: auto;
    padding: 24px 56px 64px;
  }

  /* ---- tabs ---------------------------------------------------------------------------- */
  .tabs {
    display: flex;
    gap: 8px;
    margin-bottom: 24px;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 52px;
    padding: 0 20px;
    font-family: var(--title);
    font-size: 21px;
    font-weight: 800;
  }
  .badge {
    display: grid;
    place-items: center;
    min-width: 24px;
    height: 24px;
    padding: 0 6px;
    font-family: var(--label);
    font-size: 13px;
    font-weight: 700;
    line-height: 1;
  }
  :global(:root[data-theme="pixel"]) .tabs {
    border-bottom: var(--border) solid var(--line);
  }
  :global(:root[data-theme="pixel"]) .tab:hover {
    background: var(--hi-bg);
    color: var(--hi-fg);
  }
  :global(:root[data-theme="pixel"]) .badge {
    background: var(--fg);
    color: var(--bg);
    animation: pop 0.3s steps(3) both;
  }
  :global(:root[data-theme="pixel"]) .tab:is(.on, :hover, :focus-visible) .badge {
    background: var(--hi-fg);
    color: var(--hi-bg);
  }
  :global(:root[data-theme="paper"]) .tabs {
    box-shadow: inset 0 -1px 0 var(--line);
  }
  :global(:root[data-theme="paper"]) .tab {
    color: var(--muted);
    transition: color 0.2s;
  }
  :global(:root[data-theme="paper"]) .tab:hover,
  :global(:root[data-theme="paper"]) .tab.on {
    color: var(--fg);
  }
  :global(:root[data-theme="paper"]) .tab.on::before {
    content: "";
    position: absolute;
    left: 16px;
    right: 16px;
    bottom: 0;
    height: 3px;
    background: var(--accent);
    animation: ink 0.3s var(--ease) both;
    transform-origin: left;
  }
  :global(:root[data-theme="paper"]) .tab.on :global(svg) {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .badge {
    border-radius: 12px;
    background: var(--accent);
    color: var(--panel);
    animation: pop 0.4s var(--ease) both;
  }

  /* ---- filter chips ---------------------------------------------------------------------- */
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 24px;
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .f {
    height: 36px;
    padding: 0 14px;
    font-family: var(--label);
    font-weight: 700;
    font-size: 14px;
  }
  .f em {
    font-style: normal;
    opacity: 0.55;
    margin-left: 4px;
  }
  :global(:root[data-theme="pixel"]) .f {
    border: 2px solid var(--line);
    text-transform: uppercase;
    font-size: 12px;
  }
  :global(:root[data-theme="pixel"]) .f:hover {
    background: var(--hi-bg);
    color: var(--hi-fg);
  }
  :global(:root[data-theme="paper"]) .f {
    border-radius: 18px;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--line) 30%, transparent);
    transition: box-shadow 0.2s, background 0.2s;
  }
  :global(:root[data-theme="paper"]) .f:hover {
    box-shadow: inset 0 0 0 1.5px var(--line);
  }
  :global(:root[data-theme="paper"]) .f.on {
    background: var(--fg);
    color: var(--panel);
    box-shadow: none;
  }
  :global(:root[data-theme="paper"]) .f:focus-visible::after {
    left: 14px;
    right: 14px;
  }

  /* ---- cards ----------------------------------------------------------------------------- */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(224px, 1fr));
    gap: 24px 20px;
  }
  .card {
    display: flex;
    flex-direction: column;
    text-align: left;
    min-width: 0;
    animation: rise 0.4s var(--ease) both;
    animation-delay: calc(var(--i) * 16ms);
  }
  .cp {
    position: relative;
    display: block;
    aspect-ratio: 16 / 9;
    flex: none;
    background: var(--panel);
    --sa-w: 144px;
    overflow: hidden;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 14px 16px 0;
    min-height: 82px;
  }
  .name {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    font-family: var(--title);
    font-weight: 800;
    font-size: 19px;
    line-height: 1.15;
    overflow-wrap: anywhere;
  }
  .by {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-family: var(--label);
    font-size: 13px;
    color: var(--muted);
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-top: auto;
    padding: 14px 16px 14px;
    min-height: 54px;
  }
  .chips,
  .kicker {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    min-width: 0;
  }
  .chips {
    flex-wrap: nowrap;
    overflow: hidden;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 22px;
    padding: 0 7px;
    font-family: var(--label);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    white-space: nowrap;
  }
  .state {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
    min-height: 24px;
  }
  .kind {
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }
  /* A mashup's game: long names give way to the install state. */
  .kind.host {
    min-width: 0;
  }
  .kind.host > span {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  :global(:root[data-theme="pixel"]) .card {
    border: var(--border) solid var(--line);
    animation-timing-function: steps(3);
  }
  :global(:root[data-theme="pixel"]) .cp {
    border-bottom: var(--border) solid var(--line);
  }
  :global(:root[data-theme="pixel"]) .card :global(.art) {
    filter: var(--img-filter);
  }
  :global(:root[data-theme="pixel"]) .chip {
    box-shadow: inset 0 0 0 2px currentColor;
  }
  :global(:root[data-theme="pixel"]) .card:hover {
    background: var(--hi-bg);
    color: var(--hi-fg);
  }
  :global(:root[data-theme="pixel"]) .card:is(:hover, :focus-visible) .by {
    color: inherit;
  }
  /* Marching ants around the tile under the pointer or focus. */
  :global(:root[data-theme="pixel"]) .card:is(:hover, :focus-visible)::after {
    content: "";
    position: absolute;
    inset: -9px;
    pointer-events: none;
    background:
      linear-gradient(90deg, var(--fg) 50%, transparent 0) 0 0 / 8px 3px repeat-x,
      linear-gradient(90deg, var(--fg) 50%, transparent 0) 0 100% / 8px 3px repeat-x,
      linear-gradient(0deg, var(--fg) 50%, transparent 0) 0 0 / 3px 8px repeat-y,
      linear-gradient(0deg, var(--fg) 50%, transparent 0) 100% 0 / 3px 8px repeat-y;
    animation: ants 0.6s steps(2) infinite;
  }
  :global(:root[data-theme="pixel"]) .state.hot {
    background: var(--fg);
    color: var(--bg);
    padding: 0 0 0 8px;
  }
  :global(:root[data-theme="pixel"]) .card:is(:hover, :focus-visible) .state.hot {
    background: var(--hi-fg);
    color: var(--hi-bg);
  }

  :global(:root[data-theme="paper"]) .card {
    background: var(--panel);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--line) 22%, transparent);
    transition:
      transform 0.3s var(--ease),
      box-shadow 0.3s var(--ease);
  }
  :global(:root[data-theme="paper"]) .cp {
    box-shadow: inset 0 -1px 0 color-mix(in srgb, var(--line) 22%, transparent);
  }
  :global(:root[data-theme="paper"]) .card:is(:hover, :focus-visible) {
    transform: translateY(-4px);
    box-shadow:
      0 0 0 1.5px var(--line),
      6px 6px 0 var(--line);
  }
  :global(:root[data-theme="paper"]) .card:is(:hover, :focus-visible) :global(.art) {
    filter: var(--img-filter-focus);
  }
  :global(:root[data-theme="paper"]) .card:focus-visible::after {
    display: none;
  }
  /* The red bookmark ribbon drops onto the tile under the pointer or focus. */
  :global(:root[data-theme="paper"]) .card:is(:hover, :focus-visible)::before {
    content: "";
    position: absolute;
    top: -4px;
    right: 16px;
    z-index: 2;
    width: 16px;
    height: 30px;
    background: var(--accent);
    clip-path: polygon(0 0, 100% 0, 100% 100%, 50% 76%, 0 100%);
    animation: ribbon 0.35s var(--ease) both;
  }
  @keyframes ribbon {
    from {
      transform: translateY(-100%);
      opacity: 0;
    }
  }
  :global(:root[data-theme="paper"]) .chip {
    color: var(--accent-2);
    padding: 0;
    letter-spacing: 0.05em;
  }
  :global(:root[data-theme="paper"]) .chips {
    gap: 10px;
  }
  :global(:root[data-theme="paper"]) .state.hot {
    color: var(--accent);
  }

  .card.add {
    align-items: center;
    justify-content: center;
    gap: 8px;
    text-align: center;
    padding: 24px;
  }
  .card.add .name {
    min-height: 0;
  }
  .card.add .plus {
    display: grid;
    place-items: center;
    width: 88px;
    height: 88px;
    margin-bottom: 8px;
  }
  :global(:root[data-theme="pixel"]) .card.add {
    border-style: dashed;
  }
  :global(:root[data-theme="pixel"]) .card.add .plus {
    border: var(--border) solid currentColor;
  }
  :global(:root[data-theme="paper"]) .card.add {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px color-mix(in srgb, var(--line) 40%, transparent);
  }
  :global(:root[data-theme="paper"]) .card.add .plus {
    border-radius: 50%;
    box-shadow: inset 0 0 0 1.5px var(--line);
    color: var(--accent);
  }
  .empty {
    grid-column: 1 / -1;
    margin: 48px 0;
    text-align: center;
    color: var(--muted);
    font-size: 18px;
  }

  /* ---- progress: segmented in pixel, a ruled line in paper -------------------------------- */
  .prog {
    width: 96px;
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

  /* ---- updates --------------------------------------------------------------------------- */
  .uphead {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 16px;
  }
  .uphead h2,
  .done h2 {
    margin: 0;
    font-family: var(--title);
    font-weight: 800;
    font-size: 28px;
    line-height: 1.1;
  }
  .ups {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .up {
    display: grid;
    grid-template-columns: 120px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 0 24px;
    padding: 16px 20px 16px 16px;
    animation: rise 0.4s var(--ease) both;
    animation-delay: calc(var(--i) * 40ms);
  }
  .thumb {
    position: relative;
    width: 120px;
    aspect-ratio: 3 / 2;
    overflow: hidden;
    background: var(--panel);
    --sa-w: 96px;
  }
  .who {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .who .label {
    color: var(--faint);
  }
  .who h3 {
    margin: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-family: var(--title);
    font-weight: 800;
    font-size: 24px;
    line-height: 1.15;
  }
  .vers {
    display: grid;
    grid-template-columns: auto auto auto;
    align-items: center;
    gap: 4px 10px;
    font-family: var(--label);
    font-size: 16px;
    font-weight: 700;
  }
  .vers .old {
    color: var(--faint);
    text-decoration: line-through;
    text-decoration-thickness: 2px;
  }
  .vers .arrow {
    display: flex;
    transform: scaleX(-1);
  }
  .vers .when {
    grid-column: 1 / -1;
    justify-self: end;
    font-size: 13px;
    font-weight: 400;
    color: var(--faint);
  }
  .acts {
    display: flex;
    justify-content: flex-end;
  }
  .acts:empty {
    display: none;
  }
  .why {
    grid-column: 2 / -1;
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    margin-top: 14px;
    padding-top: 14px;
  }
  .why p {
    margin: 0;
    font-size: 15px;
    line-height: 1.4;
    color: var(--muted);
    max-width: 62ch;
  }
  .whybtns {
    display: flex;
    gap: 8px;
  }
  :global(:root[data-theme="pixel"]) .up {
    border: var(--border) solid var(--line);
    animation-timing-function: steps(3);
  }
  :global(:root[data-theme="pixel"]) .thumb {
    box-shadow: 0 0 0 2px var(--line);
  }
  :global(:root[data-theme="pixel"]) .why {
    border-top: 2px dashed var(--faint);
  }
  :global(:root[data-theme="paper"]) .up {
    background: var(--panel);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--line) 22%, transparent);
  }
  :global(:root[data-theme="paper"]) .vers .new {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .why {
    border-top: 1px solid color-mix(in srgb, var(--line) 18%, transparent);
  }
  :global(:root[data-theme="paper"]) .why > :global(svg) {
    color: var(--accent-2);
  }

  .done {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    padding: 9vh 0 48px;
    text-align: center;
  }
  .done p {
    margin: 0;
    color: var(--muted);
    font-size: 16px;
  }
  .tick {
    display: grid;
    place-items: center;
    width: 160px;
    height: 160px;
    margin-bottom: 8px;
  }
  .tick.px {
    border: var(--border) solid var(--line);
    animation: wipe 0.6s steps(6) both;
  }
  @keyframes wipe {
    from {
      clip-path: inset(0 100% 0 0);
    }
    to {
      clip-path: inset(0 0 0 0);
    }
  }
  .tick.ink {
    fill: none;
    stroke: var(--fg);
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .tick.ink .ring {
    stroke-width: 1.5;
    stroke-dasharray: 1;
    animation: draw 0.7s var(--ease) both;
  }
  .tick.ink .mark {
    stroke: var(--accent);
    stroke-width: 6;
    stroke-dasharray: 1;
    animation: draw 0.5s 0.45s var(--ease) both;
  }
  @keyframes draw {
    from {
      stroke-dashoffset: 1;
    }
    to {
      stroke-dashoffset: 0;
    }
  }
  .spin {
    display: grid;
    animation: blink 1s steps(2) infinite;
  }

  /* ---- details sheet --------------------------------------------------------------------- */
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 44;
  }
  :global(:root[data-theme="pixel"]) .scrim {
    background: repeating-linear-gradient(0deg, rgba(0, 0, 0, 0.85) 0 2px, rgba(0, 0, 0, 0.6) 2px 4px);
  }
  :global(:root[data-theme="paper"]) .scrim {
    background: rgba(238, 235, 227, 0.72);
  }
  .sheet {
    position: fixed;
    top: var(--bar);
    right: 0;
    bottom: var(--hints);
    z-index: 45;
    width: min(560px, 92vw);
    background: var(--bg);
    display: flex;
    flex-direction: column;
  }
  :global(:root[data-theme="pixel"]) .sheet {
    border-left: var(--border) solid var(--line);
    animation: sheetin 0.2s steps(4) both;
  }
  :global(:root[data-theme="paper"]) .sheet {
    background: var(--panel);
    box-shadow: -1px 0 0 var(--line), -12px 0 0 color-mix(in srgb, var(--line) 6%, transparent);
    animation: sheetin 0.35s var(--ease) both;
  }
  @keyframes sheetin {
    from {
      transform: translateX(100%);
    }
  }
  .close {
    position: absolute;
    top: 16px;
    right: 16px;
    z-index: 2;
    background: var(--bg);
  }
  :global(:root[data-theme="paper"]) .close {
    background: var(--panel);
  }
  .hero {
    position: relative;
    flex: none;
    aspect-ratio: 16 / 9;
    max-height: 30vh;
    width: 100%;
    background: var(--panel);
    --sa-w: 192px;
  }
  :global(:root[data-theme="pixel"]) .hero {
    border-bottom: var(--border) solid var(--line);
  }
  :global(:root[data-theme="paper"]) .hero {
    box-shadow: inset 0 -1px 0 var(--line);
  }
  .sbody {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 24px 32px 24px;
  }
  .sbody.pad {
    padding-top: 40px;
  }
  .kicker {
    margin-bottom: 4px;
  }
  :global(:root[data-theme="pixel"]) .kicker .chip {
    box-shadow: inset 0 0 0 2px currentColor;
  }
  :global(:root[data-theme="paper"]) .kicker .chip {
    padding: 0 9px;
    border-radius: 11px;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent-2) 55%, transparent);
  }
  .sheet h2 {
    margin: 0;
    font-family: var(--title);
    font-weight: 800;
    font-size: 36px;
    line-height: 1.05;
    overflow-wrap: anywhere;
  }
  .sub {
    font-family: var(--label);
    font-size: 15px;
    color: var(--muted);
  }
  .blurb {
    margin: 8px 0 0;
    font-size: 16px;
    line-height: 1.5;
    color: var(--muted);
  }
  .callout {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr);
    gap: 12px;
    margin-top: 12px;
    padding: 14px 16px;
  }
  .callout p {
    margin: 2px 0 0;
    font-size: 15px;
    line-height: 1.45;
  }
  .callout .label {
    display: block;
  }
  :global(:root[data-theme="pixel"]) .callout {
    border: 2px solid var(--line);
  }
  :global(:root[data-theme="pixel"]) .callout.quiet {
    border-style: dashed;
    border-color: var(--faint);
  }
  :global(:root[data-theme="pixel"]) .callout.hot {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="paper"]) .callout {
    background: color-mix(in srgb, var(--accent) 7%, var(--panel));
    box-shadow: inset 3px 0 0 var(--accent);
  }
  :global(:root[data-theme="paper"]) .callout > :global(svg) {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .callout.quiet {
    background: color-mix(in srgb, var(--fg) 4%, var(--panel));
    box-shadow: inset 3px 0 0 color-mix(in srgb, var(--fg) 30%, transparent);
  }
  :global(:root[data-theme="paper"]) .callout.quiet > :global(svg) {
    color: var(--muted);
  }
  :global(:root[data-theme="paper"]) .callout.hot {
    background: color-mix(in srgb, var(--accent-2) 8%, var(--panel));
    box-shadow: inset 3px 0 0 var(--accent-2);
  }
  :global(:root[data-theme="paper"]) .callout.hot > :global(svg) {
    color: var(--accent-2);
  }
  .callout .whybtns {
    margin-top: 12px;
    flex-wrap: wrap;
  }
  /* Files a mashup needs in the game's folder: a tick when found, what to do when not. */
  .needs {
    list-style: none;
    margin: 16px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .needs li {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
  }
  .needs b {
    display: block;
    font-family: var(--label);
    font-size: 15px;
  }
  .needs span {
    display: block;
    margin-top: 2px;
    font-size: 14px;
    line-height: 1.45;
    color: var(--muted);
  }
  :global(:root[data-theme="pixel"]) .needs li {
    border: 2px solid var(--line);
  }
  :global(:root[data-theme="pixel"]) .needs li.ok {
    border-style: dashed;
    border-color: var(--faint);
  }
  :global(:root[data-theme="paper"]) .needs li {
    background: color-mix(in srgb, var(--accent) 7%, var(--panel));
    box-shadow: inset 3px 0 0 var(--accent);
  }
  :global(:root[data-theme="paper"]) .needs li > :global(svg) {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .needs li.ok {
    background: color-mix(in srgb, var(--fg) 4%, var(--panel));
    box-shadow: inset 3px 0 0 color-mix(in srgb, var(--fg) 30%, transparent);
  }
  :global(:root[data-theme="paper"]) .needs li.ok > :global(svg) {
    color: var(--muted);
  }
  .facts dd .link + .link {
    margin-left: 16px;
  }
  .facts {
    display: flex;
    flex-direction: column;
    margin: 16px 0 0;
  }
  .facts > div {
    display: grid;
    grid-template-columns: 132px minmax(0, 1fr);
    align-items: center;
    gap: 16px;
    min-height: 48px;
    padding: 8px 0;
  }
  .facts dt {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: var(--label);
    font-size: 13px;
    font-weight: 700;
    color: var(--faint);
  }
  .facts dd {
    margin: 0;
    font-family: var(--label);
    font-size: 15px;
    overflow-wrap: anywhere;
  }
  :global(:root[data-theme="pixel"]) .facts dt {
    text-transform: uppercase;
    font-size: 11px;
    letter-spacing: 0.06em;
  }
  :global(:root[data-theme="pixel"]) .facts > div + div {
    border-top: 2px dotted var(--faint);
  }
  :global(:root[data-theme="paper"]) .facts > div + div {
    border-top: 1px solid color-mix(in srgb, var(--line) 15%, transparent);
  }
  .link {
    text-align: left;
    text-decoration: underline;
    text-decoration-thickness: 1px;
    text-underline-offset: 4px;
  }
  .link:hover {
    text-decoration-thickness: 2px;
  }
  :global(:root[data-theme="paper"]) .link {
    color: var(--accent-2);
  }
  :global(:root[data-theme="pixel"]) .link:hover {
    background: var(--hi-bg);
    color: var(--hi-fg);
  }
  .faint {
    color: var(--faint);
  }
  .actions {
    flex: none;
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    padding: 20px 32px 24px;
  }
  .actions:empty {
    display: none;
  }
  :global(:root[data-theme="pixel"]) .actions {
    border-top: var(--border) solid var(--line);
  }
  :global(:root[data-theme="paper"]) .actions {
    box-shadow: inset 0 1px 0 color-mix(in srgb, var(--line) 18%, transparent);
  }
  .progress {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .in {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 12px;
  }
  .field {
    height: 52px;
    padding: 0 14px;
    font-size: 17px;
    background: var(--bg);
    border: var(--border) solid var(--line);
  }
  :global(:root[data-theme="paper"]) .field {
    border-width: 1.5px;
    border-radius: var(--radius);
  }

  /* ---- buttons under the pointer ---------------------------------------------------------- */
  :global(:root[data-theme="pixel"]) .get .btn:hover,
  :global(:root[data-theme="pixel"]) .sheet .btn:hover {
    background: var(--hi-bg);
    color: var(--hi-fg);
  }
  :global(:root[data-theme="pixel"]) .get .btn.primary:hover,
  :global(:root[data-theme="pixel"]) .sheet .btn.primary:hover {
    outline: var(--border) solid var(--fg);
    outline-offset: 4px;
  }
  :global(:root[data-theme="paper"]) .get .btn:hover,
  :global(:root[data-theme="paper"]) .sheet .btn:hover {
    box-shadow: 4px 4px 0 var(--fg);
    transform: translate(-2px, -2px);
  }
  .btn:disabled {
    opacity: 0.45;
    pointer-events: none;
  }

  @media (max-width: 1180px) {
    .get {
      padding-inline: 40px;
    }
    .up {
      grid-template-columns: 96px minmax(0, 1fr) auto auto;
      gap: 0 20px;
    }
    .thumb {
      width: 96px;
    }
    .why {
      grid-template-columns: 24px minmax(0, 1fr);
    }
    .whybtns {
      grid-column: 2;
    }
  }
</style>
