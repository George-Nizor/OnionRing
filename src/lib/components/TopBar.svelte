<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { app, type Filter, type Section } from "$lib/store.svelte";
  import Icon from "./Icon.svelte";
  import Logo from "./Logo.svelte";

  let { searchEl = $bindable(), reclaimable = 0, updates = 0 }: { searchEl?: HTMLInputElement; reclaimable?: number; updates?: number } =
    $props();

  // Resolved lazily so the browser preview can install its mock first.
  const win = () => getCurrentWindow();
  const busy = $derived(app.progress !== null);
  const home = $derived(app.section === "library");

  // Short, readable tab labels: "GameCube", "Wii", "PS3", "3DS".
  const label = (name: string, short: string) => (name.length <= 9 ? name : short.length <= 4 ? short.replace("WII", "Wii") : name);

  const tabs = $derived.by(() => {
    const t: { f: Filter; label: string }[] = [{ f: "all", label: "All" }];
    if (app.games.some((g) => g.lastPlayed)) t.push({ f: "recent", label: "Recent" });
    if (app.games.some((g) => g.favorite)) t.push({ f: "favorites", label: "Favourites" });
    const plats = [...app.counts.keys()]
      .filter((id) => id !== "port")
      .map((id) => app.platforms.get(id))
      .filter((p) => !!p)
      .sort((a, b) => a!.name.localeCompare(b!.name));
    for (const p of plats) t.push({ f: `platform:${p!.id}`, label: label(p!.name, p!.short) });
    if (app.counts.get("port")) t.push({ f: "ports", label: "Ports" });
    return t;
  });

  let searching = $state(false);
  $effect(() => {
    if (app.query) searching = true;
  });

  const go = (s: Section) => (app.section = s);
  const nav: { s: Section; icon: string; label: string }[] = [
    { s: "ports", icon: "box", label: "Ports & recomps" },
    { s: "emulators", icon: "pad", label: "Emulators" },
    { s: "storage", icon: "disk", label: "Storage" },
    { s: "settings", icon: "gear", label: "Settings" },
  ];
</script>

<header class="bar" class:home data-tauri-drag-region>
  <button class="brand" onclick={() => app.setFilter(app.filter)} data-tauri-drag-region>
    <Logo size={24} {busy} />
    <span class="word">OnionRing</span>
  </button>

  <nav class="tabs" data-tauri-drag-region>
    {#if app.boot?.dataRoot}
      {#each tabs as t (t.f)}
        <button class="tab" class:on={home && app.filter === t.f} onclick={() => app.setFilter(t.f)}>{t.label}</button>
      {/each}
    {/if}
  </nav>

  <div class="actions" data-tauri-drag-region>
    {#if app.boot?.dataRoot}
      <label class="search" class:open={searching || app.query}>
        <button class="ic" onclick={() => { searching = true; app.section = "library"; setTimeout(() => searchEl?.focus()); }} aria-label="Search">
          <Icon name="search" size={18} />
        </button>
        <input
          bind:this={searchEl}
          bind:value={app.query}
          placeholder="Search"
          spellcheck="false"
          onfocus={() => (searching = true)}
          onblur={() => !app.query && (searching = false)}
          onkeydown={(e) => {
            if (e.key === "Escape") {
              app.query = "";
              (e.target as HTMLInputElement).blur();
            }
          }}
        />
      </label>
      {#each nav as n (n.s)}
        <button class="ic" class:on={app.section === n.s} onclick={() => go(n.s)} title={n.label} aria-label={n.label}>
          <Icon name={n.icon} size={19} />
          {#if n.s === "ports" && updates}<span class="pip"></span>{/if}
          {#if n.s === "storage" && reclaimable > 5e9}<span class="pip"></span>{/if}
        </button>
      {/each}
    {/if}
    <span class="gap"></span>
    <button class="win" onclick={() => win().minimize()} aria-label="Minimise"><Icon name="min" size={15} /></button>
    <button class="win" onclick={() => win().toggleMaximize()} aria-label="Maximise"><Icon name="max" size={13} /></button>
    <button class="win close" onclick={() => win().close()} aria-label="Close"><Icon name="close" size={15} /></button>
  </div>
</header>

<style>
  .bar {
    position: relative;
    z-index: 30;
    height: 60px;
    flex: none;
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    padding-left: 26px;
    background: linear-gradient(180deg, rgba(5, 4, 8, 0.55), rgba(5, 4, 8, 0));
  }
  .bar:not(.home) {
    background: rgba(9, 8, 13, 0.9);
    border-bottom: 1px solid var(--line);
  }
  .brand {
    justify-self: start;
    display: flex;
    align-items: center;
    gap: 11px;
  }
  .word {
    font-family: var(--brand);
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.005em;
  }
  .tabs {
    display: flex;
    align-items: center;
    gap: 4px;
    max-width: calc(100vw - 600px);
    overflow-x: auto;
    scrollbar-width: none;
  }
  .tab {
    position: relative;
    height: 36px;
    padding: 0 14px;
    border-radius: 99px;
    font-size: 14.5px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.58);
    white-space: nowrap;
    transition:
      color 0.2s,
      background 0.2s;
  }
  .tab:hover {
    color: #fff;
  }
  .tab.on {
    color: #0b0a10;
    background: #fff;
  }
  .actions {
    justify-self: end;
    display: flex;
    align-items: center;
    height: 100%;
    gap: 2px;
  }
  .ic {
    position: relative;
    width: 40px;
    height: 40px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: rgba(255, 255, 255, 0.72);
    transition:
      background 0.15s,
      color 0.15s;
  }
  .ic:hover,
  .ic.on {
    color: #fff;
    background: var(--glass);
  }
  .pip {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--coral);
    box-shadow: 0 0 0 2px #0b0a10;
  }
  .search {
    display: flex;
    align-items: center;
    height: 40px;
    border-radius: 99px;
    transition:
      background 0.25s,
      width 0.3s var(--ease);
    width: 40px;
    overflow: hidden;
  }
  .search.open {
    width: 260px;
    background: var(--glass);
    box-shadow: inset 0 0 0 1px var(--glass-line);
  }
  .search .ic {
    flex: none;
  }
  .search .ic:hover {
    background: none;
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: none;
    color: #fff;
    font-size: 14px;
    padding-right: 14px;
  }
  .search input::placeholder {
    color: rgba(255, 255, 255, 0.45);
  }
  .gap {
    width: 14px;
  }
  .win {
    width: 46px;
    height: 60px;
    display: grid;
    place-items: center;
    color: rgba(255, 255, 255, 0.6);
    transition: background 0.15s;
  }
  .win:hover {
    background: var(--glass);
    color: #fff;
  }
  .win.close:hover {
    background: #e81123;
    color: #fff;
  }
  @media (max-width: 1240px) {
    .word {
      display: none;
    }
    .tabs {
      max-width: calc(100vw - 470px);
    }
    .tab {
      padding: 0 11px;
      font-size: 14px;
    }
  }
</style>
