<script lang="ts">
  import { src, type Game } from "$lib/api";
  import { app } from "$lib/store.svelte";

  /** `cover` = box art (3:4), `wide` = background/key art (16:9). Falls back to a typeset cover,
   *  or to a blank panel with `plain` (thumbnails too small for text, with the title beside them). */
  let { game, kind = "cover", plain = false }: { game: Game; kind?: "cover" | "wide"; plain?: boolean } = $props();

  const url = $derived(
    kind === "wide"
      ? src(game.hero ?? game.cover ?? game.coverThumb)
      : src(game.coverThumb ?? game.cover ?? game.hero),
  );
  // Only box art is letterboxed; key art standing in for a cover is cropped to fill.
  const isBoxArt = $derived(kind === "cover" ? !!(game.coverThumb ?? game.cover) : !game.hero);
  const logo = $derived(src(game.logo));
  const platform = $derived(app.platforms.get(game.platform));

  let fit = $state(false);
  let failed = $state(false);
  $effect(() => {
    void url;
    failed = false;
    fit = false;
  });
  function onload(e: Event) {
    const img = e.currentTarget as HTMLImageElement;
    const r = img.naturalWidth / img.naturalHeight;
    fit = isBoxArt && (kind === "cover" ? r > 0.82 : r < 1.2);
  }
</script>

<div class="frame">
  {#if url && !failed}
    <img class="art" class:fit src={url} alt="" loading="lazy" decoding="async" {onload} onerror={() => (failed = true)} />
  {:else if !plain}
    <div class="typeset">
      {#if logo}<img class="logo art" src={logo} alt="" />{/if}
      <div class="t">{game.title}</div>
      <div class="p label">{game.kind === "port" ? "PC port" : (platform?.short ?? "")}</div>
    </div>
  {/if}
</div>

<style>
  .frame {
    position: absolute;
    inset: 0;
    overflow: hidden;
    background: var(--panel);
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  img.fit {
    object-fit: contain;
    padding: 6%;
  }
  .typeset {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    gap: 6px;
    padding: 12%;
    container-type: inline-size;
  }
  :global(:root[data-theme="pixel"]) .typeset {
    border: var(--border) solid var(--line);
    background: var(--bg);
  }
  :global(:root[data-theme="paper"]) .typeset {
    border: 1px solid var(--line);
    outline: 1px solid var(--line);
    outline-offset: -8px;
  }
  .logo {
    position: static;
    width: 34%;
    height: auto;
    aspect-ratio: 1;
    margin: auto auto 10%;
  }
  .t {
    font-family: var(--title);
    font-weight: 700;
    font-size: clamp(14px, 11cqw, 34px);
    line-height: 1.02;
    text-wrap: balance;
  }
</style>
