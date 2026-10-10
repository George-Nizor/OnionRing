<script lang="ts">
  // Plays a short theme-specific transition whenever the view changes:
  // pixel = stepped blinds wiping across; paper = an e-ink full refresh (black, then white).
  import { untrack } from "svelte";
  import { app } from "$lib/store.svelte";

  let n = $state(0);
  let first = true;
  $effect(() => {
    void app.view;
    if (first) {
      first = false;
      return;
    }
    // Only the view is a dependency; bumping the counter must not re-run this effect.
    untrack(() => n++);
  });
</script>

{#key n}
  {#if n > 0}
    <div class="t {app.theme}" aria-hidden="true">
      {#if app.theme === "pixel"}
        {#each Array(10) as _, i (i)}<span style:--i={i}></span>{/each}
      {/if}
    </div>
  {/if}
{/key}

<style>
  .t {
    position: fixed;
    inset: var(--bar) 0 var(--hints) 0;
    z-index: 30;
    pointer-events: none;
  }
  .pixel {
    display: flex;
    flex-direction: column;
  }
  .pixel span {
    flex: 1;
    background: var(--fg);
    transform-origin: left;
    animation: blind 0.28s steps(4) both;
    animation-delay: calc(var(--i) * 12ms);
  }
  @keyframes blind {
    0% {
      transform: scaleX(1);
    }
    100% {
      transform: scaleX(0);
      transform-origin: right;
    }
  }
  .paper {
    animation: refresh 0.32s steps(1) both;
  }
  @keyframes refresh {
    0% {
      background: #141311;
    }
    35% {
      background: #f6f4ee;
    }
    65% {
      background: rgba(246, 244, 238, 0.5);
    }
    100% {
      background: transparent;
    }
  }
</style>
