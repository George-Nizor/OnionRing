<script lang="ts">
  // Plays a short theme-specific transition whenever the view changes:
  // pixel = stepped blinds closing in from both sides, then opening; paper = an e-ink refresh
  // (a quick ink flash, then a ghost of the page fading).
  import { untrack } from "svelte";
  import { app } from "$lib/store.svelte";

  let n = $state(0);
  // The overlay is removed once the effect has played, so a stalled animation (throttled
  // window, reduced motion) can never leave blinds stuck over the page.
  let live = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let first = true;
  $effect(() => {
    void app.view;
    if (first) {
      first = false;
      return;
    }
    // Only the view is a dependency; bumping the counter must not re-run this effect.
    untrack(() => {
      n++;
      live = true;
      clearTimeout(timer);
      timer = setTimeout(() => (live = false), 520);
    });
  });
</script>

{#key n}
  {#if n > 0 && live}
    <div class="t {app.theme}" aria-hidden="true">
      {#if app.theme === "pixel"}
        {#each Array(12) as _, i (i)}<span class:r={i % 2 === 1} style:--i={i}></span>{/each}
      {/if}
    </div>
  {/if}
{/key}

<style>
  .t {
    position: fixed;
    inset: var(--bar) 0 0 0;
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
    animation: blind 0.3s steps(5) both;
    animation-delay: calc(var(--i) * 10ms);
  }
  .pixel span.r {
    transform-origin: right;
  }
  @keyframes blind {
    0% {
      transform: scaleX(1);
    }
    100% {
      transform: scaleX(0);
    }
  }
  .paper {
    animation: refresh 0.34s steps(1) both;
  }
  @keyframes refresh {
    0% {
      background: #141311;
    }
    30% {
      background: #f6f4ee;
    }
    60% {
      background: rgba(238, 235, 227, 0.55);
    }
    100% {
      background: transparent;
    }
  }
</style>
