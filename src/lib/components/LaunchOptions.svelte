<script lang="ts">
  import { untrack } from "svelte";
  import { api, type Emulator, type Game } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import Glyph from "./Glyph.svelte";

  /** The command-line options one emulated game starts with. */
  let { game, emu }: { game: Game; emu: Emulator } = $props();

  const join = (args: string[]) => args.map((a) => (a === "" || /[\s"]/.test(a) ? `"${a.replace(/"/g, '\\"')}"` : a)).join(" ");
  function split(line: string): string[] {
    const out: string[] = [];
    let cur = "";
    let quoted = false;
    let has = false;
    for (let i = 0; i < line.length; i++) {
      const ch = line[i];
      if (ch === "\\" && line[i + 1] === '"') {
        cur += '"';
        has = true;
        i++;
      } else if (ch === '"') {
        quoted = !quoted;
        has = true;
      } else if (/\s/.test(ch) && !quoted) {
        if (has) out.push(cur);
        cur = "";
        has = false;
      } else {
        cur += ch;
        has = true;
      }
    }
    if (has) out.push(cur);
    return out;
  }

  const custom = $derived(!!game.userArgs?.length);
  const current = $derived(join(game.userArgs?.length ? game.userArgs : emu.args));
  const emuLine = $derived(join(emu.args));
  let line = $state("");
  $effect(() => {
    const c = current;
    untrack(() => (line = c));
  });
  const dirty = $derived(join(split(line)) !== current);
  const noRom = $derived(!line.includes("{rom}"));
  let saving = $state(false);

  async function save() {
    if (!dirty || saving) return;
    saving = true;
    const args = split(line);
    try {
      await api.setGameArgs(game.id, args.length && join(args) !== emuLine ? args : null);
      await app.reload();
      app.toast(args.length && join(args) !== emuLine ? `${game.title} starts with these options now` : `${game.title} uses ${emu.name}'s options`, "ok");
    } catch (e) {
      app.toast(String(e), "error");
    }
    saving = false;
  }
  async function reset() {
    try {
      await api.setGameArgs(game.id, null);
      await app.reload();
      app.toast(`${game.title} uses ${emu.name}'s options again`, "ok");
    } catch (e) {
      app.toast(String(e), "error");
    }
  }
</script>

<section class="opts">
  <div class="sh">
    <Glyph name="sliders" size={24} />
    <h2 class="label">Launch options</h2>
    <span class="rule"></span>
    <span class="meta label" class:mine={custom}>{custom ? "Just this game" : `Same as ${emu.name}`}</span>
  </div>

  <div class="field" class:dirty>
    <input
      bind:value={line}
      data-nav
      spellcheck="false"
      autocomplete="off"
      aria-label="Launch options for {game.title}"
      onkeydown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          save();
        }
      }}
    />
  </div>
  {#if noRom}
    <p class="warn"><Glyph name="alert" size={12} />Without {"{rom}"} {emu.name} won't know which game to open.</p>
  {/if}

  <dl class="ref">
    <dt class="label">{emu.name} uses</dt>
    <dd><code>{emuLine || "nothing"}</code></dd>
    <dt class="label">Stand-ins</dt>
    <dd class="subs">
      <span><code>{"{rom}"}</code> the game file</span>
      <span><code>{"{rom_dir}"}</code> the game's folder</span>
      <span><code>{"{emu_dir}"}</code> {emu.name}'s folder</span>
    </dd>
  </dl>

  {#if dirty || custom}
    <div class="acts">
      {#if dirty}
        <button class="btn primary" data-nav onclick={save} disabled={saving}><Glyph name="check" size={24} />Save</button>
        <button class="btn" data-nav onclick={() => (line = current)}><Glyph name="close" size={24} />Cancel</button>
      {:else}
        <button class="btn" data-nav onclick={reset}><Glyph name="undo" size={24} />Use {emu.name}'s</button>
      {/if}
    </div>
  {/if}
</section>

<style>
  .field input {
    width: 100%;
    height: 48px;
    padding: 0 14px;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 15px;
    background: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .field input {
    font-family: var(--label);
    font-size: 13px;
    border: var(--border) solid var(--line);
  }
  :global(:root[data-theme="pixel"]) .field input:focus {
    outline: 2px dashed var(--fg);
    outline-offset: 4px;
  }
  :global(:root[data-theme="paper"]) .field input {
    background: var(--panel);
    border: 1.5px solid var(--line);
    border-radius: var(--radius);
  }
  :global(:root[data-theme="paper"]) .field input:focus {
    box-shadow: 4px 4px 0 var(--fg);
  }
  :global(:root[data-theme="paper"]) .dirty input {
    border-color: var(--accent-2);
  }
  .warn {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 10px 0 0;
    font-family: var(--label);
    font-size: 13px;
  }
  :global(:root[data-theme="paper"]) .warn {
    color: var(--accent);
  }
  .ref {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 8px 16px;
    margin: 16px 0 0;
    font-family: var(--label);
    font-size: 13px;
    line-height: 1.6;
  }
  .ref dt {
    color: var(--faint);
    font-weight: 400;
    line-height: 1.6;
  }
  .ref dd {
    margin: 0;
    color: var(--muted);
    min-width: 0;
    overflow-wrap: anywhere;
  }
  code {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 13px;
    color: var(--fg);
  }
  :global(:root[data-theme="pixel"]) code {
    font-family: var(--label);
    font-size: 12px;
  }
  :global(:root[data-theme="paper"]) code {
    background: color-mix(in srgb, var(--fg) 6%, transparent);
    padding: 1px 4px;
  }
  .subs {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .acts {
    display: flex;
    gap: 10px;
    margin-top: 18px;
  }
  .btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .meta.mine {
    color: var(--fg);
  }
  :global(:root[data-theme="paper"]) .meta.mine {
    color: var(--accent-2);
  }
</style>
