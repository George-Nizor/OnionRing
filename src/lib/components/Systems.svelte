<script lang="ts">
  import { api, type Emulator } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import Glyph from "./Glyph.svelte";

  let { emulators, onchange }: { emulators: Emulator[]; onchange: () => void } = $props();

  // One row per platform you have games for, plus any platform an emulator covers.
  const rows = $derived.by(() => {
    const ids = new Set<string>([...app.counts.keys()].filter((p) => p !== "port"));
    for (const e of emulators) for (const p of e.platforms) ids.add(p);
    return [...ids]
      .map((id) => ({ p: app.platforms.get(id), id, n: app.counts.get(id) ?? 0, emus: emulators.filter((e) => e.platforms.includes(id)) }))
      .filter((r) => r.p)
      .sort((a, b) => b.n - a.n);
  });

  let editing = $state<string | null>(null);
  let text = $state("");
  const join = (a: string[]) => a.map((x) => (/\s/.test(x) ? `"${x}"` : x)).join(" ");
  const split = (s: string) => [...s.matchAll(/"([^"]*)"|(\S+)/g)].map((m) => m[1] ?? m[2]);
  async function saveArgs(e: Emulator) {
    await api.setEmulatorArgs(e.id, split(text));
    editing = null;
    onchange();
    app.toast(`${e.name} launch options saved`, "ok");
  }
</script>

<div class="sys">
  {#each rows as r, i (r.id)}
    <section class="row" style:--i={i}>
      <div class="plat">
        <span class="short">{r.p!.short.replace("WII", "Wii")}</span>
        <span class="label"><Glyph name="library" size={12} />{r.n}</span>
      </div>
      <div class="emus">
        {#each r.emus as e (e.id)}
          <div class="emu">
            <Glyph name={e.missing ? "alert" : "check"} size={24} />
            <span class="name">{e.name}</span>
            {#if e.version}<span class="ver label">{e.version}</span>{/if}
            <button class="btn icon" data-nav onclick={() => api.reveal(e.exe)} aria-label="Show {e.name} in folder"><Glyph name="folder" size={24} /></button>
            <button class="btn icon" data-nav onclick={() => { editing = editing === e.id ? null : e.id; text = join(e.args); }} aria-label="Launch options"><Glyph name="sliders" size={24} /></button>
          </div>
          {#if editing === e.id}
            <div class="args">
              <input class="field" data-nav bind:value={text} spellcheck="false" aria-label="Launch options" />
              <button class="btn" data-nav onclick={() => saveArgs(e)}><Glyph name="check" size={24} />Save</button>
              <button class="btn" data-nav onclick={async () => { await api.setEmulatorArgs(e.id, null); editing = null; onchange(); }}><Glyph name="undo" size={24} />Default</button>
            </div>
          {/if}
          {#if e.notes}<p class="note"><Glyph name="info" size={24} /><span>{e.notes}</span></p>{/if}
        {:else}
          <div class="emu none">
            <Glyph name="alert" size={24} /><span class="name">No emulator yet</span>
            <button class="btn" data-nav onclick={() => app.go({ name: "get", tab: "emulators" })}><Glyph name="get" size={24} />Get one</button>
          </div>
        {/each}
      </div>
    </section>
  {/each}
</div>

<style>
  .sys {
    height: 100%;
    overflow-y: auto;
    padding: 24px 56px 60px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .row {
    display: flex;
    gap: 30px;
    padding: 18px 20px;
    animation: rise 0.4s var(--ease) both;
    animation-delay: calc(var(--i) * 40ms);
  }
  :global(:root[data-theme="pixel"]) .row {
    border: var(--border) solid var(--line);
    animation-timing-function: steps(3);
  }
  :global(:root[data-theme="paper"]) .row {
    background: var(--panel);
    box-shadow: 0 0 0 1px var(--line);
  }
  .plat {
    width: 130px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .plat .label {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .short {
    font-family: var(--title);
    font-weight: 800;
    font-size: 44px;
    line-height: 1;
  }
  .emus {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 10px;
    justify-content: center;
  }
  .emu {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .name {
    font-family: var(--title);
    font-weight: 700;
    font-size: 22px;
  }
  .ver {
    color: var(--faint);
  }
  .emu .btn:first-of-type {
    margin-left: auto;
  }
  .none .btn {
    margin-left: auto;
  }
  .args {
    display: flex;
    gap: 10px;
  }
  .field {
    flex: 1;
    height: 48px;
    padding: 0 14px;
    font-family: "Cascadia Mono", Consolas, monospace;
    font-size: 15px;
    background: var(--bg);
    border: var(--border) solid var(--line);
  }
  .note {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0;
    font-family: var(--label);
    font-size: 14px;
    color: var(--muted);
  }
  :global(:root[data-theme="paper"]) .note {
    color: var(--accent);
  }
</style>
