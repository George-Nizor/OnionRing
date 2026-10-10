<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { api, type Game, type Session } from "$lib/api";
  import { playtime } from "$lib/format";
  import Glyph from "./Glyph.svelte";

  /** One game's play history: a 12-week bar chart and its latest sessions. */
  let { game }: { game: Game } = $props();

  const WEEKS = 12;
  const DAY = 86_400;

  let sessions = $state<Session[]>([]);
  let loaded = $state(false);
  const id = $derived(game.id);

  $effect(() => {
    const gid = id;
    let live = true;
    const load = () =>
      api
        .gameSessions(gid, 500)
        .then((s) => live && (sessions = s))
        .catch(() => live && (sessions = []))
        .finally(() => live && (loaded = true));
    loaded = false;
    sessions = [];
    load();
    const off = listen<{ id: number }>("game-ended", (e) => e.payload.id === gid && load());
    return () => {
      live = false;
      off.then((f) => f()).catch(() => {});
    };
  });

  function weekStart(t: number): number {
    const d = new Date(t * 1000);
    d.setHours(0, 0, 0, 0);
    d.setDate(d.getDate() - ((d.getDay() + 6) % 7));
    return Math.floor(d.getTime() / 1000);
  }

  const weeks = $derived.by(() => {
    const starts: number[] = [];
    for (let i = 0, t = weekStart(Date.now() / 1000); i < WEEKS; i++) {
      starts.unshift(t);
      t = weekStart(t - DAY);
    }
    const out = starts.map((start) => ({ start, seconds: 0, n: 0 }));
    for (const s of sessions) {
      if (s.started < starts[0]) continue;
      const i = starts.findLastIndex((w) => w <= s.started);
      if (i < 0) continue;
      out[i].seconds += s.seconds;
      out[i].n += 1;
    }
    return out;
  });
  const max = $derived(Math.max(1, ...weeks.map((w) => w.seconds)));
  const inRange = $derived(weeks.reduce((a, w) => a + w.seconds, 0));
  const recent = $derived(sessions.slice(0, 5));

  const short = (secs: number) => {
    if (secs < 60) return secs > 0 ? "<1m" : "";
    const h = Math.floor(secs / 3600);
    const m = Math.round((secs % 3600) / 60);
    return h ? (m ? `${h}h ${m}m` : `${h}h`) : `${m}m`;
  };
  const dayMonth = (t: number) => new Date(t * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short" });
  function day(t: number) {
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const d = new Date(t * 1000);
    d.setHours(0, 0, 0, 0);
    const diff = Math.round((today.getTime() - d.getTime()) / (DAY * 1000));
    if (diff === 0) return "Today";
    if (diff === 1) return "Yesterday";
    return new Date(t * 1000).toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" });
  }
  const clock = (t: number) => new Date(t * 1000).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
</script>

<section class="hist">
  <div class="sh">
    <Glyph name="clock" size={24} />
    <h2 class="label">Play history</h2>
    <span class="rule"></span>
    {#if sessions.length}<span class="meta label">{sessions.length} {sessions.length === 1 ? "session" : "sessions"}</span>{/if}
  </div>

  <div class="chart" class:blank={!inRange} role="img" aria-label="Time played each week, last {WEEKS} weeks: {playtime(inRange)}">
    {#each weeks as w, i (w.start)}
      <div class="col" style:--h="{w.seconds ? Math.max(4, (w.seconds / max) * 100) : 0}%" style:--i={i}>
        {#if w.seconds}<span class="tip label">{short(w.seconds)}</span>{/if}
        <span class="bar" class:now={i === WEEKS - 1}></span>
        <span class="hover label">Week of {dayMonth(w.start)}<br />{w.seconds ? `${short(w.seconds)} · ${w.n} ${w.n === 1 ? "session" : "sessions"}` : "Not played"}</span>
      </div>
    {/each}
    {#if loaded && !inRange}
      <div class="msg">
        {#if !sessions.length && !game.playtime}
          <b>Not played yet</b>
          <span>Each time you play, it shows up here.</span>
        {:else if !sessions.length}
          <b>{playtime(game.playtime)} played so far</b>
          <span>Sessions show up here from your next play.</span>
        {:else}
          <b>Not played in the last {WEEKS} weeks</b>
          <span>Last time was {day(sessions[0].started)}.</span>
        {/if}
      </div>
    {/if}
  </div>
  <div class="axis label">
    <span>{dayMonth(weeks[0].start)}</span>
    <span>This week</span>
  </div>

  {#if recent.length}
    <ol class="sessions">
      {#each recent as s, i (s.id)}
        <li style:--i={i}>
          <span class="d">{day(s.started)}</span>
          <span class="t label">{clock(s.started)} to {clock(s.ended)}</span>
          <b>{short(s.seconds) || "Under a minute"}</b>
        </li>
      {/each}
    </ol>
  {/if}
</section>

<style>
  .hist {
    min-width: 0;
  }
  .chart {
    position: relative;
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    gap: clamp(6px, 0.8vw, 12px);
    height: 168px;
    padding-top: 22px;
  }
  .col {
    position: relative;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    align-items: stretch;
  }
  .bar {
    height: var(--h);
    transform-origin: bottom;
    animation: barup 0.6s var(--ease) both;
    animation-delay: calc(var(--i) * 45ms);
  }
  @keyframes barup {
    from {
      transform: scaleY(0);
    }
  }
  .tip {
    position: absolute;
    left: 50%;
    bottom: calc(var(--h) + 6px);
    transform: translateX(-50%);
    font-size: 11px;
    white-space: nowrap;
    color: var(--faint);
    animation: rise 0.4s var(--ease) both;
    animation-delay: calc(var(--i) * 45ms + 300ms);
  }
  .hover {
    position: absolute;
    left: 50%;
    bottom: calc(100% + 4px);
    z-index: 2;
    transform: translateX(-50%);
    padding: 6px 10px;
    white-space: nowrap;
    text-align: center;
    line-height: 1.5;
    pointer-events: none;
    opacity: 0;
    background: var(--fg);
    color: var(--bg);
  }
  .col:first-child .hover {
    left: 0;
    transform: none;
  }
  .col:last-child .hover {
    left: auto;
    right: 0;
    transform: none;
  }
  .col:hover .hover {
    opacity: 1;
  }
  .col:hover .tip {
    visibility: hidden;
  }
  .msg {
    position: absolute;
    inset: 22px 0 18px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    text-align: center;
    pointer-events: none;
  }
  .msg b {
    font-family: var(--title);
    font-size: 20px;
    font-weight: 700;
  }
  .msg span {
    font-family: var(--label);
    font-size: 14px;
    color: var(--faint);
  }
  .axis {
    display: flex;
    justify-content: space-between;
    margin-top: 10px;
    color: var(--faint);
    font-weight: 400;
  }
  .sessions {
    list-style: none;
    margin: 24px 0 0;
    padding: 0;
  }
  .sessions li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    align-items: baseline;
    gap: 24px;
    padding: 11px 0;
    animation: rise 0.4s var(--ease) both;
    animation-delay: calc(var(--i) * 50ms + 200ms);
  }
  .sessions .d {
    font-family: var(--title);
    font-weight: 600;
  }
  .sessions .t {
    color: var(--faint);
    font-weight: 400;
  }
  .sessions b {
    min-width: 5.5em;
    text-align: right;
    font-family: var(--label);
    font-variant-numeric: tabular-nums;
  }

  /* pixel: hollow bars that step up like sprites; this week solid */
  :global(:root[data-theme="pixel"]) .bar {
    border: 2px solid var(--fg);
    border-bottom: 0;
    background:
      repeating-linear-gradient(0deg, var(--fg) 0 2px, transparent 2px 6px);
    animation-timing-function: steps(6);
  }
  :global(:root[data-theme="pixel"]) .bar.now {
    background: var(--fg);
  }
  :global(:root[data-theme="pixel"]) .chart {
    border-bottom: var(--border) solid var(--line);
  }
  :global(:root[data-theme="pixel"]) .chart.blank .col::after {
    content: "";
    height: 3px;
    margin: 0 30%;
    background: var(--faint);
  }
  :global(:root[data-theme="pixel"]) .tip {
    animation-timing-function: steps(2);
  }
  :global(:root[data-theme="pixel"]) .sessions li {
    border-top: 2px dotted var(--faint);
    animation-timing-function: steps(3);
  }

  /* paper: ink bars, this week in red */
  :global(:root[data-theme="paper"]) .bar {
    background: var(--fg);
  }
  :global(:root[data-theme="paper"]) .bar.now {
    background: var(--accent);
  }
  :global(:root[data-theme="paper"]) .chart {
    border-bottom: 1.5px solid var(--line);
    --rules: repeating-linear-gradient(0deg, transparent 0 41px, color-mix(in srgb, var(--line) 10%, transparent) 41px 42px);
  }
  :global(:root[data-theme="paper"]) .chart:not(.blank) {
    background: var(--rules);
    background-position: 0 100%;
  }
  :global(:root[data-theme="paper"]) .chart.blank .col::after {
    content: "";
    height: 1.5px;
    margin: 0 25%;
    background: color-mix(in srgb, var(--line) 30%, transparent);
  }
  :global(:root[data-theme="paper"]) .hover {
    background: var(--fg);
    color: var(--panel);
  }
  :global(:root[data-theme="paper"]) .sessions li {
    border-top: 1px solid color-mix(in srgb, var(--line) 18%, transparent);
  }
  :global(:root[data-theme="paper"]) .sessions .d {
    font-style: italic;
  }
</style>
