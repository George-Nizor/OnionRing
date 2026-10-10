<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type Copy, type Emulator, type EmulatorChecks } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { ago, bytes, playtime } from "$lib/format";
  import Art from "./Art.svelte";
  import Glyph from "./Glyph.svelte";
  import LibGlyph from "./LibGlyph.svelte";
  import SystemArt from "./SystemArt.svelte";
  import CollectionPicker from "./CollectionPicker.svelte";
  import LaunchOptions from "./LaunchOptions.svelte";
  import PlayHistory from "./PlayHistory.svelte";
  import { collections, collectionFilter } from "./collections.svelte";

  let { emulators }: { emulators: Emulator[] } = $props();

  collections.start();

  const g = $derived(app.current);
  const platform = $derived(g ? app.platforms.get(g.platform) : undefined);
  const emu = $derived(emulators.find((e) => e.id === g?.emulator));
  const choices = $derived(emulators.filter((e) => g && e.platforms.includes(g.platform)));
  const running = $derived(g ? app.running.includes(g.id) : false);
  const port = $derived(g?.appId ? (app.catalog?.ports.find((p) => p.id === g.appId) ?? null) : null);
  const installed = $derived(g?.appId ? app.catalog?.installed.find((a) => a.id === g.appId) : undefined);
  const mine = $derived(g ? collections.list.filter((c) => c.gameIds.includes(g.id)) : []);
  const hasArt = $derived(!!(g?.hero || g?.cover));

  let copies = $state<Copy[]>([]);
  let checks = $state<EmulatorChecks[]>([]);
  let more = $state(false);
  let picking = $state(false);
  let showFiles = $state(false);

  const key = $derived(g?.key);
  $effect(() => {
    const k = key;
    more = false;
    picking = false;
    showFiles = false;
    copies = [];
    if (k) api.copies(k).then((c) => (copies = c)).catch(() => (copies = []));
  });
  $effect(() => {
    api.setupChecks().then((c) => (checks = c)).catch(() => (checks = []));
  });
  const failing = $derived(checks.find((c) => c.id === g?.emulator)?.checks.filter((c) => !c.ok && !c.optional) ?? []);
  const maxCopy = $derived(Math.max(1, ...copies.map((c) => c.size)));

  const date = (t: number) => new Date(t * 1000).toLocaleDateString(undefined, { day: "numeric", month: "long", year: "numeric" });
  const list = (xs: string[]) => (xs.length < 2 ? xs.join("") : `${xs.slice(0, -1).join(", ")} and ${xs.at(-1)}`);

  async function pickCover() {
    if (!g) return;
    const file = await open({ multiple: false, filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }] });
    if (typeof file === "string") {
      await api.setCover(g.id, file);
      await app.reload();
    }
  }
  async function setEmu(id: string) {
    if (!g) return;
    await api.setGameEmulator(g.id, id);
    await app.reload();
  }
  async function hide() {
    if (!g) return;
    await api.setFlag(g.id, "hidden", !g.hidden);
    await app.reload();
    app.back();
  }
  function openCollection(id: number) {
    app.setPlatformFilter(collectionFilter(id));
    app.go({ name: "library" });
  }
</script>

