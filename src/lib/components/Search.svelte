<script lang="ts">
  // The search field in the top bar. Typing opens a results panel under the bar covering your
  // games and everything in Get more. Arrow down moves into the results, Enter opens the
  // highlighted (or top) result, Esc clears, clicking anywhere else closes.
  import type { Game } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { move } from "$lib/nav";
  import Art from "./Art.svelte";
  import Glyph from "./Glyph.svelte";
  import Pic from "./Pic.svelte";

  let wrap = $state<HTMLElement>();
  let input = $state<HTMLInputElement>();
  let panel = $state<HTMLElement>();
  let typing = $state(false);

  const q = $derived(app.query.trim());
  const open = $derived(app.searchOpen && q.length > 0);
  const r = $derived(app.everything);
  const installed = $derived(new Set((app.catalog?.installed ?? []).map((i) => i.id)));

  type Item = { id: string; name: string; meta: string };
  type Group = { tab: "ports" | "emulators" | "browser"; icon: string; label: string; kind: "port" | "emulator" | "browser"; items: Item[]; all: number };
  const games = $derived(r.games.slice(0, 9));
  const groups = $derived(
    (
      [
        { tab: "ports", icon: "cart", label: "Ports", kind: "port", all: r.ports.length, items: r.ports.slice(0, 6).map((p) => ({ id: p.id, name: p.game, meta: p.name })) },
        {
          tab: "emulators",
          icon: "pad",
          label: "Emulators",
          kind: "emulator",
          all: r.emulators.length,
          items: r.emulators.slice(0, 6).map((e) => ({ id: e.id, name: e.name, meta: e.platforms.map((p) => app.platforms.get(p)?.short ?? p).join(" · ") })),
        },
        { tab: "browser", icon: "globe", label: "Browser games", kind: "browser", all: r.browser.length, items: r.browser.slice(0, 6).map((b) => ({ id: b.id, name: b.title, meta: b.original ?? b.project })) },
      ] satisfies Group[]
    ).filter((g) => g.items.length),
  );
  const total = $derived(r.games.length + r.ports.length + r.emulators.length + r.browser.length);

  // Ctrl+K, "/" or the controller's Y ask for the field.
  $effect(() => {
    if (!app.searchFocus) return;
    input?.focus();
    input?.select();
  });

  function openGame(g: Game) {
    app.closeSearch();
    app.openGame(g);
  }
  function openItem(tab: string, item: string) {
    app.closeSearch();
    app.go({ name: "get", tab, item });
  }

  function onInputKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" && open) {
      e.preventDefault();
      panel?.querySelector<HTMLElement>(".hit")?.focus();
    } else if (e.key === "Enter" && open) {
      e.preventDefault();
      panel?.querySelector<HTMLElement>(".hit")?.click();
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      app.closeSearch();
      input?.blur();
    }
  }
  // Inside the results: up from the top row returns to the field; typing goes back to the field.
  function onPanelKey(e: KeyboardEvent) {
    if (e.key === "ArrowUp") {
      e.preventDefault();
      e.stopPropagation();
      if (!move("up")) input?.focus();
    } else if (e.key === "Backspace" || (e.key.length === 1 && e.key !== " " && !e.ctrlKey && !e.altKey)) {
      e.stopPropagation();
      input?.focus();
    }
  }
  function onPointerDown(e: PointerEvent) {
    if (app.searchOpen && wrap && !wrap.contains(e.target as Node)) app.searchOpen = false;
  }
  function onFocusOut(e: FocusEvent) {
    const to = e.relatedTarget as Node | null;
    if (to && wrap && !wrap.contains(to)) app.searchOpen = false;
  }
</script>

<svelte:window onpointerdown={onPointerDown} />

