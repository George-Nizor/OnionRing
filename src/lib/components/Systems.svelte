<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type AvailableUpdate, type CheckResult, type Emulator } from "$lib/api";
  import { app } from "$lib/store.svelte";
  import Glyph from "./Glyph.svelte";
  import SystemArt from "./SystemArt.svelte";

  let { emulators, onchange }: { emulators: Emulator[]; onchange: () => void } = $props();

  let checks = $state(new Map<string, CheckResult[]>());
  let updates = $state(new Map<string, AvailableUpdate>());
  // Re-check whenever the emulator list changes (scan, install, launch options).
  $effect(() => {
    void emulators;
    api.setupChecks()
      .then((c) => (checks = new Map(c.map((x) => [x.id, x.checks]))))
      .catch(() => {});
    api.availableUpdates()
      .then((u) => (updates = new Map(u.filter((x) => x.kind === "emulator").map((x) => [x.id, x]))))
      .catch(() => {});
  });
  const upFor = (e: Emulator) => updates.get(e.id) ?? updates.get(e.id.replace(/-\d+$/, ""));
  const needsFiles = (e: Emulator) => (checks.get(e.id) ?? []).some((c) => !c.ok && !c.optional);

  type Status = "ready" | "setup" | "none";
  // One card per platform you have games for, any platform an emulator covers, and every
  // platform OnionRing can get an emulator for (so you can set up a console before adding games).
  const rows = $derived.by(() => {
    const ids = new Set<string>([...app.counts.keys()].filter((p) => p !== "port" && p !== "mashup"));
    for (const e of emulators) for (const p of e.platforms) ids.add(p);
    for (const e of app.catalog?.emulators ?? []) for (const p of e.platforms) ids.add(p);
    return [...ids]
      .map((id) => {
        const emus = emulators.filter((e) => e.platforms.includes(id));
        const status: Status = !emus.length ? "none" : emus.some((e) => !e.missing && !needsFiles(e)) ? "ready" : "setup";
        return { p: app.platforms.get(id)!, id, n: app.counts.get(id) ?? 0, emus, status };
      })
      .filter((r) => r.p)
      .sort((a, b) => b.n - a.n || a.p.name.localeCompare(b.p.name));
  });
  const STATUS: Record<Status, [string, string]> = {
    ready: ["check", "Ready"],
    setup: ["alert", "Needs setting up"],
    none: ["get", "No emulator"],
  };
  const count = (s: Status) => rows.filter((r) => r.status === s).length;
  const ups = $derived(emulators.filter((e) => upFor(e)).length);

  let editing = $state<string | null>(null);
  let text = $state("");
  const join = (a: string[]) => a.map((x) => (/\s/.test(x) ? `"${x}"` : x)).join(" ");
  const split = (s: string) => [...s.matchAll(/"([^"]*)"|(\S+)/g)].map((m) => m[1] ?? m[2]);
  const custom = (e: Emulator) => join(e.args) !== join(e.defaultArgs);
  async function saveArgs(e: Emulator) {
    await api.setEmulatorArgs(e.id, split(text));
    editing = null;
    onchange();
    app.toast(`${e.name} launch options saved`, "ok");
  }
  async function resetArgs(e: Emulator) {
    await api.setEmulatorArgs(e.id, null);
    editing = null;
    onchange();
    app.toast(`${e.name} is back to its usual launch options`, "ok");
  }
  function update(u: AvailableUpdate) {
    app.installing = { ...app.installing, [u.id]: "Starting" };
    api.installLatest(u.id).catch(() => {});
  }
</script>

