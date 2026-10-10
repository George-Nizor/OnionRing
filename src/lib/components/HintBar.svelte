<script lang="ts">
  import { app } from "$lib/store.svelte";

  let { pad = false }: { pad?: boolean } = $props();

  type Hint = [string, string, string]; // pad button, key, what it does
  const hints = $derived.by((): Hint[] => {
    const v = app.view.name;
    if (app.searchOpen) return [["A", "Enter", "Open"], ["B", "Esc", "Close"]];
    if (app.menuOpen) return [["A", "Enter", "Go"], ["B", "Esc", "Close"]];
    const base: Hint[] = [["A", "Enter", v === "game" ? "Select" : "Open"]];
    if (v === "home" || v === "library") base.push(["X", "Space", "Play now"]);
    if (v === "library") base.push(["LB RB", "Q E", "Platform"]);
    base.push(["Y", "/", "Search"]);
    if (v !== "home") base.push(["B", "Esc", "Back"]);
    base.push(["☰", "M", "Menu"]);
    return base;
  });
</script>

<footer class="hints">
  {#each hints as [b, k, what] (what)}
    <span class="hint">
      {#each (pad ? b : k).split(" ") as key (key)}<kbd class:round={pad && key.length === 1}>{key}</kbd>{/each}
      <span class="what label">{what}</span>
    </span>
  {/each}
</footer>

<style>
  .hints {
    height: var(--hints);
    flex: none;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 26px;
    padding: 0 26px;
  }
  :global(:root[data-theme="pixel"]) .hints {
    border-top: var(--border) solid var(--line);
  }
  :global(:root[data-theme="paper"]) .hints {
    border-top: 1.5px solid var(--line);
  }
  .hint {
    display: flex;
    align-items: center;
    gap: 7px;
  }
  kbd {
    min-width: 26px;
    height: 26px;
    padding: 0 6px;
    display: grid;
    place-items: center;
    font-family: var(--label);
    font-size: 12px;
    font-weight: 700;
  }
  :global(:root[data-theme="pixel"]) kbd {
    background: var(--fg);
    color: var(--bg);
  }
  :global(:root[data-theme="paper"]) kbd {
    border: 1.5px solid var(--line);
    border-radius: 4px;
  }
  :global(:root[data-theme="paper"]) kbd.round {
    border-radius: 50%;
  }
  .what {
    color: var(--muted);
  }
</style>
