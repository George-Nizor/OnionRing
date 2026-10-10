<script lang="ts">
  import { onMount } from "svelte";
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { api, type Source } from "$lib/api";
  import { app, type Theme } from "$lib/store.svelte";
  import Glyph from "./Glyph.svelte";
  import Logo from "./Logo.svelte";

  let sources = $state<Source[]>([]);
  let update = $state<Update | null>(null);
  let updateState = $state<"idle" | "checking" | "none" | "error" | "installing">("idle");

  onMount(async () => {
    sources = await api.sources().catch(() => []);
  });

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
    if (!(await ask(`Stop reading ${s.path}?\n\nThe folder itself is not touched.`, { title: "Remove folder" }))) return;
    await api.removeSource(s.id);
    sources = await api.sources();
    app.scan();
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

  const themes: [Theme, string][] = [
    ["pixel", "Pixel"],
    ["paper", "Paper"],
  ];
</script>

<div class="settings">
  <section>
    <div class="row-head"><Glyph name="eye" size={24} /><span class="label">Look</span></div>
    <div class="themes">
      {#each themes as [t, name] (t)}
        <button class="theme" class:on={app.theme === t} data-nav onclick={() => app.setTheme(t)} aria-label="{name} theme">
          <div class="preview" data-theme={t}>
            <Logo size={64} variant={t === "pixel" ? "pixel" : "ink"} state="still" />
            <div class="sample">
              <span class="aa" style:font-family="var(--title)">Aa</span>
              <span class="mini" style:font-family="var(--label)">{name}</span>
            </div>
            <div class="mini-tiles"><span></span><span></span><span></span></div>
          </div>
          {#if app.theme === t}<span class="tick"><Glyph name="check" size={24} /></span>{/if}
        </button>
      {/each}
    </div>
  </section>

  <section>
    <div class="row-head"><Glyph name="folder" size={24} /><span class="label">Game folders</span></div>
    <div class="folders">
      {#each sources as s (s.id)}
        {@const f = folderName(s.path)}
        <div class="folder" class:gone={!s.exists}>
          <Glyph name={s.exists ? "folder" : "alert"} size={36} />
          <div class="fname"><b>{f.name}</b><span class="label">{f.drive}</span></div>
          <button class="btn icon" data-nav onclick={() => api.reveal(s.path)} aria-label="Open {f.name}"><Glyph name="eye" size={24} /></button>
          <button class="btn icon" data-nav onclick={() => remove(s)} aria-label="Remove {f.name}"><Glyph name="close" size={24} /></button>
        </div>
      {/each}
      <div class="acts">
        <button class="btn" data-nav onclick={addFolder}><Glyph name="plus" size={24} />Add folder</button>
        <button class="btn" data-nav onclick={() => app.scan()} disabled={app.progress !== null}><Glyph name="refresh" size={24} />Rescan</button>
        <button class="btn" data-nav onclick={() => api.refreshArt(true).then((n) => app.toast(`New art for ${n} games`, "ok"))}><Glyph name="library" size={24} />Refresh art</button>
      </div>
    </div>
  </section>

  <section>
    <div class="row-head"><Glyph name="refresh" size={24} /><span class="label">OnionRing {app.boot?.version}</span></div>
    <div class="acts">
      {#if update}
        <button class="btn primary" data-nav onclick={async () => { updateState = "installing"; await update?.downloadAndInstall(); await relaunch(); }}>
          <Glyph name="get" size={24} />Install {update.version}
        </button>
      {:else}
        <button class="btn" data-nav onclick={checkUpdate} disabled={updateState === "checking"}><Glyph name="refresh" size={24} />Check for updates</button>
      {/if}
      {#if updateState === "none"}<span class="label ok"><Glyph name="check" size={24} />Up to date</span>{/if}
      {#if updateState === "error"}<span class="label"><Glyph name="alert" size={24} />Update server not reachable</span>{/if}
    </div>
  </section>

  <section class="brand">
    <div class="row-head"><Glyph name="star" size={24} /><span class="label">The ring</span></div>
    <div class="stage">
      <div class="hero"><Logo size={220} variant="color" state={logoState} {progress} /></div>
      <div class="states">
        {#each [["idle", "Idle"], ["busy", "Working"], ["launch", "Launch"], ["done", "Done"]] as [s, label] (s)}
          <button class="btn" class:on={logoState === s} data-nav onclick={() => showState(s as LS)}>{label}</button>
        {/each}
      </div>
      <div class="variants">
        <div class="v" data-theme="pixel"><Logo size={96} variant="pixel" state={logoState} {progress} /><span class="label">Pixel</span></div>
        <div class="v" data-theme="paper"><Logo size={96} variant="ink" state={logoState} {progress} /><span class="label">Paper</span></div>
        <div class="v sizes">
          {#each [16, 24, 32, 48] as s (s)}<Logo size={s} variant="color" state="still" />{/each}
          <span class="label">Small sizes</span>
        </div>
      </div>
    </div>
  </section>
</div>

<style>
  .settings {
    height: 100%;
    overflow-y: auto;
    padding: 28px 56px 60px;
    display: flex;
    flex-direction: column;
    gap: 40px;
  }
  .themes {
    display: flex;
    gap: 22px;
  }
  .theme {
    position: relative;
    width: 300px;
    padding: 0;
  }
  .preview {
    height: 190px;
    padding: 22px;
    display: grid;
    grid-template-columns: auto 1fr;
    grid-template-rows: 1fr auto;
    gap: 14px;
    align-items: center;
    background: var(--bg);
    color: var(--fg);
    border: 2px solid var(--line);
  }
  .aa {
    font-size: 54px;
    font-weight: 800;
    line-height: 1;
  }
  .sample {
    display: flex;
    flex-direction: column;
  }
  .mini {
    font-size: 15px;
    font-weight: 700;
  }
  .mini-tiles {
    grid-column: 1 / -1;
    display: flex;
    gap: 8px;
  }
  .mini-tiles span {
    flex: 1;
    height: 34px;
    border: 2px solid var(--line);
  }
  .mini-tiles span:first-child {
    background: var(--fg);
  }
  [data-theme="paper"] .mini-tiles span:nth-child(2) {
    background: var(--accent);
    border-color: var(--accent);
  }
  .tick {
    position: absolute;
    top: 10px;
    right: 10px;
    background: var(--fg);
    color: var(--bg);
    padding: 4px;
  }
  :global(:root[data-theme="pixel"]) .theme:focus-visible,
  :global(:root[data-theme="pixel"]) .theme.on {
    background: none;
    outline: 4px solid var(--fg);
    outline-offset: 6px;
  }
  :global(:root[data-theme="paper"]) .theme.on .preview {
    box-shadow: 6px 6px 0 var(--fg);
  }
  .folders {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 760px;
  }
  .folder {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .folder.gone {
    opacity: 0.5;
  }
  .fname {
    flex: 1;
    display: flex;
    flex-direction: column;
  }
  .fname b {
    font-family: var(--title);
    font-size: 22px;
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
    margin-top: 6px;
  }
  .acts .label {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  :global(:root[data-theme="pixel"]) .btn.on {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="paper"]) .btn.on {
    color: var(--accent);
    border-color: var(--accent);
  }
  .stage {
    display: grid;
    grid-template-columns: 300px 1fr;
    gap: 20px 40px;
    align-items: center;
  }
  .hero {
    grid-row: span 2;
    display: grid;
    place-items: center;
    height: 300px;
    background: #17110c;
  }
  :global(:root[data-theme="pixel"]) .hero {
    border: var(--border) solid var(--line);
  }
  .states {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }
  .variants {
    display: flex;
    gap: 20px;
  }
  .v {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 18px;
    background: var(--bg);
    color: var(--fg);
    border: 2px solid var(--line);
  }
  .v.sizes {
    flex-direction: row;
    align-items: flex-end;
    flex-wrap: wrap;
    max-width: 240px;
    background: #17110c;
    color: #f6ecd9;
  }
  .v.sizes .label {
    width: 100%;
  }
</style>
