<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type Backup, type SaveSource, type Source } from "$lib/api";
  import { app, type Theme } from "$lib/store.svelte";
  import { bytes } from "$lib/format";
  import Glyph from "./Glyph.svelte";
  import Logo from "./Logo.svelte";

  let sources = $state<Source[]>([]);
  let update = $state<Update | null>(null);
  let updateState = $state<"idle" | "checking" | "none" | "error" | "installing" | "failed">("idle");
  let autoScan = $state<boolean | null>(null);
  let saveSources = $state<SaveSource[]>([]);
  let backups = $state<Backup[]>([]);
  let backingUp = $state(false);
  let fresh = $state<string | null>(null);

  onMount(() => {
    api.sources().then((s) => (sources = s)).catch(() => {});
    api.autoScan().then((on) => (autoScan = on)).catch(() => {});
    api.saveSources().then((s) => (saveSources = s)).catch(() => {});
    loadBackups();
    const off = listen("backups-changed", loadBackups);
    return () => void off.then((f) => f());
  });

  async function loadBackups() {
    backups = await api.backups().catch(() => backups);
  }

  const folderName = (p: string) => {
    const parts = p.split(/[\\/]/).filter(Boolean);
    return { drive: parts[0] ?? "", name: parts.at(-1) ?? p };
  };

  async function addFolder() {
    const dir = await open({ directory: true, multiple: false, title: "A folder with emulators or games" });
    if (typeof dir !== "string") return;
    await api.addSource(dir);
    sources = await api.sources();
    app.scan();
  }
  async function remove(s: Source) {
    const { name } = folderName(s.path);
    if (!(await ask(`Stop reading ${name}?\n\nThe folder itself is not touched.`, { title: "Remove folder" }))) return;
    await api.removeSource(s.id);
    sources = await api.sources();
    app.scan();
  }
  async function toggleAutoScan() {
    if (autoScan === null) return;
    const on = !autoScan;
    autoScan = on;
    try {
      await api.setAutoScan(on);
    } catch (e) {
      autoScan = !on;
      app.toast(String(e), "error", 8000);
    }
  }
  async function backUp() {
    backingUp = true;
    try {
      const b = await api.backUpSaves();
      await loadBackups();
      fresh = b.name;
      if (b.failed.length) app.toast(`Backed up, but ${b.failed.length === 1 ? "one folder" : `${b.failed.length} folders`} could not be saved`, "error", 9000);
      else app.toast(`Saves backed up (${bytes(b.size)})`, "ok");
    } catch (e) {
      app.toast(String(e), "error", 9000);
    }
    backingUp = false;
  }
  let token = $state("");
  let tokenSaving = $state(false);
  async function saveToken(t: string | null) {
    tokenSaving = true;
    try {
      await api.setGithubToken(t);
      token = "";
      if (app.boot) app.boot.githubToken = !!t;
      app.toast(t ? "GitHub token saved" : "GitHub token removed", "ok");
    } catch (e) {
      app.toast(String(e), "error", 9000);
    }
    tokenSaving = false;
  }
  async function installUpdate() {
    if (!update) return;
    updateState = "installing";
    try {
      await app.installUpdate(update);
    } catch {
      updateState = "failed";
    }
  }
  async function checkUpdate() {
    updateState = "checking";
    try {
      update = await check();
      updateState = update ? "idle" : "none";
    } catch {
      updateState = "error";
    }
  }

  const bp = $derived(app.progress && (app.progress.stage === "backup" || app.progress.stage === "done") ? app.progress : null);
  const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? "" : "s"}`;
  const dateOf = (unix: number) => new Date(unix * 1000).toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short", year: "numeric" });
  const timeOf = (unix: number) => new Date(unix * 1000).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  function whenOf(unix: number) {
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const diff = Math.round((today.getTime() - new Date(unix * 1000).setHours(0, 0, 0, 0)) / 86_400_000);
    return diff <= 0 ? "Today" : diff === 1 ? "Yesterday" : diff < 30 ? `${diff} days ago` : "";
  }

  // Logo states, cycled so you can see each animation.
  type LS = "idle" | "busy" | "launch" | "done";
  let logoState = $state<LS>("idle");
  let progress = $state(0);
  let timer: ReturnType<typeof setInterval> | undefined;
  function showState(s: LS) {
    clearInterval(timer);
    logoState = "idle";
    requestAnimationFrame(() => {
      logoState = s;
      if (s === "busy") {
        progress = 0;
        timer = setInterval(() => {
          progress += 0.11;
          if (progress > 1.05) progress = 0;
        }, 450);
      }
    });
  }
  $effect(() => () => clearInterval(timer));

  const themes: [Theme, string, string][] = [
    ["pixel", "Pixel", "8-bit colour, chunky and blinky"],
    ["paper", "Paper", "Ink on paper, like an e-reader"],
  ];
</script>

<div class="settings">
  <div class="sheet">
    <section>
      <div class="side">
        <h2><Glyph name="eye" size={24} />Look</h2>
        <p>Two looks, same layout. Switch any time.</p>
      </div>
      <div class="body themes">
        {#each themes as [t, name, blurb] (t)}
          <button class="theme" class:on={app.theme === t} data-nav onclick={() => app.setTheme(t)} aria-pressed={app.theme === t} aria-label="{name} theme">
            <div class="preview" data-theme={t}>
              <Logo size={56} variant={t === "pixel" ? "pixel" : "ink"} state="still" />
              <div class="sample">
                <span class="aa" style:font-family="var(--title)">Aa</span>
                <span class="mini" style:font-family="var(--label)">{name}</span>
              </div>
              <div class="mini-tiles"><span></span><span></span><span></span><span></span></div>
            </div>
            <div class="cap">
              <span class="radio" aria-hidden="true">{#if app.theme === t}<Glyph name="check" size={24} />{/if}</span>
              <span class="cname">{name}</span>
              <span class="cblurb">{blurb}</span>
            </div>
          </button>
        {/each}
      </div>
    </section>

    <section>
      <div class="side">
        <h2><Glyph name="folder" size={24} />Game folders</h2>
        <p>Where OnionRing looks for emulators and games.</p>
      </div>
      <div class="body">
        <div class="list">
          {#each sources as s (s.id)}
            {@const f = folderName(s.path)}
            <div class="item" class:gone={!s.exists}>
              <span class="ic"><Glyph name={s.exists ? "folder" : "alert"} size={24} /></span>
              <div class="iname">
                <b>{f.name}</b>
                <span class="label">{s.exists ? `On ${f.drive}` : `Can't find it on ${f.drive}`}</span>
              </div>
              <button class="btn icon" data-nav onclick={() => api.reveal(s.path)} aria-label="Open {f.name}" title="Open folder"><Glyph name="eye" size={24} /></button>
              <button class="btn icon" data-nav onclick={() => remove(s)} aria-label="Remove {f.name}" title="Stop reading this folder"><Glyph name="close" size={24} /></button>
            </div>
          {:else}
            <div class="item none"><span class="ic"><Glyph name="folder" size={24} /></span><div class="iname"><b>No folders yet</b><span class="label">Add the folder that holds your emulators and games.</span></div></div>
          {/each}
        </div>
        <div class="acts">
          <button class="btn" data-nav onclick={addFolder}><Glyph name="plus" size={24} />Add folder</button>
          <button class="btn" data-nav onclick={() => app.scan()} disabled={app.progress !== null}><Glyph name="refresh" size={24} />Rescan</button>
          <button class="btn" data-nav onclick={() => api.refreshArt(true).then((n) => app.toast(`New art for ${n} games`, "ok"))}><Glyph name="library" size={24} />Refresh art</button>
        </div>

        <button class="switchrow" data-nav role="switch" aria-checked={autoScan === true} disabled={autoScan === null} onclick={toggleAutoScan}>
          <span class="stext">
            <b>Auto-pickup</b>
            <span>Watches your game folders and adds new games by itself.</span>
          </span>
          <span class="state label">{autoScan ? "On" : "Off"}</span>
          <span class="track" class:on={autoScan}><span class="knob"></span></span>
        </button>
      </div>
    </section>

    <section>
      <div class="side">
        <h2><Glyph name="disk" size={24} />Saves</h2>
        <p>Zips a copy of your save files into your OnionRing folder. The originals stay where they are.</p>
      </div>
      <div class="body">
        {#if saveSources.length}
          <div class="covers">
            <span class="label">Backs up</span>
            {#each saveSources as s (s.name)}<span class="chip">{s.name}</span>{/each}
          </div>
        {/if}
        <div class="acts top">
          <button class="btn primary" data-nav onclick={backUp} disabled={backingUp || !saveSources.length}><Glyph name="disk" size={24} />{backingUp ? "Backing up…" : "Back up saves now"}</button>
          {#if !saveSources.length}<span class="hint">No saves found yet. Play something first.</span>{/if}
        </div>

        {#if backingUp}
          <div class="progress" aria-live="polite">
            <div class="ptrack">
              {#each Array.from({ length: Math.max(1, bp?.total ?? saveSources.length) }) as _, i (i)}
                <span class:done={i < (bp?.done ?? 0)} class:now={i === (bp?.done ?? 0)}></span>
              {/each}
            </div>
            <span class="label">{bp?.stage === "backup" ? `Saving ${bp.detail}` : "Finishing up"} · {Math.min((bp?.done ?? 0) + 1, bp?.total ?? 1)} of {bp?.total ?? saveSources.length}</span>
          </div>
        {/if}

        <div class="list backups">
          {#each backups as b (b.name)}
            <div class="item backup" class:fresh={fresh === b.name}>
              <span class="ic"><Glyph name={b.failed.length ? "alert" : "zip"} size={24} /></span>
              <div class="iname">
                <b>{dateOf(b.created)}</b>
                <span class="label">{[whenOf(b.created), timeOf(b.created)].filter(Boolean).join(", ")} · {plural(b.items.length, "folder")}{b.items.length ? `: ${b.items.map((i) => i.name).join(", ")}` : ""}</span>
                {#each b.failed as f, i (i)}<span class="warn">{f}</span>{/each}
              </div>
              <span class="size">{bytes(b.size)}</span>
              <button class="btn icon" data-nav onclick={() => api.openFolder(b.path)} aria-label="Open the backup from {dateOf(b.created)}" title="Open folder"><Glyph name="folder" size={24} /></button>
            </div>
          {:else}
            <div class="item none"><span class="ic"><Glyph name="zip" size={24} /></span><div class="iname"><b>No backups yet</b><span class="label">Your first one shows up here.</span></div></div>
          {/each}
        </div>
      </div>
    </section>

    <section>
      <div class="side">
        <h2><Glyph name="globe" size={24} />GitHub</h2>
        <p>Installs, updates and Find ask GitHub. Without a token it answers 60 times an hour; with one, 5,000.</p>
      </div>
      <div class="body">
        {#if app.boot?.githubToken}
          <div class="list">
            <div class="item"><span class="ic"><Glyph name="check" size={24} /></span><div class="iname"><b>Token saved</b><span class="label">Stored on this PC only</span></div>
              <button class="btn" data-nav onclick={() => saveToken(null)} disabled={tokenSaving}><Glyph name="close" size={24} />Remove</button>
            </div>
          </div>
        {:else}
          <div class="acts">
            <input class="token" data-nav type="password" bind:value={token} placeholder="Paste a GitHub token" spellcheck="false" autocomplete="off" aria-label="GitHub token"
              onkeydown={(e) => e.key === "Enter" && token.trim() && saveToken(token)} />
            <button class="btn" data-nav onclick={() => saveToken(token)} disabled={tokenSaving || !token.trim()}><Glyph name="check" size={24} />{tokenSaving ? "Checking…" : "Save"}</button>
            <button class="btn" data-nav onclick={() => openUrl("https://github.com/settings/personal-access-tokens/new")}><Glyph name="globe" size={24} />Make one</button>
          </div>
          <p class="hint">A fine-grained token with no permissions is enough: OnionRing only reads public projects.</p>
        {/if}
      </div>
    </section>

    <section>
      <div class="side">
        <h2><Glyph name="refresh" size={24} />OnionRing</h2>
        <p>Version {app.boot?.version ?? ""}</p>
      </div>
      <div class="body">
        <div class="acts">
          {#if update}
            <button class="btn primary" data-nav disabled={updateState === "installing"} onclick={installUpdate}>
              <Glyph name="get" size={24} />{updateState === "installing" ? "Installing…" : `Install ${update.version}`}
            </button>
          {:else}
            <button class="btn" data-nav onclick={checkUpdate} disabled={updateState === "checking"}><Glyph name="refresh" size={24} />{updateState === "checking" ? "Checking…" : "Check for updates"}</button>
          {/if}
          {#if updateState === "none"}<span class="note"><Glyph name="check" size={24} />Up to date</span>{/if}
          {#if updateState === "error"}<span class="note"><Glyph name="alert" size={24} />Can't reach the update server</span>{/if}
          {#if updateState === "failed"}<span class="note"><Glyph name="alert" size={24} />The update didn't download. Try again in a moment.</span>{/if}
        </div>
      </div>
    </section>

    <section class="brand">
      <div class="side">
        <h2><Glyph name="star" size={24} />The ring</h2>
        <p>The logo moves with what the app is doing. Try each state.</p>
      </div>
      <div class="body stage">
        <div class="hero"><Logo size={200} variant="pixel" state={logoState} {progress} /></div>
        <div class="right">
          <div class="states">
            {#each [["idle", "Idle"], ["busy", "Working"], ["launch", "Launch"], ["done", "Done"]] as [s, label] (s)}
              <button class="btn" class:on={logoState === s} data-nav onclick={() => showState(s as LS)}>{label}</button>
            {/each}
          </div>
          <div class="variants">
            <div class="v" data-theme="pixel"><Logo size={80} variant="pixel" state={logoState} {progress} /><span class="label">Pixel</span></div>
            <div class="v" data-theme="paper"><Logo size={80} variant="ink" state={logoState} {progress} /><span class="label">Paper</span></div>
            <div class="v sizes">
              <div class="row">{#each [16, 24, 32, 48] as s (s)}<Logo size={s} variant="pixel" state="still" />{/each}</div>
              <span class="label">Small sizes</span>
            </div>
          </div>
        </div>
      </div>
    </section>
  </div>
</div>

<style>
  .settings {
    height: 100%;
    overflow-y: auto;
    padding: 16px 56px 64px;
  }
  .sheet {
    max-width: 1240px;
  }
  section {
    display: grid;
    grid-template-columns: 260px 1fr;
    gap: 48px;
    padding: 32px 0;
  }
  section + section {
    border-top: 1px solid color-mix(in srgb, var(--line) 22%, transparent);
  }
  :global(:root[data-theme="pixel"]) section + section {
    border-top: 3px dotted color-mix(in srgb, var(--line) 45%, transparent);
  }
  .side h2 {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 0;
    font-family: var(--title);
    font-size: 24px;
    font-weight: 700;
    line-height: 1.2;
  }
  .side p {
    margin: 12px 0 0 36px;
    color: var(--muted);
    font-size: 15px;
    line-height: 1.45;
  }
  .body {
    min-width: 0;
  }

  /* ---- Look ---- */
  .themes {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 24px;
    max-width: 760px;
  }
  .theme {
    display: flex;
    flex-direction: column;
    gap: 12px;
    text-align: left;
  }
  .preview {
    height: 168px;
    padding: 20px;
    display: grid;
    grid-template-columns: auto 1fr;
    grid-template-rows: 1fr auto;
    gap: 12px 16px;
    align-items: center;
    background: var(--bg);
    color: var(--fg);
    border: 2px solid var(--line);
    transition:
      transform 0.15s var(--ease),
      box-shadow 0.15s var(--ease);
  }
  .aa {
    font-size: 48px;
    font-weight: 800;
    line-height: 1;
  }
  .sample {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .mini {
    font-size: 14px;
    font-weight: 700;
  }
  .mini-tiles {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
  }
  .mini-tiles span {
    height: 32px;
    border: 2px solid var(--line);
  }
  .mini-tiles span:first-child {
    background: var(--fg);
  }
  [data-theme="paper"] .mini-tiles span:nth-child(2) {
    background: var(--accent);
    border-color: var(--accent);
  }
  [data-theme="paper"] .mini-tiles span:nth-child(3) {
    background: var(--accent-2);
    border-color: var(--accent-2);
  }
  .cap {
    display: grid;
    grid-template-columns: 28px 1fr;
    column-gap: 10px;
    align-items: center;
  }
  .radio {
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border: 2px solid currentColor;
  }
  .theme.on .radio {
    background: var(--fg);
    color: var(--bg);
    border-color: var(--fg);
  }
  .cname {
    font-family: var(--title);
    font-weight: 700;
    font-size: 18px;
  }
  .cblurb {
    grid-column: 2;
    color: var(--muted);
    font-size: 14px;
  }
  :global(:root[data-theme="pixel"]) .theme:focus-visible {
    background: none;
    color: var(--fg);
  }
  :global(:root[data-theme="pixel"]) .theme.on {
    background: none;
    color: var(--fg);
  }
  :global(:root[data-theme="pixel"]) .theme.on .preview,
  :global(:root[data-theme="pixel"]) .theme:hover .preview,
  :global(:root[data-theme="pixel"]) .theme:focus-visible .preview {
    outline: 3px solid var(--fg);
    outline-offset: 5px;
  }
  :global(:root[data-theme="pixel"]) .theme:focus-visible .preview {
    outline-style: dashed;
  }
  :global(:root[data-theme="paper"]) .theme.on .preview {
    box-shadow: 6px 6px 0 var(--fg);
  }
  :global(:root[data-theme="paper"]) .theme:not(.on):hover .preview {
    transform: translate(-2px, -2px);
    box-shadow: 4px 4px 0 var(--faint);
  }
  :global(:root[data-theme="paper"]) .theme.on .radio {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--panel);
  }

  /* ---- Lists of folders and backups ---- */
  .list {
    display: flex;
    flex-direction: column;
    max-width: 760px;
  }
  .item {
    display: grid;
    grid-template-columns: 40px 1fr auto auto;
    align-items: center;
    gap: 12px;
    padding: 12px 0;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 16%, transparent);
  }
  .item:first-child {
    border-top: 1px solid color-mix(in srgb, var(--line) 16%, transparent);
  }
  .item.backup {
    grid-template-columns: 40px 1fr auto 48px;
  }
  .item.gone {
    color: var(--faint);
  }
  .item.none {
    color: var(--faint);
  }
  .ic {
    width: 40px;
    height: 40px;
    display: grid;
    place-items: center;
  }
  :global(:root[data-theme="pixel"]) .ic {
    border: 3px solid currentColor;
  }
  :global(:root[data-theme="paper"]) .ic {
    background: var(--panel);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--line) 30%, transparent);
    border-radius: 50%;
  }
  .iname {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .iname b {
    font-family: var(--title);
    font-size: 19px;
    line-height: 1.2;
  }
  .iname .label {
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .warn {
    color: var(--accent);
    font-size: 14px;
  }
  :global(:root[data-theme="pixel"]) .warn::before {
    content: "! ";
  }
  .size {
    font-family: var(--title);
    font-size: 18px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    padding: 0 12px;
  }
  .fresh {
    animation: arrive 0.9s var(--ease) both;
  }
  @keyframes arrive {
    0% {
      background: var(--fg);
      color: var(--bg);
    }
  }
  :global(:root[data-theme="pixel"]) .fresh {
    animation: arrive 0.6s steps(3) both;
  }

  .token {
    flex: 1;
    min-width: 200px;
    max-width: 440px;
    height: 48px;
    padding: 0 14px;
    font-size: 16px;
    background: var(--bg);
    border: var(--border) solid var(--line);
  }
  :global(:root[data-theme="paper"]) .token {
    border-width: 1.5px;
    border-radius: var(--radius);
  }
  .hint {
    margin-top: 12px;
    font-size: 14px;
    color: var(--muted);
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
    margin-top: 20px;
  }
  .acts.top {
    margin: 0 0 20px;
  }
  .note,
  .hint {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--muted);
  }

  /* ---- Auto-pickup switch ---- */
  .switchrow {
    display: grid;
    grid-template-columns: 1fr auto auto;
    align-items: center;
    gap: 16px;
    width: 100%;
    max-width: 760px;
    margin-top: 24px;
    padding: 16px 20px;
    text-align: left;
  }
  :global(:root[data-theme="pixel"]) .switchrow {
    border: 3px solid currentColor;
  }
  :global(:root[data-theme="paper"]) .switchrow {
    background: var(--panel);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--line) 25%, transparent);
    border-radius: var(--radius);
  }
  .stext {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .stext b {
    font-family: var(--title);
    font-size: 19px;
  }
  .stext span {
    color: var(--muted);
    font-size: 15px;
  }
  .state {
    width: 32px;
    text-align: right;
  }
  .track {
    position: relative;
    width: 60px;
    height: 32px;
    border: 3px solid currentColor;
  }
  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 20px;
    height: 20px;
    background: currentColor;
    transition: transform 0.25s var(--ease);
  }
  .track.on .knob {
    transform: translateX(28px);
  }
  :global(:root[data-theme="pixel"]) .knob {
    transition-timing-function: steps(3);
  }
  :global(:root[data-theme="pixel"]) .track:not(.on) .knob {
    background: repeating-linear-gradient(45deg, currentColor 0 2px, transparent 2px 5px);
    box-shadow: inset 0 0 0 3px currentColor;
  }
  :global(:root[data-theme="pixel"]) .switchrow:hover,
  :global(:root[data-theme="pixel"]) .switchrow:focus-visible {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .switchrow:hover .stext span,
  :global(:root[data-theme="pixel"]) .switchrow:focus-visible .stext span {
    color: var(--bg);
  }
  :global(:root[data-theme="paper"]) .track {
    border-width: 1.5px;
    border-radius: 16px;
    border-color: var(--line);
    background: var(--bg);
    transition: background 0.25s;
  }
  :global(:root[data-theme="paper"]) .knob {
    top: 4px;
    left: 4px;
    border-radius: 50%;
    background: var(--fg);
    width: 21px;
    height: 21px;
  }
  :global(:root[data-theme="paper"]) .track.on {
    background: var(--accent-2);
    border-color: var(--accent-2);
  }
  :global(:root[data-theme="paper"]) .track.on .knob {
    background: var(--panel);
  }
  :global(:root[data-theme="paper"]) .switchrow:hover {
    box-shadow:
      inset 0 0 0 1.5px var(--line),
      3px 3px 0 var(--fg);
  }

  /* ---- Saves ---- */
  .covers {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-bottom: 20px;
  }
  .covers .label {
    color: var(--faint);
    margin-right: 4px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    height: 30px;
    padding: 0 12px;
    font-family: var(--label);
    font-size: 13px;
    font-weight: 700;
  }
  :global(:root[data-theme="pixel"]) .chip {
    border: 2px solid var(--line);
    text-transform: uppercase;
    font-size: 12px;
  }
  :global(:root[data-theme="paper"]) .chip {
    background: var(--panel);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--line) 30%, transparent);
    border-radius: 15px;
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 760px;
    margin-bottom: 20px;
  }
  .ptrack {
    display: flex;
    gap: 4px;
    height: 16px;
  }
  .ptrack span {
    flex: 1;
    border: 2px solid var(--line);
  }
  .ptrack span.done {
    background: var(--fg);
  }
  :global(:root[data-theme="pixel"]) .ptrack span.now {
    animation: blink 0.5s steps(1) infinite;
    background: var(--fg);
  }
  :global(:root[data-theme="paper"]) .ptrack {
    height: 6px;
    gap: 3px;
  }
  :global(:root[data-theme="paper"]) .ptrack span {
    border: 0;
    background: color-mix(in srgb, var(--line) 15%, transparent);
  }
  :global(:root[data-theme="paper"]) .ptrack span.done {
    background: var(--accent-2);
  }
  :global(:root[data-theme="paper"]) .ptrack span.now {
    background: linear-gradient(90deg, var(--accent-2) 0 50%, transparent 0) 0 0 / 200% 100%;
    animation: fill 1.2s var(--ease) infinite;
  }
  @keyframes fill {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: 0 0;
    }
  }
  .progress .label {
    color: var(--muted);
  }

  /* ---- The ring ---- */
  .stage {
    display: grid;
    grid-template-columns: 260px 1fr;
    gap: 24px 32px;
    align-items: start;
  }
  .hero {
    display: grid;
    place-items: center;
    height: 260px;
    background: #17110c;
  }
  :global(:root[data-theme="pixel"]) .hero {
    border: var(--border) solid var(--line);
  }
  .right {
    display: flex;
    flex-direction: column;
    gap: 24px;
    min-width: 0;
  }
  .states {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }
  .variants {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
  }
  .v {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    width: 140px;
    height: 160px;
    padding: 16px;
    background: var(--bg);
    color: var(--fg);
    border: 2px solid var(--line);
  }
  .v.sizes {
    width: 200px;
    background: #17110c;
    color: #f6ecd9;
    border-color: #17110c;
  }
  .v.sizes .row {
    flex: 1;
    display: flex;
    align-items: flex-end;
    gap: 12px;
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
    section {
      grid-template-columns: 200px 1fr;
      gap: 32px;
    }
    .side p {
      margin-left: 0;
    }
    .stage {
      grid-template-columns: 1fr;
    }
    .hero {
      height: 240px;
    }
  }
</style>
