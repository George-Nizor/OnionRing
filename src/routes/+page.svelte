<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Emulator } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import { startGamepad } from "$lib/gamepad";
  import TopBar from "$lib/components/TopBar.svelte";
  import Home from "$lib/components/Home.svelte";
  import Ports from "$lib/components/Ports.svelte";
  import Emulators from "$lib/components/Emulators.svelte";
  import Storage from "$lib/components/Storage.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import Setup from "$lib/components/Setup.svelte";
  import Toasts from "$lib/components/Toasts.svelte";

  let searchEl = $state<HTMLInputElement>();
  let emulators = $state<Emulator[]>([]);
  let reclaimable = $state(0);
  let updates = $state(0);
  let setup = $state(false);

  async function loadEmulators() {
    emulators = await api.emulators().catch(() => []);
  }

  async function boot() {
    const data = import.meta.env.VITE_ONIONRING_DATA as string | undefined;
    if (import.meta.env.DEV && data && !("__TAURI_INTERNALS__" in window)) {
      const { installMock } = await import("$lib/mock");
      await installMock(data);
    }
    await app.init();
  }

  onMount(() => {
    const stopPad = startGamepad();
    boot().then(async () => {
      setup = !app.boot?.dataRoot;
      if (setup) return;
      await loadEmulators();
      api.storage().then((r) => (reclaimable = r.reclaimable)).catch(() => {});
      api.checkUpdates().then((u) => (updates = u.filter((x) => x.available).length)).catch(() => {});
    });
    return stopPad;
  });

  // Emulator list follows library changes (scans can find new ones).
  $effect(() => {
    void app.games;
    if (app.boot?.dataRoot) loadEmulators();
  });
</script>

<div class="shell">
  {#if app.loaded && !setup}
    {#if app.section === "library"}
      <Home {emulators} {searchEl} />
    {:else}
      <main>
        {#key app.section}
          <div class="view">
            {#if app.section === "ports"}
              <Ports onupdates={(n) => (updates = n)} />
            {:else if app.section === "emulators"}
              <Emulators {emulators} onchange={loadEmulators} />
            {:else if app.section === "storage"}
              <Storage onchange={(r) => (reclaimable = r)} />
            {:else}
              <Settings />
            {/if}
          </div>
        {/key}
      </main>
    {/if}
  {/if}
  <div class="top"><TopBar bind:searchEl {reclaimable} {updates} /></div>
  {#if setup}
    <Setup ondone={() => { setup = false; loadEmulators(); }} />
  {/if}
  <Toasts />
</div>

<style>
  .shell {
    position: relative;
    height: 100vh;
    overflow: hidden;
    background:
      radial-gradient(1200px 700px at 100% 0%, rgba(157, 107, 255, 0.08), transparent 60%),
      radial-gradient(900px 600px at 0% 100%, rgba(255, 93, 122, 0.06), transparent 60%),
      var(--bg);
  }
  .top {
    position: absolute;
    inset: 0 0 auto 0;
    z-index: 30;
  }
  main {
    position: absolute;
    inset: 60px 0 0 0;
    overflow-y: auto;
  }
  .view {
    animation: rise 0.4s var(--ease) both;
  }
</style>
