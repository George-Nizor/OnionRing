<script lang="ts" module>
  // ---- shared geometry (computed once) ------------------------------------------------------
  const C = 100;
  function bumpy(r: number, amps: number[], freqs: number[], phases: number[], steps = 140) {
    let d = "";
    for (let i = 0; i < steps; i++) {
      const t = (i / steps) * Math.PI * 2;
      let rr = r;
      amps.forEach((a, k) => (rr += a * Math.sin(freqs[k] * t + phases[k])));
      d += `${i ? "L" : "M"}${(C + rr * Math.cos(t)).toFixed(1)} ${(C + rr * Math.sin(t)).toFixed(1)}`;
    }
    return d + "Z";
  }
  const OUTER = bumpy(86, [2.6, 1.7, 1.0], [7, 13, 29], [0, 1.1, 2.3]);
  const INNER = bumpy(36, [1.5, 0.9], [9, 17], [0.5, 2.2]);
  const RING = `${OUTER} ${INNER}`;

  // Deterministic crumb texture on the batter.
  let seed = 7;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  const CRUMBS = Array.from({ length: 90 }, () => {
    const t = rnd() * Math.PI * 2;
    const r = 44 + rnd() * 38;
    return { x: C + r * Math.cos(t), y: C + r * Math.sin(t), rx: 0.9 + rnd() * 1.8, ry: 0.7 + rnd() * 1.2, rot: rnd() * 180, light: rnd() > 0.45 };
  });

  const deg = (a: number) => (a * Math.PI) / 180;
  const at = (a: number, r: number) => ({ x: C + r * Math.cos(deg(a)), y: C + r * Math.sin(deg(a)) });
  /** The first bite, top right, made of three overlapping tooth marks. */
  const FIRST_BITE = [
    { ...at(-46, 96), r: 24 },
    { ...at(-63, 92), r: 17 },
    { ...at(-29, 92), r: 16 },
  ];
  /** Further bites, clockwise, used to show progress. */
  const BITES = Array.from({ length: 9 }, (_, k) => {
    const a = -46 + 36 * (k + 1);
    return [{ ...at(a, 95), r: 21 }, { ...at(a + 12, 91), r: 14 }];
  });
  const BURST = Array.from({ length: 10 }, (_, k) => {
    const a = deg(k * 36 + 10);
    return { dx: Math.cos(a) * 70, dy: Math.sin(a) * 70, r: 2.5 + (k % 3) };
  });

  // ---- pixel version: 24x24 grid ---------------------------------------------------------------
  function pixelRows(progress: number) {
    const rows: { x: number; y: number; w: number }[] = [];
    const eaten = Math.floor(progress * 9);
    for (let y = 0; y < 24; y++) {
      let x = 0;
      while (x < 24) {
        const on = (px: number) => {
          const cx = px + 0.5, cy = y + 0.5;
          const d = Math.hypot(cx - 12, cy - 12);
          const inTri = cx >= 9.5 && cx <= 16.5 && Math.abs(cy - 12) <= (16.5 - cx) * 0.62;
          if (inTri) return true;
          if (d < 5.6 || d > 11.6) return false;
          if (Math.hypot(cx - 19.8, cy - 4.2) < 4.3 || Math.hypot(cx - 15.6, cy - 1.4) < 2.8 || Math.hypot(cx - 22.8, cy - 8.3) < 2.6) return false;
          for (let k = 1; k <= eaten; k++) {
            const a = deg(-46 + 36 * k);
            if (Math.hypot(cx - (12 + 11.4 * Math.cos(a)), cy - (12 + 11.4 * Math.sin(a))) < 3.9) return false;
          }
          return true;
        };
        if (on(x)) {
          let w = 1;
          while (x + w < 24 && on(x + w)) w++;
          rows.push({ x, y, w });
          x += w;
        } else x++;
      }
    }
    return rows;
  }

  // Ink hatching lines (45deg) across the whole box, clipped to the ring.
  const HATCH = Array.from({ length: 34 }, (_, i) => -40 + i * 8.5);

  let uid = 0;
