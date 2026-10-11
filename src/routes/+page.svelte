<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api, type BrowserGame, type Emulator } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { focusFirst, move, startGamepad, type Dir, type PadAction } from "$lib/nav";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import Menu from "$lib/components/Menu.svelte";
  import Transition from "$lib/components/Transition.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import Home from "$lib/components/Home.svelte";
  import Library from "$lib/components/Library.svelte";
  import GamePage from "$lib/components/GamePage.svelte";
  import GetMore from "$lib/components/GetMore.svelte";
  import Systems from "$lib/components/Systems.svelte";
  import Tidy from "$lib/components/Tidy.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import Setup from "$lib/components/Setup.svelte";
  import Stats from "$lib/components/Stats.svelte";

  let emulators = $state<Emulator[]>([]);
  let portCount = $state(0);
  let browser = $state<BrowserGame[]>([]);
  let setup = $state(false);
  let main = $state<HTMLElement>();

  async function loadEmulators() {
    emulators = await api.emulators().catch(() => []);
  }

  async function boot() {
    const data = import.meta.env.VITE_ONIONRING_DATA as string | undefined;
    if (import.meta.env.DEV && data && !("__TAURI_INTERNALS__" in window)) {
      const { installMock } = await import("$lib/mock");
      await installMock(data);
      (window as unknown as { onionring: typeof app }).onionring = app; // poke at state from devtools
    }
    await app.init();
    setup = !app.boot?.dataRoot;
    if (setup) return;
    await loadEmulators();
    api.catalog()
      .then((c) => {
        portCount = c.ports.length - c.installed.filter((i) => i.kind === "port").length;
        browser = c.browser;
      })
      .catch(() => {});
    await tick();
    focusFirst(main);
  }

  onMount(() => {
    const stop = startGamepad();
    const onPad = (e: Event) => {
      const a = (e as CustomEvent<PadAction>).detail;
      if (a !== "lb" && a !== "rb") act(a);
    };
    window.addEventListener("pad", onPad);
    boot();
    return () => {
      stop();
      window.removeEventListener("pad", onPad);
    };
  });

  // New view: put focus on its first control, so a controller always has somewhere to start.
  // Switching tabs inside a view keeps focus where it is (on the tab just picked).
  const screen = $derived(`${app.view.name}:${app.view.gameId ?? ""}`);
  $effect(() => {
    void screen;
    tick().then(() => focusFirst(main));
  });
  // The library can change on any scan; keep the emulator list in step.
  $effect(() => {
    void app.games;
    if (app.boot?.dataRoot) loadEmulators();
  });

  function act(a: PadAction | string) {
    const el = document.activeElement as HTMLElement | null;
    switch (a) {
      case "up": case "down": case "left": case "right":
        move(a);
        return true;
      case "a":
        el?.click();
        return true;
      case "b": {
        const fromSearch = app.searchOpen;
        app.back();
        if (fromSearch) tick().then(() => focusFirst(main));
        return true;
      }
      case "x": {
        const id = Number(el?.dataset.gameId);
        const g = app.games.find((x) => x.id === id);
        if (g) app.play(g);
        return !!g;
      }
      case "y":
        app.openSearch();
        return true;
      case "start": case "select": case "menu":
        app.menuOpen = !app.menuOpen;
        app.searchOpen = false;
        return true;
    }
    return false;
  }

  function leavesField(f: HTMLInputElement, key: string): boolean {
    if (key === "ArrowUp" || key === "ArrowDown") return true;
    const { selectionStart: a, selectionEnd: b } = f;
    if (a === null || b === null) return true; // fields without a caret
    if (a !== b) return false;
    return key === "ArrowLeft" ? a === 0 : a === f.value.length;
  }

  function onkey(e: KeyboardEvent) {
    const typing = (e.target as HTMLElement)?.closest("input, textarea, select");
    const keys: Record<string, string> = { ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right", Escape: "b" };
    if (typing) {
      if (e.defaultPrevented) return; // the field handled it (search's own keys)
      const field = e.target as HTMLInputElement;
      if (e.ctrlKey && e.key.toLowerCase() === "k") {
        app.openSearch();
        e.preventDefault();
      } else if (e.key === "Escape") {
        // Leave the field; go back only from an empty one, so Escape never throws away typing.
        const empty = !("value" in field) || !field.value;
        field.blur();
        if (empty) act("b");
        e.preventDefault();
      } else if (field.tagName === "INPUT" && keys[e.key] && leavesField(field, e.key) && move(keys[e.key] as Dir)) {
        // Arrows move on from a one-line field: up and down always, left and right from its ends.
        e.preventDefault();
      }
      return;
    }
    let a = keys[e.key];
    if (!a && e.key === "Backspace") a = "b";
    if (!a && e.key === " ") a = "x";
    if (!a && (e.key === "/" || (e.ctrlKey && e.key.toLowerCase() === "k"))) a = "y";
    if (!a && e.key.toLowerCase() === "m") a = "menu";
    if (a && act(a)) e.preventDefault();
  }

  // The mouse's back button goes back, like a browser.
  function onmouseup(e: MouseEvent) {
    if (e.button === 3) {
      e.preventDefault();
      act("b");
    }
  }
</script>

<svelte:window onkeydown={onkey} {onmouseup} />

<div class="shell">
  {#if app.loaded && !setup}
    <StatusBar />
    <main bind:this={main}>
      {#key app.view.name + (app.view.gameId ?? "") + (app.view.item ?? "")}
        <div class="view">
          {#if app.view.name === "home"}
            <Home {emulators} {portCount} {browser} />
          {:else if app.view.name === "library"}
            <Library />
          {:else if app.view.name === "game"}
            <GamePage {emulators} />
          {:else if app.view.name === "get"}
            <GetMore {emulators} />
          {:else if app.view.name === "systems"}
            <Systems {emulators} onchange={loadEmulators} />
          {:else if app.view.name === "tidy"}
            <Tidy />
          {:else if app.view.name === "stats"}
            <Stats />
          {:else}
            <Settings />
          {/if}
        </div>
      {/key}
    </main>
    <Transition />
    <Menu />
  {/if}
  {#if setup}
    <Setup ondone={() => { setup = false; loadEmulators(); app.go({ name: "home" }); }} />
  {/if}
  <Toasts />
</div>

<style>
  .shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  main {
    flex: 1;
    min-height: 0;
    position: relative;
  }
  .view {
    position: absolute;
    inset: 0;
  }
</style>
