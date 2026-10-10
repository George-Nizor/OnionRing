<script lang="ts">
  import { app } from "$lib/store.svelte";
  import Glyph from "./Glyph.svelte";

  const ICON = { error: "alert", ok: "check", info: "info" } as const;
</script>

<div class="toasts" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.kind}" style:--ms="{t.ms}ms">
      <span class="ic"><Glyph name={ICON[t.kind]} size={24} /></span>
      <span class="tx">{t.text}</span>
      <span class="timer" aria-hidden="true"></span>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: var(--s5);
    bottom: var(--s5);
    z-index: 60;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: var(--s3);
    pointer-events: none;
  }
  .toast {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--s3);
    min-width: 260px;
    max-width: 440px;
    min-height: 56px;
    padding: var(--s3) var(--s4);
    overflow: hidden;
    font-family: var(--label);
    font-size: 15px;
    font-weight: 700;
    line-height: 1.3;
  }
  .ic {
    flex: none;
    display: grid;
    place-items: center;
  }
  .timer {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    background: currentColor;
    transform-origin: left;
    animation: timer var(--ms) linear both;
    opacity: 0.5;
  }
  @keyframes timer {
    to {
      transform: scaleX(0);
    }
  }

  /* pixel: a solid white block that steps in from the right, with a stepped fuse */
  :global(:root[data-theme="pixel"]) .toast {
    background: var(--fg);
    color: var(--bg);
    box-shadow: 6px 6px 0 0 var(--bg), 6px 6px 0 3px var(--fg);
    animation: pxin 0.2s steps(4) both;
  }
  :global(:root[data-theme="pixel"]) .timer {
    animation-timing-function: steps(12);
    height: 4px;
    opacity: 1;
  }
  :global(:root[data-theme="pixel"]) .toast.error .ic {
    animation: blink 0.5s steps(1) 3;
  }
  @keyframes pxin {
    from {
      transform: translateX(calc(100% + 40px));
    }
  }

  /* paper: a card with a coloured edge for what kind of news it is */
  :global(:root[data-theme="paper"]) .toast {
    background: var(--panel);
    border: 1.5px solid var(--line);
    border-left-width: 6px;
    border-radius: var(--radius);
    box-shadow: 5px 5px 0 var(--line);
    font-weight: 700;
    animation: rise 0.35s var(--ease) both;
  }
  :global(:root[data-theme="paper"]) .toast.ok {
    border-left-color: var(--accent-2);
  }
  :global(:root[data-theme="paper"]) .toast.ok .ic {
    color: var(--accent-2);
  }
  :global(:root[data-theme="paper"]) .toast.error {
    border-color: var(--accent);
    box-shadow: 5px 5px 0 var(--accent);
  }
  :global(:root[data-theme="paper"]) .toast.error .ic {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .toast.info {
    border-left-color: var(--accent-3);
  }
  :global(:root[data-theme="paper"]) .timer {
    opacity: 0.25;
  }
</style>