<div class="sys">
  <header class="summary">
    <span class="stat"><Glyph name="pad" size={24} /><b>{rows.length}</b> {rows.length === 1 ? "system" : "systems"}</span>
    <span class="stat"><Glyph name="check" size={24} /><b>{count("ready")}</b> ready to play</span>
    {#if count("setup")}<span class="stat warn"><Glyph name="alert" size={24} /><b>{count("setup")}</b> need setting up</span>{/if}
    {#if ups}
      <button class="stat link" data-nav onclick={() => app.go({ name: "get", tab: "updates" })}>
        <Glyph name="refresh" size={24} /><b>{ups}</b> {ups === 1 ? "update" : "updates"}
      </button>
    {/if}
  </header>

  <div class="grid">
    {#each rows as r, i (r.id)}
      <section class="plat" style:--i={i}>
        <div class="head">
          <span class="pa"><SystemArt id={r.id} label={r.p.name} /></span>
          <div class="title">
            <h2>{r.p.name}</h2>
            <span class="meta"><Glyph name="library" size={12} />{r.n} {r.n === 1 ? "game" : "games"}</span>
          </div>
          <span class="status {r.status}"><Glyph name={STATUS[r.status][0]} size={24} /><span>{STATUS[r.status][1]}</span></span>
        </div>

        <div class="emus">
          {#each r.emus as e (e.id)}
            {@const list = checks.get(e.id) ?? []}
            {@const up = upFor(e)}
            {@const busy = up ? app.installing[up.id] : undefined}
            <div class="emu">
              <div class="eh">
                <span class="mark" class:bad={e.missing || needsFiles(e)}><Glyph name={e.missing || needsFiles(e) ? "alert" : "check"} size={24} /></span>
                <div class="en">
                  <span class="name">{e.name}</span>
                  <span class="sub">{e.version ? `Version ${e.version}` : "Version not known"} · {e.managed ? "installed by OnionRing" : "found on this PC"}</span>
                </div>
                <div class="tools">
                  <button class="btn icon" class:on={editing === e.id} data-nav title="Launch options" aria-label="Launch options for {e.name}"
                    onclick={() => { editing = editing === e.id ? null : e.id; text = join(e.args); }}>
                    <Glyph name="sliders" size={24} />{#if custom(e)}<span class="dot" title="Changed from the usual"></span>{/if}
                  </button>
                  <button class="btn icon" data-nav title="Show in its folder" aria-label="Show {e.name} in its folder" onclick={() => api.reveal(e.exe)}><Glyph name="folder" size={24} /></button>
                </div>
              </div>

              {#if editing === e.id}
                <div class="args">
                  <span class="label">Launch options</span>
                  <div class="argrow">
                    <input class="field" data-nav bind:value={text} spellcheck="false" aria-label="Launch options"
                      onkeydown={(ev) => ev.key === "Enter" && saveArgs(e)} />
                    <button class="btn primary" data-nav onclick={() => saveArgs(e)}><Glyph name="check" size={24} />Save</button>
                    <button class="btn" data-nav onclick={() => resetArgs(e)} disabled={!custom(e)}><Glyph name="undo" size={24} />Usual</button>
                  </div>
                  <span class="hint">{"{rom}"} stands for the game file.</span>
                </div>
              {/if}

              {#if e.missing}
                <p class="line bad"><Glyph name="alert" size={24} /><span>OnionRing can't find this emulator any more. Get it again, or scan your folders.</span></p>
              {/if}

              {#if up}
                <div class="notice">
                  <Glyph name="refresh" size={24} />
                  <span>Version <b>{up.latest}</b> is out{up.managed ? "." : ". You set this copy up yourself, so update it from its download page."}</span>
                  {#if busy}
                    <span class="label">{busy}</span>
                  {:else if up.managed}
                    <button class="btn" data-nav onclick={() => update(up)}><Glyph name="refresh" size={24} />Update</button>
                  {:else if up.url}
                    <button class="btn" data-nav onclick={() => openUrl(up.url!)}><Glyph name="globe" size={24} />Download page</button>
                  {/if}
                </div>
              {/if}

              {#if list.length}
                <ul class="checks">
                  {#each list as c, j (c.what)}
                    {@const st = c.ok ? "ok" : c.optional ? "opt" : "miss"}
                    <li class={st} style:--j={j}>
                      <span class="box"><Glyph name={c.ok ? "check" : c.optional ? "plus" : "close"} size={24} /></span>
                      <div class="ct">
                        <span class="what">{c.what}{#if c.optional}<span class="tag">Optional</span>{/if}</span>
                        {#if !c.ok}<span class="help">{c.help}</span>{/if}
                      </div>
                      <span class="cs">{c.ok ? "Found" : c.optional ? "Not added" : "Missing"}</span>
                      {#if c.folder}
                        <button class="btn sm" data-nav onclick={() => api.openFolder(c.folder!)}><Glyph name="folder" size={24} />Open folder</button>
                      {:else}
                        <span></span>
                      {/if}
                    </li>
                  {/each}
                </ul>
              {/if}

              {#if e.notes}<p class="line note"><Glyph name="info" size={24} /><span>{e.notes}</span></p>{/if}
            </div>
          {:else}
            <div class="emu empty">
              <p class="line"><Glyph name="info" size={24} /><span>Nothing on this PC plays {r.p.name} games yet.</span></p>
              <button class="btn primary" data-nav onclick={() => app.go({ name: "get", tab: "emulators" })}><Glyph name="get" size={24} />Get an emulator</button>
            </div>
          {/each}
        </div>
      </section>
    {/each}
  </div>
</div>

<style>
  .sys {
    height: 100%;
    overflow-y: auto;
    padding: 24px 56px 64px;
  }

  /* ---- summary --------------------------------------------------------------------------- */
  .summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 32px;
    margin-bottom: 24px;
    min-height: 52px;
  }
  .stat {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 40px;
    font-family: var(--label);
    font-size: 15px;
    color: var(--muted);
  }
  .stat b {
    font-family: var(--title);
    font-size: 24px;
    font-weight: 800;
    color: var(--fg);
  }
  button.stat {
    padding: 0 12px;
    margin-left: -12px;
  }
  :global(:root[data-theme="pixel"]) .stat {
    text-transform: uppercase;
    font-size: 12px;
    letter-spacing: 0.06em;
  }
  :global(:root[data-theme="pixel"]) button.stat:hover {
    background: var(--hi-bg);
    color: var(--hi-fg);
  }
  :global(:root[data-theme="pixel"]) button.stat:is(:hover, :focus-visible) b {
    color: inherit;
  }
  :global(:root[data-theme="paper"]) .stat.warn :global(svg) {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) button.stat {
    color: var(--accent-2);
  }
  :global(:root[data-theme="paper"]) button.stat b {
    color: var(--accent-2);
  }
  :global(:root[data-theme="paper"]) button.stat:hover {
    text-decoration: underline;
    text-underline-offset: 4px;
  }

  /* ---- platform cards -------------------------------------------------------------------- */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 600px), 1fr));
    gap: 24px;
    align-items: start;
  }
  .plat {
    display: flex;
    flex-direction: column;
    min-width: 0;
    animation: rise 0.4s var(--ease) both;
    animation-delay: calc(var(--i) * 50ms);
  }
  .head {
    display: grid;
    grid-template-columns: 144px minmax(0, 1fr) auto;
    align-items: center;
    gap: 24px;
    padding: 16px 20px 16px 16px;
  }
  .pa {
    display: block;
    width: 144px;
    height: 96px;
  }
  .title {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .title h2 {
    margin: 0;
    font-family: var(--title);
    font-weight: 800;
    font-size: 27px;
    line-height: 1.08;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 8px;
    font-family: var(--label);
    font-size: 14px;
    color: var(--muted);
  }
  .status {
    align-self: start;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 12px 0 8px;
    font-family: var(--label);
    font-size: 13px;
    font-weight: 700;
    white-space: nowrap;
  }
  .emus {
    display: flex;
    flex-direction: column;
  }
  .emu {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 20px 18px;
  }
  .eh {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) auto;
    align-items: center;
    gap: 14px;
  }
  .mark {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
  }
  .en {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .name {
    font-family: var(--title);
    font-weight: 800;
    font-size: 22px;
    line-height: 1.15;
  }
  .sub {
    font-family: var(--label);
    font-size: 13px;
    color: var(--faint);
  }
  .tools {
    display: flex;
    gap: 8px;
  }
  .tools .btn {
    width: 44px;
    height: 44px;
  }
  .dot {
    position: absolute;
    top: 5px;
    right: 5px;
    width: 8px;
    height: 8px;
    background: currentColor;
  }

  .line {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr);
    align-items: start;
    gap: 12px;
    margin: 0;
    padding-left: 4px;
    font-size: 15px;
    line-height: 1.45;
    color: var(--muted);
  }
  .notice {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    padding: 10px 10px 10px 14px;
    font-size: 15px;
    line-height: 1.4;
  }

  .checks {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  .checks li {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 14px;
    min-height: 52px;
    padding: 8px 0;
  }
  .box {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    animation: pop 0.35s var(--ease) both;
    animation-delay: calc(0.25s + var(--i) * 50ms + var(--j) * 60ms);
  }
  .ct {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .what {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: var(--label);
    font-size: 15px;
    font-weight: 700;
  }
  .tag {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .help {
    font-size: 14px;
    line-height: 1.4;
    color: var(--muted);
  }
  .cs {
    font-family: var(--label);
    font-size: 13px;
    color: var(--faint);
  }
  .btn.sm {
    height: 40px;
    padding: 0 14px;
    font-size: 13px;
  }

  .args {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px;
  }
  .argrow {
    display: flex;
    gap: 8px;
  }
  .field {
    flex: 1;
    min-width: 0;
    height: 48px;
    padding: 0 14px;
    font-family: "Cascadia Mono", Consolas, monospace;
    font-size: 15px;
    background: var(--bg);
    border: var(--border) solid var(--line);
  }
  .hint {
    font-size: 13px;
    color: var(--faint);
  }
  .emu.empty {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .emu.empty .line {
    align-items: center;
  }

  /* ---- pixel ----------------------------------------------------------------------------- */
  :global(:root[data-theme="pixel"]) .plat {
    border: var(--border) solid var(--line);
    animation-timing-function: steps(3);
  }
  :global(:root[data-theme="pixel"]) .head {
    border-bottom: var(--border) solid var(--line);
  }
  :global(:root[data-theme="pixel"]) .title h2 {
    font-size: 26px;
  }
  :global(:root[data-theme="pixel"]) .meta,
  :global(:root[data-theme="pixel"]) .status,
  :global(:root[data-theme="pixel"]) .sub,
  :global(:root[data-theme="pixel"]) .cs {
    text-transform: uppercase;
    font-size: 11px;
    letter-spacing: 0.06em;
  }
  :global(:root[data-theme="pixel"]) .status {
    box-shadow: inset 0 0 0 2px var(--line);
  }
  :global(:root[data-theme="pixel"]) .status.ready {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .status.setup {
    animation: blink 1.2s steps(2) 3;
  }
  :global(:root[data-theme="pixel"]) .emu + .emu {
    border-top: 2px dashed var(--faint);
  }
  :global(:root[data-theme="pixel"]) .mark {
    box-shadow: inset 0 0 0 2px var(--line);
  }
  :global(:root[data-theme="pixel"]) .mark.bad {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .notice {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .notice .btn {
    border-color: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .notice .btn:is(:hover, :focus-visible) {
    background: var(--bg);
    color: var(--fg);
  }
  :global(:root[data-theme="pixel"]) .checks li + li {
    border-top: 2px dotted var(--faint);
  }
  :global(:root[data-theme="pixel"]) .box {
    box-shadow: inset 0 0 0 2px var(--line);
    animation-timing-function: steps(3);
  }
  :global(:root[data-theme="pixel"]) .ok .box {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="pixel"]) .opt .box {
    box-shadow: inset 0 0 0 2px var(--faint);
    color: var(--faint);
  }
  :global(:root[data-theme="pixel"]) .miss .box {
    animation:
      pop 0.35s steps(3) both,
      blink 1s steps(2) 0.6s 3;
  }
  :global(:root[data-theme="pixel"]) .what {
    text-transform: uppercase;
    font-size: 13px;
    letter-spacing: 0.04em;
  }
  :global(:root[data-theme="pixel"]) .args {
    border: 2px dashed var(--line);
  }
  :global(:root[data-theme="pixel"]) .sys .btn:hover,
  :global(:root[data-theme="pixel"]) .tools .btn.on {
    background: var(--hi-bg);
    color: var(--hi-fg);
  }

  /* ---- paper ----------------------------------------------------------------------------- */
  :global(:root[data-theme="paper"]) .plat {
    background: var(--panel);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--line) 22%, transparent);
  }
  :global(:root[data-theme="paper"]) .head {
    box-shadow: inset 0 -1px 0 color-mix(in srgb, var(--line) 18%, transparent);
  }
  :global(:root[data-theme="paper"]) .pa {
    color: var(--fg);
  }
  :global(:root[data-theme="paper"]) .status {
    border-radius: 16px;
    padding: 0 14px 0 10px;
  }
  :global(:root[data-theme="paper"]) .status.ready {
    color: var(--accent-2);
    background: color-mix(in srgb, var(--accent-2) 10%, var(--panel));
  }
  :global(:root[data-theme="paper"]) .status.setup {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 10%, var(--panel));
  }
  :global(:root[data-theme="paper"]) .status.none {
    color: color-mix(in srgb, var(--accent-3) 65%, var(--fg));
    background: color-mix(in srgb, var(--accent-3) 16%, var(--panel));
  }
  :global(:root[data-theme="paper"]) .emu + .emu {
    box-shadow: inset 0 1px 0 color-mix(in srgb, var(--line) 14%, transparent);
  }
  :global(:root[data-theme="paper"]) .mark {
    color: var(--accent-2);
  }
  :global(:root[data-theme="paper"]) .mark.bad,
  :global(:root[data-theme="paper"]) .line.bad {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .notice {
    background: color-mix(in srgb, var(--accent-2) 8%, var(--panel));
    box-shadow: inset 3px 0 0 var(--accent-2);
  }
  :global(:root[data-theme="paper"]) .notice > :global(svg) {
    color: var(--accent-2);
  }
  :global(:root[data-theme="paper"]) .checks li + li {
    border-top: 1px solid color-mix(in srgb, var(--line) 12%, transparent);
  }
  :global(:root[data-theme="paper"]) .box {
    border-radius: 50%;
    box-shadow: inset 0 0 0 1.5px currentColor;
  }
  :global(:root[data-theme="paper"]) .ok .box {
    color: var(--accent-2);
  }
  :global(:root[data-theme="paper"]) .miss .box,
  :global(:root[data-theme="paper"]) .miss .cs {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .opt .box {
    color: var(--faint);
    box-shadow: inset 0 0 0 1.5px currentColor;
    border: 0;
  }
  :global(:root[data-theme="paper"]) .args {
    background: color-mix(in srgb, var(--fg) 4%, var(--panel));
  }
  :global(:root[data-theme="paper"]) .field {
    border-width: 1.5px;
    border-radius: var(--radius);
  }
  :global(:root[data-theme="paper"]) .note :global(svg) {
    color: var(--accent);
  }
  :global(:root[data-theme="paper"]) .sys .btn:hover {
    box-shadow: 4px 4px 0 var(--fg);
    transform: translate(-2px, -2px);
  }
  :global(:root[data-theme="paper"]) .tools .btn.on {
    background: var(--fg);
    color: var(--panel);
  }
  .btn:disabled {
    opacity: 0.45;
    pointer-events: none;
  }

  @media (max-width: 1180px) {
    .sys {
      padding-inline: 40px;
    }
  }
</style>