<div class="search" class:active={app.searchOpen} bind:this={wrap} data-layer={open ? "" : undefined} onfocusout={onFocusOut}>
  <label class="field">
    <Glyph name="search" size={24} />
    <input
      bind:this={input}
      bind:value={app.query}
      placeholder="Search games, ports, emulators"
      spellcheck="false"
      autocomplete="off"
      aria-label="Search"
      onfocus={() => {
        typing = true;
        app.searchOpen = true;
      }}
      onblur={() => (typing = false)}
      oninput={() => (app.searchOpen = true)}
      onkeydown={onInputKey}
    />
    {#if app.query}
      <button
        class="clear"
        tabindex="-1"
        aria-label="Clear"
        onclick={() => {
          app.query = "";
          input?.focus();
        }}><Glyph name="close" size={12} /></button
      >
    {/if}
  </label>

  {#if open}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="panel" bind:this={panel} onkeydown={onPanelKey}>
      {#if total === 0}
        <div class="none">
          <Glyph name="search" size={48} />
          <p>Nothing called “{q}” in your games or in Get more.</p>
        </div>
      {:else}
        {#if games.length}
          <section style:--d={0}>
            <header class="sh">
              <Glyph name="library" size={24} />
              <span class="label">Your games</span>
              <span class="n label">{r.games.length}</span>
            </header>
            <div class="grid">
              {#each games as g, i (g.id)}
                <button class="hit" class:top={typing && i === 0} data-nav data-game-id={g.id} onclick={() => openGame(g)}>
                  <span class="th cover"><Art game={g} /></span>
                  <span class="tx">
                    <span class="nm">{g.title}</span>
                    <span class="mt label">{g.kind === "port" ? "PC port" : (app.platforms.get(g.platform)?.short ?? "")}</span>
                  </span>
                  <span class="go"><Glyph name="next" size={24} /></span>
                </button>
              {/each}
            </div>
          </section>
        {/if}
        {#each groups as grp, gi (grp.tab)}
          <section style:--d={gi + 1}>
            <header class="sh">
              <Glyph name={grp.icon} size={24} />
              <span class="label">{grp.label}</span>
              <span class="n label">{grp.all}</span>
            </header>
            <div class="grid">
              {#each grp.items as it, i (it.id)}
                <button class="hit" class:top={typing && !games.length && gi === 0 && i === 0} data-nav onclick={() => openItem(grp.tab, it.id)}>
                  <span class="th pic" class:light={grp.kind === "emulator"}><Pic kind={grp.kind} id={it.id} icon={grp.icon} fit={grp.kind === "emulator" ? "contain" : "cover"} /></span>
                  <span class="tx">
                    <span class="nm">{it.name}</span>
                    <span class="mt label">{it.meta}</span>
                  </span>
                  {#if installed.has(it.id)}
                    <span class="have"><Glyph name="check" size={24} label="Installed" /></span>
                  {:else}
                    <span class="go"><Glyph name="next" size={24} /></span>
                  {/if}
                </button>
              {/each}
            </div>
          </section>
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .search {
    position: relative;
    min-width: 0;
    height: 100%;
    display: flex;
    align-items: center;
  }
  .field {
    flex: 1;
    min-width: 0;
    height: 40px;
    display: flex;
    align-items: center;
    gap: var(--s3);
    padding: 0 var(--s2) 0 var(--s3);
    color: var(--faint);
    cursor: text;
    transition:
      color 0.15s,
      box-shadow 0.2s var(--ease);
  }
  .field:hover,
  .active .field {
    color: var(--fg);
  }
  :global(:root[data-theme="pixel"]) .field {
    border: var(--border) solid var(--faint);
  }
  :global(:root[data-theme="pixel"]) .field:hover,
  :global(:root[data-theme="pixel"]) .active .field {
    border-color: var(--line);
  }
  :global(:root[data-theme="paper"]) .field {
    background: var(--panel);
    border-radius: var(--radius);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--line) 40%, transparent);
  }
  :global(:root[data-theme="paper"]) .field:hover {
    box-shadow: inset 0 0 0 1.5px var(--line);
  }
  :global(:root[data-theme="paper"]) .active .field {
    box-shadow:
      inset 0 0 0 1.5px var(--line),
      3px 3px 0 var(--line);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: 0;
    background: none;
    font-family: var(--text);
    font-size: 16px;
    color: var(--fg);
    caret-color: var(--fg);
  }
  :global(:root[data-theme="paper"]) input {
    caret-color: var(--accent);
  }
  input::placeholder {
    color: var(--faint);
    opacity: 1;
  }
  .clear {
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    color: var(--faint);
  }
  .clear:hover {
    color: var(--fg);
  }

  /* ---- results panel ---- */
  .panel {
    position: fixed;
    top: calc(var(--bar) + var(--s2));
    left: 50%;
    transform: translateX(-50%);
    width: min(1040px, calc(100vw - 2 * var(--gutter)));
    max-height: calc(100vh - var(--bar) - var(--s7));
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--s5);
    padding: var(--s5);
    background: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .panel {
    border: var(--border) solid var(--line);
    box-shadow:
      8px 8px 0 0 #000,
      8px 8px 0 3px var(--line);
    animation: drop 0.18s steps(4) both;
  }
  :global(:root[data-theme="paper"]) .panel {
    background: var(--panel);
    border: 1.5px solid var(--line);
    border-radius: var(--radius);
    box-shadow: 8px 8px 0 color-mix(in srgb, var(--line) 85%, transparent);
    animation: flash 0.26s steps(1) both;
  }
  @keyframes drop {
    from {
      clip-path: inset(0 0 100% 0);
    }
  }
  @keyframes flash {
    0% {
      filter: invert(1);
    }
    40% {
      filter: none;
      opacity: 0.6;
    }
  }
  section {
    display: flex;
    flex-direction: column;
    gap: var(--s3);
  }
  .sh {
    display: flex;
    align-items: center;
    gap: var(--s2);
    color: var(--muted);
  }
  .sh .n {
    margin-left: auto;
    color: var(--faint);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: var(--s2);
  }
  .hit {
    display: flex;
    align-items: center;
    gap: var(--s3);
    height: 72px;
    padding: var(--s2);
    text-align: left;
    min-width: 0;
    animation: rise 0.3s var(--ease) both;
    animation-delay: calc(var(--d, 0) * 40ms);
  }
  :global(:root[data-theme="pixel"]) .hit {
    animation-timing-function: steps(3);
  }
  .th {
    position: relative;
    flex: none;
    height: 56px;
    overflow: hidden;
    background: var(--panel);
  }
  .th.cover {
    width: 42px;
  }
  .th.pic {
    width: 84px;
  }
  .th.light {
    background: #fff;
  }
  :global(:root[data-theme="pixel"]) .th {
    outline: 1px solid var(--faint);
  }
  :global(:root[data-theme="paper"]) .th {
    box-shadow: 0 0 0 1px var(--line);
  }
  .tx {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .nm {
    font-family: var(--title);
    font-weight: 700;
    font-size: 16px;
    line-height: 1.2;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .mt {
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .go,
  .have {
    flex: none;
    width: 24px;
    display: grid;
    place-items: center;
  }
  .go {
    opacity: 0;
  }
  .hit:hover .go,
  .hit:focus-visible .go,
  .hit.top .go {
    opacity: 1;
  }

  /* pixel: hovered/highlighted results invert, like the rest of the pixel UI */
  :global(:root[data-theme="pixel"]) .hit:hover,
  :global(:root[data-theme="pixel"]) .hit.top {
    background: var(--hi-bg);
    color: var(--hi-fg);
  }
  :global(:root[data-theme="pixel"]) .hit:is(:hover, .top, :focus-visible) .mt {
    color: inherit;
  }
  /* paper: a ruled index card that lifts */
  :global(:root[data-theme="paper"]) .hit {
    border-radius: var(--radius);
    transition:
      background 0.15s,
      box-shadow 0.2s var(--ease);
  }
  :global(:root[data-theme="paper"]) .hit:hover,
  :global(:root[data-theme="paper"]) .hit.top {
    background: var(--bg);
    box-shadow: inset 0 0 0 1.5px var(--line);
  }
  :global(:root[data-theme="paper"]) .hit:focus-visible {
    background: var(--bg);
    box-shadow:
      inset 0 0 0 2px var(--line),
      3px 3px 0 var(--line);
  }
  :global(:root[data-theme="paper"]) .hit:focus-visible::after {
    display: none;
  }
  :global(:root[data-theme="paper"]) .hit:is(:hover, .top, :focus-visible) .nm {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .hit:is(:hover, .top, :focus-visible) :global(.art) {
    filter: none;
  }
  :global(:root[data-theme="paper"]) .have {
    color: var(--accent-2);
  }
  .none {
    display: flex;
    align-items: center;
    gap: var(--s4);
    padding: var(--s3) var(--s2);
    color: var(--faint);
  }
  .none p {
    margin: 0;
    font-size: 17px;
  }
</style>
