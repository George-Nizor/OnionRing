<script lang="ts">
  import { src, type Game } from "$lib/api";
  import { app } from "$lib/store.svelte";

  let { game, full = false }: { game: Game; full?: boolean } = $props();

  const platform = $derived(app.platforms.get(game.platform));
  const color = $derived(app.accent(game));
  const coverUrl = $derived(src(full ? (game.cover ?? game.coverThumb) : (game.coverThumb ?? game.cover)));
  const heroUrl = $derived(src(game.hero));
  const logoUrl = $derived(src(game.logo));

  // Square-ish box art (DS/3DS) is shown whole over a blurred fill instead of being cropped.
  let fit = $state(false);
  let failed = $state(false);
  function onload(e: Event) {
    const img = e.currentTarget as HTMLImageElement;
    fit = img.naturalWidth / img.naturalHeight > 0.82;
  }
  $effect(() => {
    void coverUrl;
    failed = false;
  });
</script>

<div class="cover" style:--c={color}>
  {#if coverUrl && !failed}
    {#if fit}
      <img class="fill" src={coverUrl} alt="" aria-hidden="true" decoding="async" />
    {/if}
    <img class="art" class:fit src={coverUrl} alt="" loading="lazy" decoding="async" {onload} onerror={() => (failed = true)} />
  {:else if heroUrl && heroUrl !== logoUrl}
    <!-- No box art: crop the game's own key art into a poster. -->
    <img class="art" src={heroUrl} alt="" loading="lazy" decoding="async" />
    <div class="scrim"></div>
    <div class="label bottom">
      <div class="t">{game.title}</div>
      <div class="p">{platform?.name ?? ""}</div>
    </div>
  {:else}
    <!-- Nothing to show: a typographic poster in the game's accent colour. -->
    <div class="poster">
      {#if logoUrl}<img class="logo" src={logoUrl} alt="" loading="lazy" />{/if}
      <div class="label" class:centered={!!logoUrl}>
        <div class="t">{game.title}</div>
        <div class="p">{game.kind === "port" ? "Native port" : (platform?.name ?? "")}</div>
      </div>
    </div>
  {/if}
</div>

<style>
  .cover {
    position: absolute;
    inset: 0;
    overflow: hidden;
    background: #100d15;
  }
  img {
    display: block;
  }
  .art {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .art.fit {
    object-fit: contain;
    padding: 9% 7%;
    filter: drop-shadow(0 14px 22px rgba(0, 0, 0, 0.6));
  }
  .fill {
    position: absolute;
    inset: -25%;
    width: 150%;
    height: 150%;
    object-fit: cover;
    filter: blur(24px) saturate(1.5) brightness(0.5);
  }
  .scrim {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, transparent 35%, rgba(6, 5, 9, 0.92));
  }
  .poster {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    background:
      radial-gradient(90% 60% at 50% 35%, color-mix(in srgb, var(--c) 45%, transparent), transparent 75%),
      radial-gradient(140% 100% at 50% 120%, color-mix(in srgb, var(--c) 18%, transparent), transparent 60%),
      linear-gradient(180deg, color-mix(in srgb, var(--c) 22%, #0c0a10), #08070b);
  }
  .poster::before {
    /* faint concentric rings: the brand motif */
    content: "";
    position: absolute;
    inset: 0;
    background: repeating-radial-gradient(circle at 50% 35%, transparent 0 18cqw, rgba(255, 255, 255, 0.045) 18cqw calc(18cqw + 1px));
    mask: radial-gradient(circle at 50% 35%, #000 20%, transparent 70%);
  }
  .logo {
    position: relative;
    width: 34cqw;
    aspect-ratio: 1;
    object-fit: contain;
    margin-top: -14cqw;
    filter: drop-shadow(0 12px 30px rgba(0, 0, 0, 0.6));
  }
  .label {
    position: absolute;
    left: 9cqw;
    right: 9cqw;
    bottom: 10cqw;
  }
  .label.centered {
    text-align: center;
  }
  .t {
    font-family: var(--display);
    font-weight: 750;
    font-size: clamp(14px, 11cqw, 34px);
    line-height: 1;
    letter-spacing: -0.03em;
    text-wrap: balance;
    text-shadow: 0 4px 24px rgba(0, 0, 0, 0.6);
  }
  .p {
    margin-top: 2.5cqw;
    font-size: clamp(9px, 5.4cqw, 13px);
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--c) 45%, white);
    opacity: 0.85;
  }
</style>
