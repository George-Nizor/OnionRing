<script lang="ts">
  import { app } from "$lib/store.svelte";
  import Glyph from "./Glyph.svelte";
</script>

<div class="toasts" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.kind}">
      <Glyph name={t.kind === "error" ? "alert" : t.kind === "ok" ? "check" : "info"} size={24} />
      <span>{t.text}</span>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 26px;
    bottom: calc(var(--hints) + 20px);
    z-index: 60;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 10px;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 12px;
    max-width: 480px;
    padding: 14px 18px;
    font-family: var(--label);
    font-size: 15px;
    font-weight: 700;
  }
  :global(:root[data-theme="pixel"]) .toast {
    background: var(--fg);
    color: var(--bg);
    animation: pxin 0.2s steps(3) both;
  }
  :global(:root[data-theme="paper"]) .toast {
    background: var(--panel);
    border: 2px solid var(--line);
    box-shadow: 5px 5px 0 var(--line);
    animation: rise 0.3s var(--ease) both;
  }
  :global(:root[data-theme="paper"]) .toast.error {
    border-color: var(--accent);
    box-shadow: 5px 5px 0 var(--accent);
  }
  @keyframes pxin {
    from {
      transform: translateX(110%);
    }
  }
</style>
