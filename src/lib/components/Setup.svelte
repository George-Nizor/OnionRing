<script lang="ts">
  import { tick } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { focusFirst } from "$lib/nav";
  import Glyph from "./Glyph.svelte";
  import Logo from "./Logo.svelte";

  let { ondone }: { ondone: () => void } = $props();

  let step = $state(0);
  let root = $state(app.boot?.suggestedRoot ?? "");
  let folders = $state<string[]>([...(app.boot?.suggestedSources ?? [])]);
  let chosen = $state<Set<string>>(new Set(app.boot?.suggestedSources ?? []));
  let error = $state("");
  let building = $state(false);
  let panel = $state<HTMLElement>();

  $effect(() => {
    void step;
    tick().then(() => focusFirst(panel));
  });

  async function browseRoot() {
    const d = await open({ directory: true, multiple: false, title: "Where should OnionRing keep its files?" });
    if (typeof d === "string") root = d.toLowerCase().endsWith("onionring") ? d : `${d.replace(/\\$/, "")}\\OnionRing`;
  }
  async function addFolder() {
    const d = await open({ directory: true, multiple: false, title: "A folder with emulators or games" });
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

<div class="setup" data-layer bind:this={panel}>
  {#if step === 0}
    <div class="center">
      <Logo size={200} variant="color" state="idle" />
      <h1>OnionRing</h1>
      <p class="label">Your emulators and PC ports, in one place</p>
      <button class="btn big primary" data-nav onclick={() => step++}><Glyph name="play" size={24} />Start</button>
    </div>
  {:else if step === 1}
    <div class="center">
      <Glyph name="disk" size={96} />
      <h2>Where should OnionRing keep its files?</h2>
      <p class="label">Art, downloads and installed ports go here. Pick a big drive.</p>
      <div class="pick">
        <input class="field" data-nav bind:value={root} spellcheck="false" aria-label="Folder" />
        <button class="btn big icon" data-nav onclick={browseRoot} aria-label="Browse"><Glyph name="folder" size={24} /></button>
      </div>
      {#if error}<p class="err label">{error}</p>{/if}
      <button class="btn big primary" data-nav onclick={next} disabled={!root}><Glyph name="check" size={24} />Use this folder</button>
    </div>
  {:else if step === 2}
    <div class="center">
      <Glyph name="pad" size={96} />
      <h2>Got emulators already?</h2>
      <p class="label">Tick the folders they're in. They stay where they are.</p>
      <div class="folders">
        {#each folders as f (f)}
          <button class="folder" class:on={chosen.has(f)} data-nav onclick={() => toggle(f)}>
            <Glyph name={chosen.has(f) ? "check" : "folder"} size={36} /><span>{f}</span>
          </button>
        {/each}
        <button class="folder" data-nav onclick={addFolder}><Glyph name="plus" size={36} /><span>Add a folder</span></button>
      </div>
      <button class="btn big primary" data-nav onclick={next} disabled={building}>
        {#if building}<Logo size={28} variant="auto" state="busy" progress={0.5} />Looking…{:else}<Glyph name="play" size={24} />{chosen.size ? "Find my games" : "Skip"}{/if}
      </button>
    </div>
  {:else}
    <div class="center">
      <Logo size={160} variant="color" state="done" />
      <h2>{app.games.length} games found</h2>
      <button class="btn big primary" data-nav onclick={ondone}><Glyph name="home" size={24} />Let's go</button>
    </div>
  {/if}
</div>

<style>
  .setup {
    position: fixed;
    inset: 0;
    z-index: 70;
    display: grid;
    place-items: center;
    background: var(--bg);
  }
  .center {
    width: min(680px, 90vw);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 18px;
    text-align: center;
    animation: rise 0.5s var(--ease) both;
  }
  h1 {
    margin: 0;
    font-family: var(--title);
    font-weight: 800;
    font-size: 72px;
  }
  h2 {
    margin: 0;
    font-family: var(--title);
    font-weight: 800;
    font-size: 40px;
  }
  .pick {
    display: flex;
    gap: 10px;
    width: 100%;
  }
  .field {
    flex: 1;
    height: 60px;
    padding: 0 16px;
    font-size: 18px;
    background: var(--bg);
    border: var(--border) solid var(--line);
  }
  .err {
    color: var(--accent);
  }
  .folders {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
  }
  .folder {
    display: flex;
    align-items: center;
    gap: 16px;
    height: 64px;
    padding: 0 16px;
    font-family: var(--title);
    font-size: 20px;
    font-weight: 700;
    text-align: left;
    border: var(--border) solid var(--line);
  }
</style>