{#if g}
  <div class="page">
    <div class="banner" class:blank={!hasArt}>
      {#if hasArt}
        <Art game={g} kind="wide" />
      {:else}
        <div class="field" aria-hidden="true"><div class="sys"><SystemArt id={g.platform} /></div></div>
      {/if}
    </div>

    <div class="head">
      <div class="cover"><Art game={g} /></div>
      <div class="info">
        <span class="kicker label">{g.kind === "port" ? (port?.kind ?? "PC port") : (platform?.name ?? "")}</span>
        {#key g.id}<h1 class:long={g.title.length > 28}>{g.title}</h1>{/key}

        <dl class="stats">
          <div class="stat">
            <dt class="label"><Glyph name="clock" size={12} />Played</dt>
            <dd>{g.playtime ? playtime(g.playtime) : "Not yet"}</dd>
          </div>
          {#if g.lastPlayed}
            <div class="stat"><dt class="label"><Glyph name="refresh" size={12} />Last played</dt><dd>{ago(g.lastPlayed)}</dd></div>
          {/if}
          <div class="stat"><dt class="label"><Glyph name="disk" size={12} />Size</dt><dd>{bytes(g.size)}</dd></div>
          {#if g.kind === "rom"}
            <div class="stat">
              <dt class="label"><Glyph name="pad" size={12} />Emulator</dt>
              <dd>{emu ? emu.name : "None"}{#if emu?.version}<small>{emu.version}</small>{/if}</dd>
            </div>
          {:else if installed}
            <div class="stat"><dt class="label"><Glyph name="info" size={12} />Version</dt><dd>{installed.version}</dd></div>
          {/if}
        </dl>

        <div class="actions">
          <button class="btn big primary play" data-nav onclick={() => app.play(g)} disabled={g.missing || running}>
            <Glyph name={running ? "clock" : g.packed ? "zip" : "play"} size={24} />{running ? "Playing" : g.packed ? "Unzip" : "Play"}
          </button>
          {#if g.kind === "port"}
            <button class="btn big" data-nav onclick={() => api.configurePort(g.id).catch((e) => app.toast(String(e), "error"))}>
              <Glyph name="sliders" size={24} />Port settings
            </button>
          {/if}
          <button class="btn big icon" class:on={g.favorite} data-nav onclick={() => app.toggleFavorite(g)} aria-label="Favourite" aria-pressed={g.favorite} title={g.favorite ? "Favourite" : "Add to favourites"}>
            <Glyph name="star" size={24} />
          </button>
          <div class="pickwrap">
            <button class="btn big icon" class:on={picking} class:has={mine.length > 0} data-nav onclick={() => (picking = !picking)} aria-label="Collections" aria-expanded={picking} title="Collections">
              <LibGlyph name={mine.length ? "bookmark-on" : "bookmark"} size={24} />
              {#if mine.length}<span class="n">{mine.length}</span>{/if}
            </button>
            {#if picking}<CollectionPicker game={g} onclose={() => (picking = false)} />{/if}
          </div>
          <button class="btn big icon" class:on={more} data-nav onclick={() => (more = !more)} aria-label="More" aria-expanded={more} title="More">
            <Glyph name="options" size={24} />
          </button>
        </div>

        {#if more}
          <div class="more">
            {#each choices.length > 1 ? choices : [] as c (c.id)}
              <button class="btn" class:on={c.id === g.emulator} data-nav onclick={() => setEmu(c.id)}><Glyph name={c.id === g.emulator ? "check" : "pad"} size={24} />{c.name}</button>
            {/each}
            <button class="btn" data-nav onclick={pickCover}><Glyph name="library" size={24} />Change cover</button>
            <button class="btn" data-nav onclick={() => api.reveal(g.launchPath)}><Glyph name="folder" size={24} />Open folder</button>
            <button class="btn" data-nav onclick={hide}><Glyph name="eye" size={24} />{g.hidden ? "Show in library" : "Hide from library"}</button>
          </div>
        {/if}

        {#if g.kind === "rom" && emu && failing.length}
          <div class="callout">
            <Glyph name="alert" size={24} />
            <div class="ct">
              <b>{emu.name} needs the {list(failing.map((c) => c.what))} before games will start</b>
              <span>{failing[0].help}</span>
            </div>
            <div class="ca">
              {#if failing[0].folder}
                <button class="btn" data-nav onclick={() => api.openFolder(failing[0].folder!).catch((e) => app.toast(String(e), "error"))}><Glyph name="folder" size={24} />Open its folder</button>
              {/if}
              <button class="btn" data-nav onclick={() => app.go({ name: "systems", item: emu.id })}><Glyph name="pad" size={24} />Systems</button>
            </div>
          </div>
        {:else if g.kind === "rom" && !emu}
          <div class="callout">
            <Glyph name="alert" size={24} />
            <div class="ct"><b>No emulator for {platform?.name ?? "this system"} yet</b><span>Get one and this game is ready to play.</span></div>
            <div class="ca"><button class="btn" data-nav onclick={() => app.go({ name: "get", tab: "emulators" })}><Glyph name="get" size={24} />Get one</button></div>
          </div>
        {:else if g.missing}
          <div class="callout">
            <Glyph name="alert" size={24} />
            <div class="ct"><b>This game's files are gone</b><span>Plug the drive back in, or rescan if you moved them.</span></div>
            <div class="ca"><button class="btn" data-nav onclick={() => app.scan()}><Glyph name="refresh" size={24} />Rescan</button></div>
          </div>
        {/if}
      </div>
    </div>

    <div class="sections">
      <div class="col main">
        <PlayHistory game={g} />
      </div>

      <div class="col side">
        {#if g.kind === "rom" && emu}
          <LaunchOptions game={g} {emu} />
        {/if}

        {#if port}
          <section>
            <div class="sh"><Glyph name="info" size={24} /><h2 class="label">About this port</h2><span class="rule"></span></div>
            {#if port.blurb}<p class="blurb">{port.blurb}</p>{/if}
            {#if port.requires}<p class="needs"><Glyph name="alert" size={12} /><span><b>You need</b> {port.requires}</span></p>{/if}
          </section>
        {/if}

        <section>
          <div class="sh"><Glyph name="library" size={24} /><h2 class="label">Details</h2><span class="rule"></span></div>
          <dl class="facts">
            <dt class="label">Collections</dt>
            <dd class="chips">
              {#each mine as c (c.id)}
                <button class="chip" data-nav onclick={() => openCollection(c.id)}><LibGlyph name="bookmark-on" size={12} />{c.name}</button>
              {:else}
                <button class="chip ghost" data-nav onclick={() => (picking = true)}><Glyph name="plus" size={12} />Add to one</button>
              {/each}
            </dd>
            {#if port}
              <dt class="label">Port</dt><dd>{port.name}</dd>
              {#if port.original.length}<dt class="label">Original</dt><dd>{port.original.join(", ")}</dd>{/if}
            {/if}
            {#if g.serial}<dt class="label">Serial</dt><dd>{g.serial}</dd>{/if}
            {#if g.region}<dt class="label">Region</dt><dd>{g.region}</dd>{/if}
            <dt class="label">Times played</dt><dd>{g.playCount}</dd>
            <dt class="label">Added</dt><dd>{date(g.addedAt)}</dd>
          </dl>
        </section>

        {#if copies.length > 1}
          <section class="disk">
            <div class="sh">
              <Glyph name="disk" size={24} /><h2 class="label">On disk</h2><span class="rule"></span>
              <span class="meta label">{copies.length} copies</span>
            </div>
            <div class="copies">
              {#each copies as c, i (c.path)}
                <div class="copy" class:keep={c.primary} style:--w="{Math.max(8, (c.size / maxCopy) * 100)}%" style:--i={i}>
                  <span class="bar"></span>
                  <span class="cap label"><Glyph name={c.archive ? "zip" : c.primary ? "check" : "disk"} size={12} />{bytes(c.size)}{c.primary ? " · the one you play" : ""}</span>
                  {#if showFiles}<span class="path">{c.path}</span>{/if}
                </div>
              {/each}
            </div>
            <div class="acts">
              <button class="btn" data-nav onclick={() => app.go({ name: "tidy" })}><Glyph name="broom" size={24} />Tidy up</button>
              <button class="btn" class:on={showFiles} data-nav onclick={() => (showFiles = !showFiles)}><Glyph name="eye" size={24} />{showFiles ? "Hide files" : "Show files"}</button>
            </div>
          </section>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .page {
    --pad: clamp(32px, 4vw, 72px);
    --cover: clamp(168px, 14vw, 260px);
    --gutter: clamp(28px, 2.8vw, 48px);
    height: 100%;
    overflow-y: auto;
    padding-bottom: 72px;
  }

  /* ---- banner ---- */
  .banner {
    position: relative;
    height: clamp(180px, 31vh, 380px);
    overflow: hidden;
  }
  :global(:root[data-theme="pixel"]) .banner {
    border-bottom: var(--border) solid var(--line);
    animation: pxshow 0.3s steps(6) both;
  }
  @keyframes pxshow {
    from {
      clip-path: inset(0 0 100% 0);
    }
  }
  :global(:root[data-theme="paper"]) .banner {
    border-bottom: 1.5px solid var(--line);
    animation: einkin 0.5s steps(3) both;
  }
  @keyframes einkin {
    0% {
      filter: invert(1);
    }
    35% {
      filter: invert(0.2);
    }
  }
  :global(:root[data-theme="paper"]) .banner :global(.art) {
    filter: var(--img-filter-focus);
  }
  .field {
    position: absolute;
    inset: 0;
  }
  .sys {
    position: absolute;
    right: calc(var(--pad) + 2%);
    top: 14%;
    bottom: 14%;
    width: min(46%, 520px);
  }
  :global(:root[data-theme="pixel"]) .field {
    background: radial-gradient(circle, var(--faint) 1px, transparent 1.5px) 0 0 / 12px 12px;
  }
  :global(:root[data-theme="pixel"]) .sys {
    color: var(--fg);
    background: var(--bg);
    padding: 12px;
  }
  :global(:root[data-theme="paper"]) .field {
    background:
      repeating-linear-gradient(0deg, transparent 0 27px, color-mix(in srgb, var(--line) 9%, transparent) 27px 28px),
      var(--panel);
  }
  :global(:root[data-theme="paper"]) .sys {
    color: var(--fg);
  }

  /* ---- head: cover overlapping the banner, title and actions beside it ---- */
  .head {
    display: grid;
    grid-template-columns: var(--cover) minmax(0, 1fr);
    column-gap: var(--gutter);
    padding: 0 var(--pad);
  }
  .cover {
    position: relative;
    width: var(--cover);
    aspect-ratio: 3 / 4;
    margin-top: calc(var(--cover) * -0.72);
    z-index: 1;
    animation: rise 0.5s var(--ease) both 0.1s;
  }
  :global(:root[data-theme="pixel"]) .cover {
    outline: 4px solid var(--line);
    background: var(--bg);
    animation-timing-function: steps(4);
  }
  :global(:root[data-theme="paper"]) .cover {
    box-shadow:
      0 0 0 1.5px var(--line),
      8px 8px 0 var(--line);
  }
  .info {
    min-width: 0;
    padding-top: 26px;
  }
  .kicker {
    display: block;
    color: var(--faint);
  }
  :global(:root[data-theme="paper"]) .kicker {
    color: var(--accent);
  }
  h1 {
    margin: 10px 0 0;
    font-family: var(--title);
    font-weight: 800;
    font-size: clamp(36px, 3.4vw, 60px);
    line-height: 1.02;
    letter-spacing: -0.01em;
    text-wrap: balance;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    overflow: hidden;
    animation: rise 0.45s var(--ease) both;
  }
  h1.long {
    font-size: clamp(30px, 2.6vw, 46px);
  }
  :global(:root[data-theme="pixel"]) h1 {
    letter-spacing: 0;
    animation: type 0.4s steps(10) both;
  }
  @keyframes type {
    from {
      clip-path: inset(0 100% 0 0);
    }
  }

  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 14px 0;
    margin: 22px 0 26px;
  }
  .stat {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0 28px;
  }
  .stat:first-child {
    padding-left: 0;
  }
  .stat + .stat {
    border-left: 1px solid color-mix(in srgb, var(--line) 22%, transparent);
  }
  :global(:root[data-theme="pixel"]) .stat + .stat {
    border-left: 2px dotted var(--faint);
  }
  .stat dt {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--faint);
    font-weight: 400;
  }
  .stat dd {
    margin: 0;
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-family: var(--title);
    font-size: 20px;
    font-weight: 700;
    white-space: nowrap;
  }
  .stat small {
    font-family: var(--label);
    font-size: 12px;
    font-weight: 400;
    color: var(--faint);
  }

  .actions,
  .more {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
  }
  .actions .btn.icon {
    width: 60px;
  }
  .play {
    min-width: 168px;
    justify-content: center;
  }
  .pickwrap {
    position: relative;
  }
  .n {
    position: absolute;
    top: 6px;
    right: 6px;
    min-width: 16px;
    height: 16px;
    padding: 0 3px;
    display: grid;
    place-items: center;
    font-family: var(--label);
    font-size: 10px;
    line-height: 1;
  }
  :global(:root[data-theme="pixel"]) .n {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .btn.on .n,
  :global(:root[data-theme="pixel"]) .btn:focus-visible .n {
    background: var(--bg);
    color: var(--fg);
  }
  :global(:root[data-theme="paper"]) .n {
    background: var(--accent);
    color: var(--panel);
    border-radius: 8px;
  }
  :global(:root[data-theme="paper"]) .has {
    color: var(--accent);
  }
  .more {
    margin-top: 16px;
    animation: rise 0.25s var(--ease) both;
  }
  :global(:root[data-theme="pixel"]) .more {
    animation-timing-function: steps(3);
  }
  :global(:root[data-theme="pixel"]) .btn.on {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="paper"]) .btn.on {
    color: var(--accent);
    border-color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .actions .btn.icon.on :global(svg) {
    fill: currentColor;
  }
  .btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  /* mouse hover: a lift in paper, a ring in pixel */
  :global(:root[data-theme="pixel"]) .page :global(.btn:hover:not(:focus-visible):not(:disabled)) {
    outline: 2px solid var(--fg);
    outline-offset: 3px;
  }
  :global(:root[data-theme="paper"]) .page :global(.btn:hover:not(:focus-visible):not(:disabled)) {
    box-shadow: 3px 3px 0 var(--fg);
    transform: translate(-1px, -1px);
  }

  .callout {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 16px;
    max-width: 860px;
    margin-top: 24px;
    padding: 14px 14px 14px 18px;
    animation: rise 0.35s var(--ease) both 0.15s;
  }
  .ct {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-family: var(--label);
    font-size: 14px;
    line-height: 1.45;
  }
  .ct b {
    font-family: var(--title);
    font-size: 17px;
  }
  .ca {
    display: flex;
    gap: 10px;
  }
  :global(:root[data-theme="pixel"]) .callout {
    border: var(--border) solid var(--line);
    box-shadow: 6px 6px 0 var(--fg);
    animation-timing-function: steps(3);
  }
  :global(:root[data-theme="pixel"]) .callout > :global(svg) {
    animation: blink 1s steps(1) infinite;
  }
  :global(:root[data-theme="paper"]) .callout {
    border-left: 3px solid var(--accent);
    background: color-mix(in srgb, var(--accent) 8%, var(--panel));
  }
  :global(:root[data-theme="paper"]) .callout > :global(svg),
  :global(:root[data-theme="paper"]) .ct b {
    color: var(--accent);
  }

  /* ---- sections ---- */
  .sections {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(340px, 440px);
    gap: 48px clamp(40px, 4vw, 72px);
    padding: 56px var(--pad) 0;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 44px;
    min-width: 0;
  }
  .page :global(.sh) {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0 0 18px;
  }
  .page :global(.sh h2) {
    margin: 0;
  }
  .page :global(.sh .rule) {
    flex: 1;
    align-self: center;
  }
  .page :global(.sh .meta) {
    color: var(--faint);
    font-weight: 400;
  }
  :global(:root[data-theme="pixel"]) .page :global(.sh .rule) {
    height: var(--border);
    background: var(--line);
  }
  :global(:root[data-theme="paper"]) .page :global(.sh .rule) {
    height: 1px;
    background: color-mix(in srgb, var(--line) 30%, transparent);
  }
  :global(:root[data-theme="paper"]) .page :global(.sh > svg) {
    color: var(--accent);
  }

  .blurb {
    margin: 0;
    font-size: 16px;
    line-height: 1.6;
    color: var(--muted);
  }
  :global(:root[data-theme="pixel"]) .blurb {
    color: var(--fg);
  }
  .needs {
    display: flex;
    gap: 10px;
    margin: 14px 0 0;
    font-family: var(--label);
    font-size: 14px;
    line-height: 1.5;
  }
  .needs :global(svg) {
    margin-top: 3px;
  }
  :global(:root[data-theme="paper"]) .needs b {
    color: var(--accent);
  }

  .facts {
    display: grid;
    grid-template-columns: 8.5em minmax(0, 1fr);
    margin: 0;
    font-family: var(--label);
    font-size: 14px;
  }
  .facts dt,
  .facts dd {
    padding: 10px 0;
    min-height: 44px;
    display: flex;
    align-items: center;
  }
  .facts dt {
    color: var(--faint);
    font-weight: 400;
  }
  .facts dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  :global(:root[data-theme="pixel"]) .facts dt:not(:first-of-type),
  :global(:root[data-theme="pixel"]) .facts dd:not(:first-of-type) {
    border-top: 2px dotted var(--faint);
  }
  :global(:root[data-theme="paper"]) .facts dt:not(:first-of-type),
  :global(:root[data-theme="paper"]) .facts dd:not(:first-of-type) {
    border-top: 1px solid color-mix(in srgb, var(--line) 15%, transparent);
  }
  .chips {
    flex-wrap: wrap;
    gap: 8px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 10px;
    font-family: var(--label);
    font-size: 13px;
    font-weight: 700;
  }
  :global(:root[data-theme="pixel"]) .chip {
    border: 2px solid var(--fg);
  }
  :global(:root[data-theme="pixel"]) .chip:hover:not(:focus-visible) {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .chip.ghost {
    border-style: dashed;
    color: var(--faint);
  }
  :global(:root[data-theme="paper"]) .chip {
    border-radius: 14px;
    background: color-mix(in srgb, var(--accent) 10%, var(--panel));
    color: var(--accent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 40%, transparent);
  }
  :global(:root[data-theme="paper"]) .chip:hover {
    background: var(--accent);
    color: var(--panel);
  }
  :global(:root[data-theme="paper"]) .chip.ghost {
    background: none;
    color: var(--faint);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--line) 35%, transparent);
  }
  :global(:root[data-theme="paper"]) .chip:focus-visible::after {
    left: 10px;
    right: 10px;
    bottom: -5px;
    height: 2px;
  }

  /* ---- on disk ---- */
  .copies {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-bottom: 18px;
  }
  .copy {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 12px;
  }
  .bar {
    width: calc(var(--w) * 0.62);
    height: 20px;
    transform-origin: left;
    animation: grow 0.5s var(--ease) both;
    animation-delay: calc(var(--i) * 80ms);
  }
  :global(:root[data-theme="pixel"]) .bar {
    border: var(--border) solid var(--line);
    animation-timing-function: steps(6);
  }
  :global(:root[data-theme="pixel"]) .keep .bar {
    background: var(--fg);
  }
  :global(:root[data-theme="paper"]) .bar {
    border: 1.5px dashed var(--line);
  }
  :global(:root[data-theme="paper"]) .keep .bar {
    background: var(--fg);
    border-style: solid;
  }
  @keyframes grow {
    from {
      transform: scaleX(0);
    }
  }
  .cap {
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }
  .path {
    flex-basis: 100%;
    font-family: var(--label);
    font-size: 12px;
    color: var(--faint);
    overflow-wrap: anywhere;
    user-select: text;
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  @media (max-width: 1180px) {
    .sections {
      grid-template-columns: minmax(0, 1fr);
    }
    .col.side {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
      gap: 44px 48px;
      align-items: start;
    }
  }
</style>
