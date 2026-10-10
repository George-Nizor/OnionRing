<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import Icon from "./Icon.svelte";
  import Logo from "./Logo.svelte";

  let { ondone }: { ondone: () => void } = $props();

  let step = $state(0);
  let root = $state(app.boot?.suggestedRoot ?? "");
  let folders = $state<string[]>([...(app.boot?.suggestedSources ?? [])]);
  let chosen = $state<Set<string>>(new Set(app.boot?.suggestedSources ?? []));
  let error = $state("");
  let building = $state(false);

  async function browseRoot() {
    const d = await open({ directory: true, multiple: false, title: "Where should OnionRing keep its library?" });
    if (typeof d === "string") root = d.toLowerCase().endsWith("onionring") ? d : `${d.replace(/\\$/, "")}\\OnionRing`;
  }
  async function addFolder() {
    const d = await open({ directory: true, multiple: false, title: "Folder with emulators or games" });
    if (typeof d === "string" && !folders.includes(d)) {
      folders = [...folders, d];
      chosen = new Set([...chosen, d]);
    }
  }
  function toggle(f: string) {
    const n = new Set(chosen);
    if (n.has(f)) n.delete(f);
    else n.add(f);
    chosen = n;
  }

  async function next() {
    error = "";
    if (step === 1) {
      try {
        await api.setDataRoot(root);
      } catch (e) {
        error = String(e);
        return;
      }
    }
    if (step === 2) {
      building = true;
      for (const f of chosen) await api.addSource(f);
      app.boot = await api.bootstrap();
      await app.scan();
      building = false;
    }
    step++;
  }
</script>

<div class="setup">
  <div class="aurora"></div>
  <div class="panel">
    {#if step === 0}
      <div class="hero-logo"><Logo size={120} busy /></div>
      <h1 class="display">OnionRing</h1>
      <p class="sub">One launcher for your emulators, native ports, recomps and decomps. Built for the games the big stores don't sell.</p>
      <button class="btn primary big" onclick={() => step++}>Get started</button>
    {:else if step === 1}
      <div class="eyebrow">Step 1 of 2</div>
      <h2 class="display">Where should your library live?</h2>
      <p class="sub">
        Artwork, the library database, and any ports or emulators you install go here. Pick a roomy drive. The app itself
        stays tiny.
      </p>
      <div class="pick">
        <input class="field mono" bind:value={root} spellcheck="false" />
        <button class="btn" onclick={browseRoot}><Icon name="folder" size={16} /> Browse</button>
      </div>
      {#if error}<p class="err">{error}</p>{/if}
      <div class="nav">
        <button class="btn ghost" onclick={() => step--}>Back</button>
        <button class="btn primary" onclick={next} disabled={!root}>Continue</button>
      </div>
    {:else if step === 2}
      <div class="eyebrow">Step 2 of 2</div>
      <h2 class="display">Already have emulators set up?</h2>
      <p class="sub">
        Point OnionRing at them. It finds your emulators and games where they are and never moves or changes them. No
        setup yet? Skip this and install emulators and ports from inside the app.
      </p>
      <div class="folders">
        {#each folders as f (f)}
          <label class="folder" class:on={chosen.has(f)}>
            <input type="checkbox" checked={chosen.has(f)} onchange={() => toggle(f)} />
            <Icon name="folder" size={18} />
            <span class="mono">{f}</span>
            {#if app.boot?.suggestedSources.includes(f)}<span class="chip">Found</span>{/if}
          </label>
        {/each}
        <button class="add" onclick={addFolder}><Icon name="plus" size={16} /> Add a folder</button>
      </div>
      <div class="nav">
        <button class="btn ghost" onclick={() => step--} disabled={building}>Back</button>
        <button class="btn primary" onclick={next} disabled={building}>
          {#if building}<Logo size={18} busy /> Building your library…{:else}{chosen.size ? "Build my library" : "Skip for now"}{/if}
        </button>
      </div>
    {:else}
      <div class="hero-logo"><Logo size={90} /></div>
      <h2 class="display">You're all set</h2>
      <p class="sub">{app.games.length} games found. Artwork keeps loading in the background.</p>
      <button class="btn primary big" onclick={() => { app.section = "library"; ondone(); }}>Open my library</button>
    {/if}
  </div>
</div>

<style>
  .setup {
    position: fixed;
    inset: var(--titlebar) 0 0 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: var(--bg);
    overflow: hidden;
  }
  .aurora {
    position: absolute;
    inset: -20%;
    background:
      radial-gradient(40% 40% at 30% 30%, rgba(255, 181, 71, 0.18), transparent 70%),
      radial-gradient(40% 40% at 70% 40%, rgba(255, 93, 122, 0.16), transparent 70%),
      radial-gradient(50% 50% at 50% 80%, rgba(157, 107, 255, 0.2), transparent 70%);
    filter: blur(40px);
    animation: float 18s ease-in-out infinite alternate;
  }
  @keyframes float {
    to {
      transform: rotate(8deg) scale(1.1);
    }
  }
  .panel {
    position: relative;
    width: min(640px, 90vw);
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    animation: rise 0.6s var(--ease);
  }
  .hero-logo {
    margin-bottom: 10px;
    filter: drop-shadow(0 20px 60px rgba(255, 93, 122, 0.45));
  }
  h1 {
    margin: 0;
    font-size: 54px;
    font-weight: 700;
    letter-spacing: -0.03em;
  }
  h2 {
    margin: 0;
    font-size: 28px;
    font-weight: 650;
    letter-spacing: -0.02em;
  }
  .sub {
    margin: 0 0 10px;
    color: var(--muted);
    font-size: 15px;
    max-width: 520px;
  }
  .btn.big {
    height: 50px;
    padding: 0 30px;
    font-size: 15px;
    border-radius: 15px;
  }
  .pick {
    display: flex;
    gap: 8px;
    width: 100%;
  }
  .pick .field {
    flex: 1;
    height: 44px;
  }
  .pick .btn {
    height: 44px;
  }
  .err {
    color: var(--bad);
    margin: 0;
  }
  .nav {
    display: flex;
    justify-content: space-between;
    width: 100%;
    margin-top: 16px;
  }
  .folders {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    text-align: left;
  }
  .folder {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border-radius: 13px;
    background: var(--surface);
    border: 1px solid var(--line);
    cursor: pointer;
    color: var(--muted);
    transition: all 0.15s;
  }
  .folder.on {
    border-color: rgba(255, 93, 122, 0.45);
    background: rgba(255, 93, 122, 0.06);
    color: var(--text);
  }
  .folder input {
    accent-color: var(--coral);
  }
  .folder .mono {
    flex: 1;
    font-size: 13px;
  }
  .add {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: 46px;
    border-radius: 13px;
    border: 1px dashed var(--line-2);
    color: var(--muted);
  }
  .add:hover {
    color: var(--text);
    background: var(--surface);
  }
</style>
