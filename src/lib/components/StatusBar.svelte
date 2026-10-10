<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { app, type ViewName } from "$lib/store.svelte";
  import Glyph from "./Glyph.svelte";
  import Logo from "./Logo.svelte";
  import Search from "./Search.svelte";

  const NAMES: Record<ViewName, [string, string]> = {
    home: ["home", "Home"],
    library: ["library", "Library"],
    game: ["play", "Game"],
    get: ["get", "Get more"],
    systems: ["pad", "Systems"],
    tidy: ["broom", "Tidy up"],
    settings: ["gear", "Settings"],
    stats: ["chart", "Play history"],
  };
  const here = $derived(NAMES[app.view.name]);
  const canBack = $derived(app.view.name !== "home" && app.history.length > 0);

  let now = $state(new Date());
  $effect(() => {
    const t = setInterval(() => (now = new Date()), 15000);
    return () => clearInterval(t);
  });
  const clock = $derived(now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }));

  const busy = $derived(app.progress !== null || Object.keys(app.installing).length > 0);
  const pct = $derived(app.progress && app.progress.total ? app.progress.done / app.progress.total : 0.4);
  const exact = $derived(!!app.progress?.total);
  const busyText = $derived.by(() => {
    const inst = Object.values(app.installing)[0];
    if (inst) return inst;
    const p = app.progress;
    if (!p) return "";
    if (p.stage === "art") return `Art ${p.done} of ${p.total}`;
    if (p.stage === "unpack") return "Unzipping";
    if (p.stage === "backup") return "Backing up saves";
    return "Looking for games";
  });
  const win = () => getCurrentWindow();
</script>

<header class="bar" data-tauri-drag-region>
  <div class="left" data-tauri-drag-region>
    <button class="menu" class:open={app.menuOpen} data-nav onclick={() => (app.menuOpen = !app.menuOpen)} aria-label="Menu" aria-expanded={app.menuOpen}>
      <Logo size={app.theme === "pixel" ? 48 : 40} state={busy ? "busy" : "still"} progress={pct} />
      <Glyph name="menu" size={24} />
    </button>
    <span class="rule" aria-hidden="true"></span>
    {#if canBack}
      <button class="back" onclick={() => app.back()} aria-label="Back"><Glyph name="back" size={24} /></button>
    {/if}
    {#key app.view.name}
      <div class="where" data-tauri-drag-region>
        <Glyph name={here[0]} size={24} />
        <span class="title">{here[1]}</span>
      </div>
    {/key}
  </div>

  <Search />

  <div class="right" data-tauri-drag-region>
    {#if busy}
      <div class="busy" title={busyText}>
        <span class="bt label">{busyText}</span>
        <span class="track"><span class="fill" class:sweep={!exact} style:--p={pct}></span></span>
      </div>
    {/if}
    <span class="clock label">{clock}</span>
    <div class="win">
      <button onclick={() => win().minimize()} aria-label="Minimise"><span class="w-min"></span></button>
      <button onclick={() => win().toggleMaximize()} aria-label="Maximise"><span class="w-max"></span></button>
      <button class="x" onclick={() => win().close()} aria-label="Close"><Glyph name="close" size={12} /></button>
    </div>
  </div>
</header>

<style>
  .bar {
    position: relative;
    z-index: 36;
    height: var(--bar);
    flex: none;
    display: grid;
    grid-template-columns: minmax(max-content, 1fr) minmax(220px, 560px) minmax(max-content, 1fr);
    align-items: center;
    gap: var(--s5);
    padding-left: var(--s3);
    background: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .bar {
    border-bottom: var(--border) solid var(--line);
  }
  :global(:root[data-theme="paper"]) .bar {
    border-bottom: 1.5px solid var(--line);
  }
  .left {
    display: flex;
    align-items: center;
    gap: var(--s3);
    height: 100%;
    min-width: 0;
  }
  .menu {
    display: flex;
    align-items: center;
    gap: var(--s2);
    height: 44px;
    padding: 0 var(--s3) 0 var(--s1);
  }
  .rule {
    width: 0;
    height: 24px;
    border-left: 1.5px solid var(--line);
    opacity: 0.35;
  }
  :global(:root[data-theme="pixel"]) .rule {
    border-left: 3px dotted var(--line);
    opacity: 1;
  }
  .back {
    width: 36px;
    height: 36px;
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  .where {
    display: flex;
    align-items: center;
    gap: var(--s3);
    white-space: nowrap;
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
    font-size: 22px;
    font-weight: 700;
    line-height: 1;
  }
  :global(:root[data-theme="paper"]) .title {
    letter-spacing: -0.01em;
  }

  /* Hover: pixel inverts, paper inks a soft square. */
  :global(:root[data-theme="pixel"]) .menu:hover,
  :global(:root[data-theme="pixel"]) .menu.open,
  :global(:root[data-theme="pixel"]) .back:hover {
    background: var(--hi-bg);
    color: var(--hi-fg);
  }
  :global(:root[data-theme="paper"]) .menu:hover,
  :global(:root[data-theme="paper"]) .menu.open,
  :global(:root[data-theme="paper"]) .back:hover {
    background: var(--panel);
    box-shadow: inset 0 0 0 1.5px var(--line);
    border-radius: var(--radius);
  }

  .right {
    justify-self: end;
    display: flex;
    align-items: center;
    gap: var(--s5);
    height: 100%;
  }
  .busy {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 180px;
  }
  .bt {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--muted);
  }
  .track {
    position: relative;
    height: 4px;
    overflow: hidden;
  }
  :global(:root[data-theme="pixel"]) .track {
    height: 6px;
    border: 1px solid var(--line);
  }
  :global(:root[data-theme="paper"]) .track {
    background: color-mix(in srgb, var(--fg) 12%, transparent);
  }
  .fill {
    position: absolute;
    inset: 0;
    background: var(--fg);
    transform-origin: left;
    transform: scaleX(var(--p));
    transition: transform 0.3s var(--ease);
  }
  :global(:root[data-theme="paper"]) .fill {
    background: var(--accent-2);
  }
  .fill.sweep {
    transform: none;
    width: 30%;
    animation: sweep 1.2s var(--ease) infinite;
  }
  :global(:root[data-theme="pixel"]) .fill.sweep {
    animation: sweep 1.2s steps(8) infinite;
  }
  @keyframes sweep {
    from {
      left: -30%;
    }
    to {
      left: 100%;
    }
  }
  .clock {
    font-size: 15px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
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
  :global(:root[data-theme="paper"]) .win .x:hover {
    background: var(--accent);
    color: var(--panel);
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