</script>

<script lang="ts">
  import { app } from "$lib/store.svelte";

  type Variant = "auto" | "color" | "pixel" | "ink";
  type State = "idle" | "busy" | "launch" | "done" | "still";
  let {
    size = 40,
    variant = "auto",
    state = "idle",
    progress = 0,
  }: { size?: number; variant?: Variant; state?: State; progress?: number } = $props();

  const id = `lg${uid++}`;
  const v = $derived(variant === "auto" ? (app.theme === "pixel" ? "pixel" : "ink") : variant);
  const eaten = $derived(state === "busy" ? Math.max(0, Math.min(9, Math.floor(progress * 9.999))) : 0);
  const prows = $derived(pixelRows(state === "busy" ? progress : 0));
</script>

{#if v === "pixel"}
  <svg class="logo px {state}" width={size} height={size} viewBox="0 0 24 24" shape-rendering="crispEdges" role="img" aria-label="OnionRing">
    {#each prows as r (`${r.x}-${r.y}`)}
      <rect x={r.x} y={r.y} width={r.w} height="1" fill="currentColor" />
    {/each}
  </svg>
{:else if v === "ink"}
  <svg class="logo ink {state}" width={size} height={size} viewBox="0 0 200 200" role="img" aria-label="OnionRing">
    <defs>
      <mask id="{id}m">
        <rect width="200" height="200" fill="#fff" />
        {#each FIRST_BITE as b, i (i)}<circle cx={b.x} cy={b.y} r={b.r} fill="#000" />{/each}
        {#each BITES.slice(0, eaten) as bite, k (k)}
          {#each bite as b, i (i)}<circle class="bite" cx={b.x} cy={b.y} r={b.r} fill="#000" />{/each}
        {/each}
      </mask>
      <clipPath id="{id}c"><path d={RING} clip-rule="evenodd" /></clipPath>
    </defs>
    <g mask="url(#{id}m)">
      <path d={RING} fill-rule="evenodd" fill="var(--accent-3, #c29433)" opacity="0.9" />
      <g clip-path="url(#{id}c)" stroke="var(--fg, #141311)" stroke-width="1.6" opacity="0.55">
        {#each HATCH as h (h)}<line x1={h} y1="0" x2={h + 200} y2="200" />{/each}
      </g>
      <path d={RING} fill-rule="evenodd" fill="none" stroke="var(--fg, #141311)" stroke-width="5" stroke-linejoin="round" />
    </g>
    <path class="tri" d="M88 80 L122 100 L88 120 Z" fill="var(--panel, #f6f4ee)" stroke="var(--fg, #141311)" stroke-width="6" stroke-linejoin="round" />
  </svg>
{:else}
  <svg class="logo color {state}" width={size} height={size} viewBox="0 0 200 200" role="img" aria-label="OnionRing">
    <defs>
      <radialGradient id="{id}g" cx="36%" cy="30%" r="82%">
        <stop offset="0" stop-color="#FFE7A4" />
        <stop offset="0.45" stop-color="#F4AC3D" />
        <stop offset="1" stop-color="#A9520F" />
      </radialGradient>
      <mask id="{id}m">
        <rect width="200" height="200" fill="#fff" />
        {#each FIRST_BITE as b, i (i)}<circle cx={b.x} cy={b.y} r={b.r} fill="#000" />{/each}
        {#each BITES.slice(0, eaten) as bite, k (k)}
          {#each bite as b, i (i)}<circle class="bite" cx={b.x} cy={b.y} r={b.r} fill="#000" />{/each}
        {/each}
      </mask>
      <clipPath id="{id}c"><path d={RING} clip-rule="evenodd" /></clipPath>
    </defs>
    <ellipse class="shadow" cx="100" cy="193" rx="66" ry="6" fill="#000" opacity="0.3" />
    <g class="body">
      <g mask="url(#{id}m)">
        <path d={RING} fill-rule="evenodd" fill="#6E300C" transform="translate(0 4)" />
        <path d={RING} fill-rule="evenodd" fill="url(#{id}g)" />
        <g clip-path="url(#{id}c)">
          <path d={INNER} fill="none" stroke="#7E3A0D" stroke-width="10" opacity="0.4" />
          <path d={OUTER} fill="none" stroke="#8C4510" stroke-width="5" opacity="0.5" />
          {#each CRUMBS as c, i (i)}
            <ellipse cx={c.x} cy={c.y} rx={c.rx} ry={c.ry} transform="rotate({c.rot} {c.x} {c.y})" fill={c.light ? "#FFF0C2" : "#B8621B"} opacity={c.light ? 0.6 : 0.45} />
          {/each}
          <path d="M44 86 A58 58 0 0 1 78 46" fill="none" stroke="#FFF7DC" stroke-width="7" stroke-linecap="round" opacity="0.55" />
        </g>
      </g>
      <circle cx="139" cy="60" r="2.6" fill="#F4AC3D" /><circle cx="148" cy="70" r="1.8" fill="#FFE7A4" /><circle cx="131" cy="50" r="2" fill="#C8721F" />
      <path class="tri" d="M88 80 L122 100 L88 120 Z" fill="#FFF3D6" stroke="#FFF3D6" stroke-width="8" stroke-linejoin="round" />
    </g>
    {#if state === "done"}
      {#each BURST as b, i (i)}
        <circle class="crumb" cx="100" cy="100" r={b.r} fill={i % 2 ? "#F4AC3D" : "#FFE7A4"} style="--dx:{b.dx}px; --dy:{b.dy}px" />
      {/each}
    {/if}
  </svg>
{/if}

<style>
  .logo {
    display: block;
    flex: none;
    overflow: visible;
  }
  /* colour */
  .color .body {
    transform-origin: 100px 100px;
  }
  .color.idle .body {
    animation: bob 3.4s ease-in-out infinite;
  }
  .color.idle .shadow {
    animation: shadow 3.4s ease-in-out infinite;
    transform-origin: 100px 193px;
  }
  .color.launch .body,
  .ink.launch {
    animation: spin 0.9s cubic-bezier(0.5, -0.4, 0.3, 1.4);
  }
  .launch .tri {
    animation: pop 0.6s 0.3s both;
    transform-box: fill-box;
    transform-origin: center;
  }
  .bite {
    animation: chomp 0.35s cubic-bezier(0.3, 1.6, 0.5, 1) both;
    transform-box: fill-box;
    transform-origin: center;
  }
  .busy.color .body {
    animation: nibble 0.5s ease-in-out infinite;
  }
  .crumb {
    animation: burst 0.8s cubic-bezier(0.2, 0.8, 0.3, 1) both;
  }
  @keyframes bob {
    50% {
      transform: translateY(-5px) rotate(-3deg);
    }
  }
  @keyframes shadow {
    50% {
      transform: scaleX(0.86);
      opacity: 0.18;
    }
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @keyframes pop {
    0% {
      transform: scale(0.4);
    }
    70% {
      transform: scale(1.25);
    }
  }
  @keyframes chomp {
    from {
      transform: scale(0);
    }
  }
  @keyframes nibble {
    50% {
      transform: rotate(-4deg) scale(0.98);
    }
  }
  @keyframes burst {
    to {
      transform: translate(var(--dx), var(--dy));
      opacity: 0;
    }
  }
  .ink {
    transform-origin: 50% 50%;
  }
  /* pixel: stepped, like a sprite */
  .px.idle .tri,
  .px.idle {
    animation: none;
  }
  .px.launch {
    animation: pxspin 0.6s steps(4) both;
  }
  .px.busy {
    animation: pxbob 0.8s steps(2) infinite;
  }
  .px.done {
    animation: pxflash 0.4s steps(2) 2;
  }
  @keyframes pxspin {
    to {
      transform: rotate(360deg);
    }
  }
  @keyframes pxbob {
    50% {
      transform: translateY(-2px);
    }
  }
  @keyframes pxflash {
    50% {
      filter: invert(1);
    }
  }
</style>
