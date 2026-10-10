<script lang="ts">
  // Three offset rings: the layers of an onion, and the rings of the onion router.
  let { size = 28, busy = false }: { size?: number; busy?: boolean } = $props();
  const id = `g${Math.random().toString(36).slice(2, 8)}`;
</script>

<svg class:busy width={size} height={size} viewBox="0 0 64 64" aria-label="OnionRing">
  <defs>
    <linearGradient id={id} x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#ffb547" />
      <stop offset="0.5" stop-color="#ff5d7a" />
      <stop offset="1" stop-color="#9d6bff" />
    </linearGradient>
  </defs>
  <g fill="none" stroke="url(#{id})" stroke-linecap="round">
    <circle class="r1" cx="32" cy="32" r="27" stroke-width="5" stroke-dasharray="140 30" />
    <circle class="r2" cx="32" cy="33.5" r="18" stroke-width="5" stroke-dasharray="88 25" />
    <circle class="r3" cx="32" cy="35" r="9" stroke-width="5" stroke-dasharray="44 13" />
  </g>
</svg>

<style>
  svg {
    display: block;
    overflow: visible;
  }
  circle {
    transform-origin: 32px 32px;
    transition: transform 0.8s var(--ease);
  }
  .r1 {
    transform: rotate(-60deg);
  }
  .r2 {
    transform: rotate(40deg);
    transform-origin: 32px 33.5px;
  }
  .r3 {
    transform: rotate(150deg);
    transform-origin: 32px 35px;
  }
  .busy .r1 {
    animation: t1 2.4s linear infinite;
  }
  .busy .r2 {
    animation: t2 1.8s linear infinite;
  }
  .busy .r3 {
    animation: t3 1.2s linear infinite;
  }
  @keyframes t1 {
    to {
      transform: rotate(300deg);
    }
  }
  @keyframes t2 {
    to {
      transform: rotate(-320deg);
    }
  }
  @keyframes t3 {
    to {
      transform: rotate(510deg);
    }
  }
</style>
