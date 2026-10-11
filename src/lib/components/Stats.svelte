<!-- Play history: totals, the last 12 weeks, most played and recent sessions. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { focusFirst } from "$lib/nav";
  import { listen } from "@tauri-apps/api/event";
  import { api, type PlayTotals, type Session, type TopGame, type WeekTotal, type Game } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import Glyph from "./Glyph.svelte";
  import Art from "./Art.svelte";
  import SystemArt from "./SystemArt.svelte";

  const WEEKS = 12;
  let totals = $state<PlayTotals | null>(null);
  let weeks = $state<WeekTotal[]>([]);
  let top = $state<TopGame[]>([]);
  let recent = $state<Session[]>([]);
  let sel = $state(WEEKS - 1);

  async function load() {
    const [t, w, tg] = await Promise.all([
      api.playTotals().catch(() => ({ seconds: 0, sessions: 0, games: 0 })),
      api.weeklyPlaytime(WEEKS).catch(() => [] as WeekTotal[]),
      api.topGames(5).catch(() => [] as TopGame[]),
    ]);
    // The newest sessions belong to the most recently played games.
    const ids = new Set<number>(tg.map((g) => g.gameId));
    for (const g of [...app.games].filter((g) => g.lastPlayed).sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0)).slice(0, 8)) ids.add(g.id);
    const lists = await Promise.all([...ids].map((id) => api.gameSessions(id, 8).catch(() => [] as Session[])));
    totals = t;
    weeks = w;
    top = tg;
    recent = lists.flat().sort((a, b) => b.started - a.started).slice(0, 6);
    sel = Math.max(0, w.length - 1);
    // The page's controls exist only now; start focus on them, not on the window.
    if (first) {
      first = false;
      await tick();
      if (!root?.contains(document.activeElement)) focusFirst(root);
    }
  }
  let first = true;
  let root = $state<HTMLElement>();

  onMount(() => {
    load();
    const off = listen("game-ended", () => load());
    return () => void off.then((f) => f());
  });

  const played = $derived((totals?.sessions ?? 0) > 0);
  const thisWeek = $derived(weeks.at(-1)?.seconds ?? 0);
  const byId = $derived(new Map<number, Game>(app.games.map((g) => [g.id, g])));

  // Y axis: a round number of hours per gridline, three or four lines.
  const scale = $derived.by(() => {
    const most = Math.max(0, ...weeks.map((w) => w.seconds)) / 3600;
    const step = [0.5, 1, 2, 5, 10, 20, 50, 100].find((s) => most / s <= 4) ?? 200;
    const lines = Math.max(1, Math.ceil(most / step));
    const topH = most === 0 ? 4 : step * lines;
    const ticks: number[] = [];
    for (let h = 0; h <= topH + 1e-9; h += most === 0 ? 1 : step) ticks.push(h);
    return { top: topH * 3600, ticks };
  });

  /** "42h 10m" as parts, so the units can be set smaller than the numbers. */
  function parts(secs: number): [string, string][] {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    if (!h) return [[String(m), "m"]];
    if (h >= 100 || !m) return [[String(h), "h"]];
    return [
      [String(h), "h"],
      [String(m), "m"],
    ];
  }
  const dur = (secs: number) => parts(secs).map(([n, u]) => n + u).join(" ");
  const hours = (h: number) => (h < 1 && h > 0 ? `${h * 60}m` : `${h}h`);
  const weekLabel = (start: number) => new Date(start * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short" });
  const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? "" : "s"}`;

  function day(unix: number) {
    const d = new Date(unix * 1000);
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const diff = Math.round((today.getTime() - new Date(d).setHours(0, 0, 0, 0)) / 86_400_000);
    if (diff <= 0) return "Today";
    if (diff === 1) return "Yesterday";
    if (diff < 7) return d.toLocaleDateString(undefined, { weekday: "long" });
    return d.toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" });
  }
  const clock = (unix: number) => new Date(unix * 1000).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });

  const selWeek = $derived(weeks[sel]);
  const starters = $derived(app.continueList.slice(0, 10));

  const hasPic = (g: Game) => !!(g.coverThumb ?? g.cover ?? g.hero);
  function open(id: number) {
    const g = byId.get(id);
    if (g) app.openGame(g);
  }
</script>

<div class="stats" bind:this={root}>
  {#if totals}
    <div class="cells" class:quiet={!played}>
      <div class="cell">
        <span class="k label"><Glyph name="clock" size={24} />Time played</span>
        <span class="v">{#each parts(totals.seconds) as [n, u], i (i)}<b>{n}</b><small>{u}</small>{/each}</span>
      </div>
      <div class="cell">
        <span class="k label"><Glyph name="star" size={24} />This week</span>
        <span class="v">{#each parts(thisWeek) as [n, u], i (i)}<b>{n}</b><small>{u}</small>{/each}</span>
      </div>
      <div class="cell">
        <span class="k label"><Glyph name="play" size={24} />Sessions</span>
        <span class="v"><b>{totals.sessions}</b></span>
      </div>
      <div class="cell">
        <span class="k label"><Glyph name="pad" size={24} />Games played</span>
        <span class="v"><b>{totals.games}</b></span>
      </div>
    </div>

    <section class="weekly">
      <div class="head">
        <span class="label h">Last 12 weeks</span>
        {#if played && selWeek}
          <span class="readout">
            <span class="label">{sel === weeks.length - 1 ? "This week" : `Week of ${weekLabel(selWeek.start)}`}</span>
            <b>{selWeek.seconds ? dur(selWeek.seconds) : "Nothing played"}</b>
            {#if selWeek.sessions}<span class="label n">{plural(selWeek.sessions, "session")}</span>{/if}
          </span>
        {/if}
      </div>

      <div class="chart" class:empty={!played}>
        <div class="plot" role="group" aria-label="Time played per week" onmouseleave={() => (sel = weeks.length - 1)}>
          {#each scale.ticks as t (t)}
            <div class="tick" style:bottom="{(t * 3600 * 100) / scale.top}%"><span class="label">{hours(t)}</span></div>
          {/each}
          <div class="cols">
            {#each weeks as w, i (w.start)}
              <button
                class="col"
                class:sel={played && sel === i}
                class:now={i === weeks.length - 1}
                data-nav
                disabled={!played}
                onmouseenter={() => (sel = i)}
                onfocus={() => (sel = i)}
                aria-label="Week of {weekLabel(w.start)}: {w.seconds ? dur(w.seconds) : 'nothing played'}"
              >
                <span class="bar" style:--h="{(w.seconds * 100) / scale.top}%" style:--i={i}>
                  {#if played && sel === i && w.seconds}<span class="val label">{dur(w.seconds)}</span>{/if}
                </span>
              </button>
            {/each}
          </div>
          {#if !played}
            <div class="blank">
              <Glyph name="clock" size={48} />
              <b>No play time yet</b>
              <span>Start a game from OnionRing and your hours fill in here, week by week.</span>
            </div>
          {/if}
        </div>
        <div class="weeks">
          {#each weeks as w, i (w.start)}
            <span class="label" class:on={played && sel === i}>{i === weeks.length - 1 ? "Now" : weekLabel(w.start)}</span>
          {/each}
        </div>
      </div>
    </section>

    {#if played}
      <div class="lists">
        <section>
          <div class="head"><span class="label h">Most played</span></div>
          <div class="list">
            {#each top as t, i (t.gameId)}
              {@const g = byId.get(t.gameId)}
              <button class="game" data-nav onclick={() => open(t.gameId)} style:--i={i}>
                <span class="rank label">{i + 1}</span>
                <span class="cover">
                  {#if g && hasPic(g)}<Art game={g} />{:else}<span class="sysfill"><SystemArt id={t.platform} /></span>{/if}
                </span>
                <span class="mid">
                  <span class="t" title={t.title}>{t.title}</span>
                  <span class="p label">{app.platforms.get(t.platform)?.name ?? t.platform}</span>
                  <span class="share"><span style:--w="{(t.seconds * 100) / Math.max(1, top[0]?.seconds ?? 1)}%"></span></span>
                </span>
                <span class="end">
                  <b>{dur(t.seconds)}</b>
                  <span class="label">{plural(t.sessions, "session")}</span>
                </span>
              </button>
            {/each}
          </div>
        </section>

        <section>
          <div class="head"><span class="label h">Recent sessions</span></div>
          <div class="list">
            {#each recent as s, i (s.id)}
              {@const g = byId.get(s.gameId)}
              <button class="session" data-nav onclick={() => open(s.gameId)} style:--i={i}>
                <span class="cover small">
                  {#if g && hasPic(g)}<Art game={g} />{:else}<span class="sysfill"><SystemArt id={g?.platform ?? ""} /></span>{/if}
                </span>
                <span class="mid">
                  <span class="t" title={g?.title}>{g?.title ?? "A removed game"}</span>
                  <span class="p label">{day(s.started)}, {clock(s.started)}</span>
                </span>
                <b class="len">{dur(s.seconds)}</b>
              </button>
            {/each}
          </div>
        </section>
      </div>
    {:else if starters.length}
      <section class="starters">
        <div class="head"><span class="label h">Something to start with</span></div>
        <div class="shelf">
          {#each starters as g, i (g.id)}
            <button class="pick" data-nav onclick={() => app.openGame(g)} style:--i={i}>
              <span class="cover"><Art game={g} /></span>
              <span class="t">{g.title}</span>
            </button>
          {/each}
        </div>
      </section>
    {/if}
  {/if}
</div>

<style>
  .stats {
    height: 100%;
    overflow-y: auto;
    padding: 32px 56px 64px;
    display: flex;
    flex-direction: column;
    gap: 48px;
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 16px;
    min-height: 32px;
    margin-bottom: 16px;
  }
  .h {
    color: var(--muted);
  }

  /* ---- Totals ---- */
  .cells {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    border-top: var(--border) solid var(--line);
    border-bottom: var(--border) solid var(--line);
  }
  :global(:root[data-theme="paper"]) .cells {
    border-width: 1px;
  }
  .cell {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 20px 24px;
    min-width: 0;
  }
  .cell:first-child {
    padding-left: 0;
  }
  .cell + .cell {
    border-left: 1px solid var(--faint);
  }
  :global(:root[data-theme="pixel"]) .cell + .cell {
    border-left: 3px dotted var(--faint);
  }
  .k {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--faint);
    white-space: nowrap;
  }
  .v {
    display: flex;
    align-items: baseline;
    gap: 2px;
    font-family: var(--title);
    line-height: 1;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .v b {
    font-size: 56px;
    font-weight: 700;
  }
  .v small {
    font-size: 24px;
    margin-right: 12px;
    color: var(--muted);
  }
  :global(:root[data-theme="paper"]) .v b {
    letter-spacing: -0.02em;
  }
  :global(:root[data-theme="paper"]) .cell:first-child .v b {
    color: var(--accent);
  }
  .quiet .v {
    color: var(--faint);
  }
  :global(:root[data-theme="paper"]) .quiet .cell:first-child .v b {
    color: var(--faint);
  }

  /* ---- Weekly chart ---- */
  .readout {
    margin-left: auto;
    display: flex;
    align-items: baseline;
    gap: 12px;
  }
  .readout .label {
    color: var(--faint);
  }
  .readout b {
    font-family: var(--title);
    font-size: 22px;
  }
  .chart {
    padding-left: 56px;
  }
  .plot {
    position: relative;
    height: clamp(180px, 28vh, 300px);
  }
  .tick {
    position: absolute;
    left: 0;
    right: 0;
    height: 0;
    border-top: 1px dashed color-mix(in srgb, var(--line) 25%, transparent);
  }
  .tick:first-child {
    border-top: var(--border) solid var(--line);
  }
  :global(:root[data-theme="paper"]) .tick:first-child {
    border-top-width: 1.5px;
  }
  :global(:root[data-theme="pixel"]) .tick:not(:first-child) {
    border-top: 2px dotted color-mix(in srgb, var(--line) 40%, transparent);
  }
  .tick .label {
    position: absolute;
    right: calc(100% + 12px);
    top: 0;
    transform: translateY(-50%);
    color: var(--faint);
    white-space: nowrap;
  }
  .cols {
    position: absolute;
    inset: 0;
    display: grid;
    grid-template-columns: repeat(12, 1fr);
  }
  .col {
    position: relative;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    padding: 0 18%;
    cursor: default;
  }
  .col:not(:disabled) {
    cursor: pointer;
  }
  /* The chart shows its own selection; no inverted block or underline. */
  :global(:root[data-theme="pixel"]) .col:focus-visible {
    background: none;
    color: var(--fg);
  }
  :global(:root[data-theme="paper"]) .col:focus-visible::after {
    display: none;
  }
  .bar {
    position: relative;
    width: 100%;
    max-width: 72px;
    height: var(--h);
    min-height: 0;
  }
  :global(:root[data-theme="pixel"]) .bar {
    background: repeating-linear-gradient(to top, var(--fg) 0 9px, transparent 9px 12px);
    background-position: bottom;
    transform-origin: bottom;
    animation: stack 0.5s steps(6) both;
    animation-delay: calc(var(--i) * 45ms);
  }
  :global(:root[data-theme="pixel"]) .col:hover .bar,
  :global(:root[data-theme="pixel"]) .sel .bar {
    background: var(--fg);
  }
  :global(:root[data-theme="pixel"]) .sel .bar {
    outline: 3px solid var(--fg);
    outline-offset: 3px;
  }
  @keyframes stack {
    from {
      transform: scaleY(0);
    }
  }
  :global(:root[data-theme="paper"]) .bar {
    background: var(--muted);
    animation: inkin 0.8s var(--ease) both;
    animation-delay: calc(var(--i) * 50ms);
  }
  :global(:root[data-theme="paper"]) .now .bar {
    background: var(--fg);
  }
  :global(:root[data-theme="paper"]) .col:hover .bar,
  :global(:root[data-theme="paper"]) .sel .bar {
    background: var(--accent);
  }
  @keyframes inkin {
    0% {
      clip-path: inset(100% 0 0 0);
      opacity: 0.4;
    }
    60% {
      opacity: 1;
    }
    100% {
      clip-path: inset(-40px -40px 0 -40px);
    }
  }
  .val {
    position: absolute;
    bottom: calc(100% + 10px);
    left: 50%;
    transform: translateX(-50%);
    white-space: nowrap;
    padding: 4px 8px;
    background: var(--fg);
    color: var(--bg);
  }
  .weeks {
    display: grid;
    grid-template-columns: repeat(12, 1fr);
    margin-top: 12px;
  }
  .weeks span {
    text-align: center;
    color: var(--faint);
    white-space: nowrap;
  }
  .weeks span.on {
    color: var(--fg);
  }
  :global(:root[data-theme="paper"]) .weeks span.on {
    color: var(--accent);
  }
  .empty .col {
    pointer-events: none;
  }
  .empty .bar {
    display: none;
  }
  .blank {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    text-align: center;
    padding-bottom: 8px;
  }
  .blank :global(svg) {
    color: var(--faint);
    margin-bottom: 4px;
  }
  .blank b {
    font-family: var(--title);
    font-size: 26px;
  }
  .blank span {
    color: var(--muted);
    max-width: 420px;
    padding: 0 16px;
  }
  :global(:root[data-theme="pixel"]) .blank b,
  :global(:root[data-theme="pixel"]) .blank span {
    background: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .blank :global(svg) {
    animation: blink 1.2s steps(1) infinite;
  }
  :global(:root[data-theme="paper"]) .blank b,
  :global(:root[data-theme="paper"]) .blank span {
    background: var(--bg);
    padding: 0 12px;
  }

  /* ---- Lists ---- */
  .lists {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 48px;
  }
  .list {
    display: flex;
    flex-direction: column;
  }
  .game,
  .session {
    display: grid;
    align-items: center;
    gap: 16px;
    padding: 8px;
    margin: 0 -8px;
    text-align: left;
    border-top: 1px solid color-mix(in srgb, var(--line) 18%, transparent);
    animation: rise 0.4s var(--ease) both;
    animation-delay: calc(var(--i) * 50ms + 200ms);
  }
  :global(:root[data-theme="pixel"]) .game,
  :global(:root[data-theme="pixel"]) .session {
    animation-timing-function: steps(3);
  }
  .game {
    grid-template-columns: 24px 48px 1fr auto;
  }
  .session {
    grid-template-columns: 36px 1fr auto;
  }
  .cover {
    position: relative;
    display: block;
    width: 48px;
    aspect-ratio: 3 / 4;
    overflow: hidden;
  }
  .cover.small {
    width: 36px;
  }
  .sysfill {
    position: absolute;
    inset: 0;
    background: var(--panel);
  }
  .rank {
    color: var(--faint);
    text-align: center;
  }
  .mid {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .t {
    font-family: var(--title);
    font-weight: 700;
    font-size: 17px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .p {
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .share {
    display: block;
    height: 6px;
    margin-top: 4px;
  }
  .share span {
    display: block;
    height: 100%;
    width: var(--w);
    background: currentColor;
  }
  :global(:root[data-theme="pixel"]) .share span {
    background: repeating-linear-gradient(90deg, currentColor 0 6px, transparent 6px 8px);
  }
  :global(:root[data-theme="paper"]) .game:first-child .share span {
    background: var(--accent);
  }
  .end {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
    white-space: nowrap;
  }
  .end b,
  .len {
    font-family: var(--title);
    font-size: 18px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .end .label {
    color: var(--faint);
  }

  /* ---- Empty: a shelf of games to start with ---- */
  .shelf {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    grid-template-rows: auto;
    grid-auto-rows: 0;
    column-gap: 20px;
    overflow: hidden;
    padding: 8px;
    margin: -8px;
  }
  .pick {
    display: flex;
    flex-direction: column;
    gap: 10px;
    text-align: left;
    padding: 0;
    animation: rise 0.4s var(--ease) both;
    animation-delay: calc(var(--i) * 40ms + 200ms);
  }
  :global(:root[data-theme="pixel"]) .pick {
    animation-timing-function: steps(3);
  }
  .pick .cover {
    width: 100%;
  }
  .pick .t {
    white-space: normal;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    font-size: 16px;
    line-height: 1.25;
    min-height: 2.5em;
  }

  /* ---- Hover (mouse) and focus ---- */
  :global(:root[data-theme="pixel"]) .game:hover,
  :global(:root[data-theme="pixel"]) .session:hover {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .game:hover .p,
  :global(:root[data-theme="pixel"]) .game:hover .rank,
  :global(:root[data-theme="pixel"]) .game:hover .end .label,
  :global(:root[data-theme="pixel"]) .session:hover .p,
  :global(:root[data-theme="pixel"]) .game:focus-visible .p,
  :global(:root[data-theme="pixel"]) .game:focus-visible .rank,
  :global(:root[data-theme="pixel"]) .game:focus-visible .end .label,
  :global(:root[data-theme="pixel"]) .session:focus-visible .p {
    color: var(--bg);
  }
  :global(:root[data-theme="paper"]) .game:hover,
  :global(:root[data-theme="paper"]) .session:hover,
  :global(:root[data-theme="paper"]) .game:focus-visible,
  :global(:root[data-theme="paper"]) .session:focus-visible {
    background: var(--panel);
  }
  :global(:root[data-theme="paper"]) .game:hover .t,
  :global(:root[data-theme="paper"]) .session:hover .t {
    text-decoration: underline;
    text-decoration-thickness: 1.5px;
    text-underline-offset: 3px;
  }
  .pick .cover {
    transition: transform 0.2s var(--ease);
  }
  :global(:root[data-theme="pixel"]) .pick:hover .cover,
  :global(:root[data-theme="pixel"]) .pick:focus-visible .cover {
    outline: 3px solid var(--fg);
    outline-offset: 4px;
  }
  :global(:root[data-theme="pixel"]) .pick:focus-visible {
    background: none;
    color: var(--fg);
  }
  :global(:root[data-theme="paper"]) .pick:hover .cover,
  :global(:root[data-theme="paper"]) .pick:focus-visible .cover {
    transform: translate(-2px, -2px);
    box-shadow: 4px 4px 0 var(--fg);
  }
  :global(:root[data-theme="paper"]) .pick :global(.art) {
    filter: var(--img-filter);
  }
  :global(:root[data-theme="paper"]) .pick:hover :global(.art),
  :global(:root[data-theme="paper"]) .pick:focus-visible :global(.art) {
    filter: var(--img-filter-focus);
  }

  @media (max-width: 1200px) {
    .v b {
      font-size: 44px;
    }
    .v small {
      font-size: 20px;
    }
    .cell {
      padding: 16px 20px;
    }
    .lists {
      gap: 32px;
    }
  }
</style>
