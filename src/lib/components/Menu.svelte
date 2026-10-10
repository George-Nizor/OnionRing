<script lang="ts">
  import { tick } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { app, type ViewName } from "$lib/store.svelte";
  import { focusFirst } from "$lib/nav";
  import Glyph from "./Glyph.svelte";
  import Logo from "./Logo.svelte";

  const items: [ViewName, string, string][] = [
    ["home", "home", "Home"],
    ["library", "library", "Library"],
    ["get", "get", "Get more"],
    ["systems", "pad", "Systems"],
    ["tidy", "broom", "Tidy up"],
    ["settings", "gear", "Settings"],
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
</script>

{#if app.menuOpen}
  <div class="scrim" role="presentation" onclick={() => (app.menuOpen = false)}></div>
  <nav class="menu" data-layer bind:this={panel}>
    <div class="top">
      <Logo size={56} state="idle" />
      <span class="word">OnionRing</span>
    </div>
    {#each items as [name, icon, label], i (name)}
      <button class="item" class:on={app.view.name === name} data-nav style:--i={i} onclick={() => app.go({ name })}>
        <Glyph name={icon} size={36} />
        <span>{label}</span>
      </button>
    {/each}
    <button class="item quit" data-nav style:--i={items.length} onclick={() => getCurrentWindow().close()}>
      <Glyph name="power" size={36} />
      <span>Quit</span>
    </button>
  </nav>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
  }
  :global(:root[data-theme="pixel"]) .scrim {
    background: repeating-linear-gradient(0deg, rgba(0, 0, 0, 0.85) 0 2px, rgba(0, 0, 0, 0.6) 2px 4px);
  }
  :global(:root[data-theme="paper"]) .scrim {
    background: rgba(238, 235, 227, 0.75);
  }
  .menu {
    position: fixed;
    left: 0;
    top: 0;
    bottom: 0;
    z-index: 41;
    width: 340px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 26px 18px;
    background: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .menu {
    border-right: var(--border) solid var(--line);
    animation: slide 0.22s steps(5) both;
  }
  :global(:root[data-theme="paper"]) .menu {
    background: var(--panel);
    border-right: 2px solid var(--line);
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
    gap: 14px;
    padding: 0 10px 22px;
  }
  .word {
    font-family: var(--title);
    font-weight: 800;
    font-size: 26px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 18px;
    height: 64px;
    padding: 0 16px;
    font-family: var(--title);
    font-size: 22px;
    font-weight: 700;
    text-align: left;
    animation: rise 0.3s var(--ease) both;
    animation-delay: calc(var(--i) * 35ms);
  }
  :global(:root[data-theme="pixel"]) .item {
    animation-timing-function: steps(3);
  }
  :global(:root[data-theme="pixel"]) .item:focus-visible span::before {
    content: "▶ ";
    animation: blink 0.8s steps(1) infinite;
  }
  .quit {
    margin-top: auto;
    color: var(--faint);
  }
  :global(:root[data-theme="paper"]) .item.on {
    color: var(--accent);
  }
</style>
