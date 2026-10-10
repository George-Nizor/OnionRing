<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { app, type ViewName } from "$lib/store.svelte";
  import Glyph from "./Glyph.svelte";
  import Logo from "./Logo.svelte";

  const NAMES: Record<ViewName, [string, string]> = {
    home: ["home", "Home"],
    library: ["library", "Library"],
    game: ["play", "Game"],
    get: ["get", "Get more"],
    systems: ["pad", "Systems"],
    tidy: ["broom", "Tidy up"],
    settings: ["gear", "Settings"],
  };
  const here = $derived(NAMES[app.view.name]);

  let now = $state(new Date());
  $effect(() => {
    const t = setInterval(() => (now = new Date()), 15000);
    return () => clearInterval(t);
  });
  const clock = $derived(now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }));

  const busy = $derived(app.progress !== null || Object.keys(app.installing).length > 0);
  const pct = $derived(app.progress && app.progress.total ? app.progress.done / app.progress.total : 0.4);
  const busyText = $derived.by(() => {
    const inst = Object.values(app.installing)[0];
    if (inst) return inst;
    const p = app.progress;
    if (!p) return "";
    if (p.stage === "art") return `Art ${p.done}/${p.total}`;
    if (p.stage === "unpack") return "Unzipping";
    return "Scanning";
  });
  const win = () => getCurrentWindow();
</script>

<header class="bar" data-tauri-drag-region>
  <button class="menu" data-nav onclick={() => (app.menuOpen = !app.menuOpen)} aria-label="Menu">
    <Logo size={app.theme === "pixel" ? 48 : 40} state={busy ? "busy" : "still"} progress={pct} />
    <Glyph name="menu" size={24} />
  </button>
  {#key app.view.name}
    <div class="where" data-tauri-drag-region>
      <Glyph name={here[0]} size={24} />
      <span class="title">{here[1]}</span>
    </div>
  {/key}
  <div class="right" data-tauri-drag-region>
    {#if busy}
      <div class="busy label">{busyText}</div>
    {/if}
    <div class="clock label">{clock}</div>
    <div class="win">
      <button onclick={() => win().minimize()} aria-label="Minimise"><span class="w-min"></span></button>
      <button onclick={() => win().toggleMaximize()} aria-label="Maximise"><span class="w-max"></span></button>
      <button onclick={() => win().close()} aria-label="Close"><Glyph name="close" size={12} /></button>
    </div>
  </div>
</header>

<style>
  .bar {
    height: var(--bar);
    flex: none;
    display: flex;
    align-items: center;
    gap: 20px;
    padding-left: 20px;
  }
  :global(:root[data-theme="pixel"]) .bar {
    border-bottom: var(--border) solid var(--line);
  }
  :global(:root[data-theme="paper"]) .bar {
    border-bottom: 1.5px solid var(--line);
  }
  .menu {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 46px;
    padding: 0 12px 0 6px;
  }
  .where {
    display: flex;
    align-items: center;
    gap: 12px;
    animation: rise 0.35s var(--ease) both;
  }
  :global(:root[data-theme="pixel"]) .where {
    animation: type 0.3s steps(4) both;
  }
  @keyframes type {
    from {
      clip-path: inset(0 100% 0 0);
    }
  }
  .title {
    font-family: var(--title);
    font-size: 24px;
    font-weight: 700;
  }
  .right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 22px;
    height: 100%;
  }
  .busy {
    padding: 6px 10px;
    border: var(--border) solid var(--line);
  }
  .clock {
    font-size: 16px;
  }
  .win {
    display: flex;
    height: 100%;
  }
  .win button {
    width: 46px;
    height: 100%;
    display: grid;
    place-items: center;
    color: var(--faint);
  }
  .win button:hover {
    background: var(--hi-bg);
    color: var(--hi-fg);
  }
  .w-min {
    width: 12px;
    height: 2px;
    background: currentColor;
  }
  .w-max {
    width: 12px;
    height: 12px;
    border: 2px solid currentColor;
  }
</style>
