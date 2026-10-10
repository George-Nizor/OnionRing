<script lang="ts">
  import { onMount } from "svelte";
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type Source } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import Icon from "./Icon.svelte";

  let sources = $state<Source[]>([]);
  let update = $state<Update | null>(null);
  let updateState = $state<"idle" | "checking" | "none" | "error" | "installing">("idle");
  let updateError = $state("");

  async function load() {
    sources = await api.sources();
  }
  onMount(load);

  async function addFolder() {
    const dir = await open({ directory: true, multiple: false, title: "Choose a folder with emulators or games" });
    if (typeof dir !== "string") return;
    await api.addSource(dir);
    await load();
    app.scan();
  }

  async function remove(s: Source) {
    const ok = await ask(`Stop scanning ${s.path}?\n\nThe folder itself is not touched.`, { title: "Remove folder", kind: "info" });
    if (!ok) return;
    await api.removeSource(s.id);
    await load();
    app.scan();
  }

  async function changeRoot() {
    const dir = await open({ directory: true, multiple: false, title: "Choose where OnionRing keeps its library" });
    if (typeof dir !== "string") return;
    const ok = await ask(
      `Use ${dir} as the data folder?\n\nIf it already holds an OnionRing library it is opened as is; otherwise a fresh library is started there. Your current folder is left untouched.`,
      { title: "Change data folder", kind: "info" },
    );
    if (!ok) return;
    await api.setDataRoot(dir);
    app.boot = await api.bootstrap();
    await load();
    await app.reload();
  }

  async function checkUpdate() {
    updateState = "checking";
    try {
      update = await check();
      updateState = update ? "idle" : "none";
    } catch (e) {
      updateError = String(e);
      updateState = "error";
    }
  }
  async function installUpdate() {
    if (!update) return;
    updateState = "installing";
    await update.downloadAndInstall();
    await relaunch();
  }
</script>

<div class="page">
  <div class="eyebrow">Settings</div>
  <h1>Settings</h1>
  <p class="lede">OnionRing itself stays small. Everything it gathers lives in your data folder, so you decide which drive it uses.</p>

  <section class="card block">
    <div class="title"><Icon name="disk" size={18} /> Data folder</div>
    <p class="desc">Library database, artwork, installed ports and emulators, and downloads.</p>
    <div class="line">
      <span class="mono path">{app.boot?.dataRoot}</span>
      <button class="btn sm" onclick={() => app.boot?.dataRoot && api.reveal(app.boot.dataRoot + "\\library.db")}><Icon name="folder" size={14} /> Open</button>
      <button class="btn sm" onclick={changeRoot}>Change…</button>
    </div>
  </section>

  <section class="card block">
    <div class="title"><Icon name="library" size={18} /> Game folders</div>
    <p class="desc">Folders OnionRing reads emulators and games from. They are scanned in place and never modified.</p>
    {#each sources as s (s.id)}
      <div class="line">
        <span class="mono path" class:gone={!s.exists}>{s.path}</span>
        {#if !s.exists}<span class="chip warn">Offline</span>{/if}
        <button class="btn sm ghost" onclick={() => api.reveal(s.path)}><Icon name="folder" size={14} /></button>
        <button class="btn sm ghost" onclick={() => remove(s)}><Icon name="close" size={14} /></button>
      </div>
    {/each}
    <div class="row">
      <button class="btn sm" onclick={addFolder}><Icon name="plus" size={14} /> Add folder</button>
      <button class="btn sm ghost" onclick={() => app.scan()} disabled={app.progress !== null}><Icon name="refresh" size={14} /> Rescan now</button>
    </div>
  </section>

  <section class="card block">
    <div class="title"><Icon name="image" size={18} /> Artwork</div>
    <p class="desc">Box art comes from the libretro thumbnail project, matched by file name. No account or API key needed. Custom covers you pick are kept.</p>
    <div class="row">
      <button class="btn sm" onclick={() => api.refreshArt(false).then((n) => app.toast(`Found art for ${n} games`, "ok"))}>Fetch missing art</button>
      <button class="btn sm ghost" onclick={() => api.refreshArt(true).then((n) => app.toast(`Refreshed art for ${n} games`, "ok"))}>Refresh all</button>
    </div>
  </section>

  <section class="card block">
    <div class="title"><span class="logo-dot"></span> OnionRing {app.boot?.version}</div>
    <p class="desc">Updates are signed, downloaded from GitHub Releases and installed in the background.</p>
    <div class="row">
      {#if update}
        <button class="btn sm primary" onclick={installUpdate} disabled={updateState === "installing"}>
          <Icon name="download" size={14} /> {updateState === "installing" ? "Installing…" : `Install ${update.version} and restart`}
        </button>
      {:else}
        <button class="btn sm" onclick={checkUpdate} disabled={updateState === "checking"}>
          {updateState === "checking" ? "Checking…" : "Check for updates"}
        </button>
      {/if}
      {#if updateState === "none"}<span class="ok"><Icon name="check" size={14} /> You're on the latest version</span>{/if}
      {#if updateState === "error"}<span class="err" title={updateError}>Couldn't reach the update server</span>{/if}
      <button class="btn sm ghost" onclick={() => openUrl("https://github.com/George-Nizor/OnionRing")}><Icon name="external" size={14} /> GitHub</button>
    </div>
  </section>

  <p class="keys">
    <b>Keyboard:</b> arrows to move, Enter to play, type or Ctrl K to search. <b>Controller:</b> D-pad or stick to move,
    A to play, Y to favourite, LB/RB to switch platform.
  </p>
</div>

<style>
  .block {
    padding: 18px 20px;
    border-radius: 16px;
    margin-bottom: 14px;
    max-width: 900px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 650;
    font-size: 15px;
  }
  .desc {
    margin: 4px 0 12px;
    color: var(--muted);
    font-size: 13px;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 0;
  }
  .path {
    flex: 1;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .path.gone {
    color: var(--faint);
  }
  .chip.warn {
    color: var(--warn);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
  }
  .ok {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--ok);
    font-size: 12.5px;
  }
  .err {
    color: var(--bad);
    font-size: 12.5px;
  }
  .logo-dot {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 3px solid transparent;
    background:
      linear-gradient(var(--bg-2), var(--bg-2)) padding-box,
      var(--ring-linear) border-box;
  }
  .keys {
    color: var(--faint);
    font-size: 12.5px;
    max-width: 900px;
  }
  .keys b {
    color: var(--muted);
  }
</style>
