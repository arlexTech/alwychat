<script lang="ts" module>
  import { linkPreview, openExternal, type LinkPreview } from "$lib/backend";

  /** One request per URL for the whole app (the timeline re-renders bubbles often). */
  const cache = new Map<string, Promise<LinkPreview | null>>();
  function load(url: string) {
    let hit = cache.get(url);
    if (!hit) {
      hit = linkPreview(url).catch(() => null);
      cache.set(url, hit);
    }
    return hit;
  }
</script>

<script lang="ts">
  import { mediaUrl } from "$lib/media";

  let { url }: { url: string } = $props();

  let preview = $state<LinkPreview | null>(null);
  $effect(() => {
    const wanted = url;
    load(wanted).then((p) => wanted === url && (preview = p));
  });

  const host = $derived.by(() => {
    try {
      return new URL(url).hostname.replace(/^www\./, "");
    } catch {
      return url;
    }
  });
  /** Wide images (articles, videos) go on top; small/square ones (logos) sit on the side. */
  const wide = $derived(
    !!preview?.image_width && !!preview.image_height && preview.image_width / preview.image_height > 1.3 && preview.image_width >= 400,
  );
</script>

{#if preview}
  <button class="card" class:wide onclick={() => openExternal(url).catch(() => {})} title={url}>
    {#if preview.image}
      <img
        src={mediaUrl(preview.image, wide ? 320 : 72)}
        alt=""
        loading="lazy"
        style:aspect-ratio={wide ? `${preview.image_width} / ${preview.image_height}` : undefined}
      />
    {/if}
    <span class="meta">
      <small class="site">{preview.site_name ?? host}</small>
      {#if preview.title}<strong>{preview.title}</strong>{/if}
      {#if preview.description}<span class="desc">{preview.description}</span>{/if}
    </span>
  </button>
{/if}

<style>
  .card {
    display: flex;
    gap: 10px;
    width: 100%;
    max-width: 320px;
    margin-top: 6px;
    padding: 0;
    overflow: hidden;
    border: none;
    border-left: 3px solid color-mix(in srgb, currentColor 50%, transparent);
    border-radius: 10px;
    background: rgb(0 0 0 / 0.18);
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .card.wide {
    flex-direction: column;
    gap: 0;
  }
  img {
    flex: none;
    width: 72px;
    height: 72px;
    object-fit: cover;
    background: color-mix(in srgb, currentColor 6%, transparent);
  }
  .wide img {
    width: 100%;
    height: auto;
    max-height: 200px;
  }
  .meta {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px 8px 0;
  }
  .card:not(:has(img)) .meta,
  .wide .meta {
    padding-left: 10px;
  }
  .site {
    opacity: 0.75;
    font-size: 0.75rem;
  }
  strong {
    font-size: 0.9rem;
    line-height: 1.25;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .desc {
    font-size: 0.82rem;
    opacity: 0.85;
    line-height: 1.3;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
</style>
