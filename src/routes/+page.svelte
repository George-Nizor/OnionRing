<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api, type Emulator } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { focusFirst, move, startGamepad, type PadAction } from "$lib/nav";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import HintBar from "$lib/components/HintBar.svelte";
  import Menu from "$lib/components/Menu.svelte";
  import Search from "$lib/components/Search.svelte";
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

  let emulators = $state<Emulator[]>([]);
  let portCount = $state(0);
  let setup = $state(false);
  let pad = $state(false);
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
    api.catalog().then((c) => (portCount = c.ports.length - c.installed.filter((i) => i.kind === "port").length)).catch(() => {});
    await tick();
    focusFirst(main);
  }

  onMount(() => {
    const stop = startGamepad();
    const onConnect = () => (pad = true);
    const onPad = (e: Event) => {
      pad = true;
      const a = (e as CustomEvent<PadAction>).detail;
      if (a !== "lb" && a !== "rb") act(a);
    };
    window.addEventListener("gamepadconnected", onConnect);
    window.addEventListener("pad", onPad);
    boot();
    return () => {
      stop();
      window.removeEventListener("gamepadconnected", onConnect);
      window.removeEventListener("pad", onPad);
    };
  });

  // New view: put focus on its first control, so a controller always has somewhere to start.
  $effect(() => {
    void app.view;
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
      case "b":
        app.back();
        return true;
      case "x": {
        const id = Number(el?.dataset.gameId);
        const g = app.games.find((x) => x.id === id);
        if (g) app.play(g);
        return !!g;
      }
      case "y":
        app.searchOpen = !app.searchOpen;
        app.menuOpen = false;
        return true;
      case "start": case "select": case "menu":
        app.menuOpen = !app.menuOpen;
        app.searchOpen = false;
        return true;
    }
    return false;
  }

  function onkey(e: KeyboardEvent) {
    const typing = (e.target as HTMLElement)?.closest("input, textarea, select");
    const keys: Record<string, string> = { ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right", Escape: "b" };
    if (typing) {
      if (e.key === "Escape") {
        (e.target as HTMLElement).blur();
        act("b");
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
</script>

<svelte:window onkeydown={onkey} />

<div class="shell">
  {#if app.loaded && !setup}
    <StatusBar />
    <main bind:this={main}>
      {#key app.view.name + (app.view.gameId ?? "")}
        <div class="view">
          {#if app.view.name === "home"}
            <Home {emulators} {portCount} />
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
          {:else}
            <Settings />
          {/if}
        </div>
      {/key}
    </main>
    <HintBar {pad} />
    <Transition />
    <Search />
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
