<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type Copy, type Emulator, type PortDef } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { ago, bytes, playtime } from "$lib/format";
  import Art from "./Art.svelte";
  import Glyph from "./Glyph.svelte";

  let { emulators }: { emulators: Emulator[] } = $props();

  const g = $derived(app.current);
  const platform = $derived(g ? app.platforms.get(g.platform) : undefined);
  const emu = $derived(emulators.find((e) => e.id === g?.emulator));
  const choices = $derived(emulators.filter((e) => g && e.platforms.includes(g.platform)));
  const running = $derived(g ? app.running.includes(g.id) : false);

  let copies = $state<Copy[]>([]);
  let port = $state<PortDef | null>(null);
  let more = $state(false);
  $effect(() => {
    const game = g;
    if (!game) return;
    api.copies(game.key).then((c) => (copies = c)).catch(() => (copies = []));
    if (game.appId) api.catalog().then((c) => (port = c.ports.find((p) => p.id === game.appId) ?? null)).catch(() => {});
  });
  const maxCopy = $derived(Math.max(1, ...copies.map((c) => c.size)));

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
</script>

{#if g}
  <div class="page">
    <div class="banner"><Art game={g} kind="wide" /></div>
    <div class="body">
      <div class="cover"><Art game={g} /></div>
      <div class="main">
        <span class="plat label">{g.kind === "port" ? "PC PORT" : (platform?.name ?? "")}</span>
        <h1>{g.title}</h1>
        <div class="stats">
          <span class="stat"><Glyph name="clock" size={24} label="Played" /><b>{g.playtime ? playtime(g.playtime) : "0h"}</b><small>{g.lastPlayed ? ago(g.lastPlayed) : "never"}</small></span>
          <span class="stat"><Glyph name="disk" size={24} label="Size" /><b>{bytes(g.size)}</b></span>
          {#if g.kind === "rom"}<span class="stat"><Glyph name="pad" size={24} label="Emulator" /><b>{emu ? emu.name : "None"}</b><small>{emu?.version ?? ""}</small></span>{/if}
        </div>

        <div class="actions">
          <button class="btn big primary" data-nav onclick={() => app.play(g)} disabled={g.missing || running}>
            <Glyph name={g.packed ? "zip" : "play"} size={24} />{running ? "Playing" : g.packed ? "Unzip" : "Play"}
          </button>
          {#if g.kind === "port"}
            <button class="btn big" data-nav onclick={() => api.configurePort(g.id).catch((e) => app.toast(String(e), "error"))}>
              <Glyph name="sliders" size={24} />Port settings
            </button>
          {/if}
          <button class="btn big icon" class:on={g.favorite} data-nav onclick={() => app.toggleFavorite(g)} aria-label="Favourite"><Glyph name="star" size={24} /></button>
          <button class="btn big icon" class:on={more} data-nav onclick={() => (more = !more)} aria-label="More"><Glyph name="options" size={24} /></button>
        </div>

        {#if more}
          <div class="more">
            {#each choices.length > 1 ? choices : [] as c (c.id)}
              <button class="btn" class:on={c.id === g.emulator} data-nav onclick={() => setEmu(c.id)}><Glyph name={c.id === g.emulator ? "check" : "pad"} size={24} />{c.name}</button>
            {/each}
            <button class="btn" data-nav onclick={pickCover}><Glyph name="library" size={24} />Cover</button>
            <button class="btn" data-nav onclick={() => api.reveal(g.launchPath)}><Glyph name="folder" size={24} />Folder</button>
            <button class="btn" data-nav onclick={hide}><Glyph name="eye" size={24} />{g.hidden ? "Show" : "Hide"}</button>
          </div>
        {/if}

        {#if port?.requires}
          <p class="note"><Glyph name="alert" size={24} /><span>{port.requires}</span></p>
        {:else if g.kind === "rom" && !emu}
          <p class="note"><Glyph name="alert" size={24} /><span>No emulator for {platform?.name} yet.</span>
            <button class="btn" data-nav onclick={() => app.go({ name: "get", tab: "emulators" })}><Glyph name="get" size={24} />Get one</button></p>
        {/if}

        {#if copies.length > 1}
          <div class="disk">
            <div class="row-head"><Glyph name="disk" size={24} /><span class="label">On disk</span></div>
            <div class="copies">
              {#each copies as c, i (c.path)}
                <div class="copy" class:keep={c.primary} style:--w="{Math.max(8, (c.size / maxCopy) * 100)}%" style:--i={i}>
                  <span class="bar"></span>
                  <span class="cap label"><Glyph name={c.archive ? "zip" : c.primary ? "check" : "disk"} size={12} />{bytes(c.size)}</span>
                </div>
              {/each}
            </div>
            <button class="btn" data-nav onclick={() => app.go({ name: "tidy" })}><Glyph name="broom" size={24} />Tidy up</button>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .page {
    height: 100%;
    overflow-y: auto;
    padding-bottom: 50px;
  }
  .banner {
    position: relative;
    height: 34vh;
    min-height: 220px;
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
    border-bottom: 2px solid var(--line);
  }
  :global(:root[data-theme="paper"]) .banner :global(.art) {
    filter: var(--img-filter-focus);
  }
  .body {
    display: flex;
    gap: 44px;
    padding: 0 56px;
  }
  .cover {
    position: relative;
    flex: none;
    width: 230px;
    aspect-ratio: 3 / 4;
    margin-top: -120px;
    animation: rise 0.5s var(--ease) both 0.1s;
  }
  :global(:root[data-theme="pixel"]) .cover {
    outline: 4px solid var(--line);
    background: var(--bg);
    animation-timing-function: steps(4);
  }
  :global(:root[data-theme="paper"]) .cover {
    box-shadow:
      0 0 0 2px var(--line),
      8px 8px 0 var(--line);
  }
  .main {
    flex: 1;
    min-width: 0;
    padding-top: 26px;
  }
  .plat {
    font-size: 14px;
  }
  :global(:root[data-theme="paper"]) .plat {
    color: var(--accent);
  }
  h1 {
    margin: 8px 0 0;
    font-family: var(--title);
    font-weight: 800;
    font-size: clamp(38px, 4vw, 64px);
    line-height: 1;
    animation: rise 0.45s var(--ease) both;
  }
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 14px 34px;
    margin: 22px 0 26px;
  }
  .stat {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: var(--label);
    font-size: 17px;
  }
  .stat small {
    color: var(--faint);
    font-size: 13px;
  }
  .actions,
  .more {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }
  .more {
    margin-top: 16px;
    animation: rise 0.25s var(--ease) both;
  }
  :global(:root[data-theme="pixel"]) .btn.on {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="paper"]) .btn.on {
    color: var(--accent);
    border-color: var(--accent);
  }
  .note {
    display: flex;
    align-items: center;
    gap: 14px;
    margin: 24px 0 0;
    max-width: 760px;
    font-family: var(--label);
    font-size: 15px;
  }
  :global(:root[data-theme="paper"]) .note {
    color: var(--accent);
  }
  .disk {
    margin-top: 34px;
    max-width: 640px;
  }
  .copies {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 16px;
  }
  .copy {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .bar {
    width: var(--w);
    height: 22px;
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
</style>
