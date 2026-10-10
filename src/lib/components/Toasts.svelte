<script lang="ts">
  import { app } from "$lib/store.svelte";
  import Icon from "./Icon.svelte";
</script>

<div class="toasts" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.kind}">
      <Icon name={t.kind === "error" ? "alert" : t.kind === "ok" ? "check" : "sparkle"} size={16} />
      <span>{t.text}</span>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 22px;
    bottom: 22px;
    z-index: 100;
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-end;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    max-width: 440px;
    padding: 12px 16px;
    border-radius: 14px;
    background: rgba(24, 21, 32, 0.94);
    border: 1px solid var(--line-2);
    box-shadow: 0 20px 50px -18px rgba(0, 0, 0, 0.9);
    backdrop-filter: blur(16px);
    font-size: 13px;
    animation: rise 0.35s var(--ease);
    pointer-events: auto;
  }
  .toast :global(svg) {
    flex: none;
    margin-top: 1px;
  }
  .ok :global(svg) {
    color: var(--ok);
  }
  .error :global(svg) {
    color: var(--bad);
  }
  .info :global(svg) {
    color: var(--gold);
  }
</style>
