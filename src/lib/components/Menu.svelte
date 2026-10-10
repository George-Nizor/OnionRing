<script lang="ts">
  import { tick } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { app, type ViewName } from "$lib/store.svelte";
  import { focusFirst } from "$lib/nav";
  import Glyph from "./Glyph.svelte";
  import Logo from "./Logo.svelte";

  type Item = [ViewName, string, string];
  const groups: Item[][] = [
    [
      ["home", "home", "Home"],
      ["library", "library", "Library"],
      ["get", "get", "Get more"],
      ["systems", "pad", "Systems"],
    ],
    [
      ["stats", "chart", "Play history"],
      ["tidy", "broom", "Tidy up"],
      ["settings", "gear", "Settings"],
    ],
  ];
  let panel = $state<HTMLElement>();
  $effect(() => {
    if (app.menuOpen)
      tick().then(() => {
        const on = panel?.querySelector<HTMLElement>(".item.on");
        if (on) on.focus();
        else focusFirst(panel);
      });
  });
  const hover = (e: MouseEvent) => (e.currentTarget as HTMLElement).focus({ preventScroll: true });
</script>

{#if app.menuOpen}
  <div class="scrim" role="presentation" onclick={() => (app.menuOpen = false)}></div>
  <nav class="menu" data-layer bind:this={panel} aria-label="Menu">
    <div class="top">
      <Logo size={48} state="idle" />
      <span class="word">OnionRing</span>
    </div>
    {#each groups as group, gi (gi)}
      <div class="group">
        {#each group as [name, icon, label], i (name)}
          <button class="item" class:on={app.view.name === name} data-nav style:--i={gi * 4 + i} onclick={() => app.go({ name })} onmouseenter={hover}>
            <Glyph name={icon} size={24} />
            <span>{label}</span>
          </button>
        {/each}
      </div>
    {/each}
    <div class="foot">
      <button class="item quit" data-nav style:--i={8} onclick={() => getCurrentWindow().close()} onmouseenter={hover}>
        <Glyph name="power" size={24} />
        <span>Quit</span>
      </button>
      {#if app.boot?.version}<span class="ver label">v{app.boot.version}</span>{/if}
    </div>
  </nav>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
  }
  :global(:root[data-theme="pixel"]) .scrim {
    background: repeating-linear-gradient(0deg, rgba(0, 0, 0, 0.88) 0 2px, rgba(0, 0, 0, 0.62) 2px 4px);
    animation: scrim 0.2s steps(3) both;
  }
  :global(:root[data-theme="paper"]) .scrim {
    background: rgba(238, 235, 227, 0.72);
    animation: scrim 0.3s both;
  }
  @keyframes scrim {
    from {
      opacity: 0;
    }
  }
  .menu {
    position: fixed;
    left: 0;
    top: 0;
    bottom: 0;
    z-index: 41;
    width: 300px;
    display: flex;
    flex-direction: column;
    padding: var(--s4) var(--s3) var(--s4);
    background: var(--bg);
    overflow-y: auto;
  }
  :global(:root[data-theme="pixel"]) .menu {
    border-right: var(--border) solid var(--line);
    animation: slide 0.22s steps(5) both;
  }
  :global(:root[data-theme="paper"]) .menu {
    background: var(--panel);
    border-right: 1.5px solid var(--line);
    box-shadow: 8px 0 0 color-mix(in srgb, var(--line) 10%, transparent);
    animation: slide 0.35s var(--ease) both;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
  }
  .top {
    display: flex;
    align-items: center;
    gap: var(--s3);
    height: 56px;
    padding: 0 var(--s3);
    margin-bottom: var(--s3);
  }
  .word {
    font-family: var(--title);
    font-weight: 800;
    font-size: 24px;
    letter-spacing: -0.01em;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--s2) 0;
  }
  .group + .group {
    border-top: 1.5px solid color-mix(in srgb, var(--line) 25%, transparent);
  }
  :global(:root[data-theme="pixel"]) .group + .group {
    border-top: 3px dotted var(--faint);
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--s4);
    height: 48px;
    padding: 0 var(--s3);
    font-family: var(--title);
    font-size: 19px;
    font-weight: 600;
    text-align: left;
    animation: rise 0.3s var(--ease) both;
    animation-delay: calc(var(--i) * 30ms);
  }
  :global(:root[data-theme="pixel"]) .item {
    animation-timing-function: steps(3);
  }
  :global(:root[data-theme="pixel"]) .item:hover {
    background: var(--hi-bg);
    color: var(--hi-fg);
  }
  :global(:root[data-theme="pixel"]) .item:is(:focus-visible, :hover) span::before {
    content: "▶ ";
    animation: blink 0.8s steps(1) infinite;
  }
  :global(:root[data-theme="pixel"]) .item.on:not(:focus-visible):not(:hover) {
    background: none;
    color: var(--fg);
    box-shadow: inset 6px 0 0 var(--fg);
  }
  /* paper: the current page is marked with the red ribbon; hover inks the row */
  :global(:root[data-theme="paper"]) .item {
    border-radius: var(--radius);
    transition: background 0.15s;
  }
  :global(:root[data-theme="paper"]) .item:is(:hover, :focus-visible) {
    background: var(--bg);
  }
  :global(:root[data-theme="paper"]) .item:focus-visible::after {
    left: var(--s3);
    right: auto;
    width: 40%;
    bottom: 6px;
    height: 2px;
  }
  :global(:root[data-theme="paper"]) .item.on {
    color: var(--accent);
    box-shadow: inset 3px 0 0 var(--accent);
  }
  .foot {
    margin-top: auto;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-top: var(--s3);
  }
  .quit {
    color: var(--faint);
    flex: 1;
  }
  .ver {
    color: var(--faint);
    padding: 0 var(--s3);
  }
</style>
