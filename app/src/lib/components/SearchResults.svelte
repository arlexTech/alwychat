<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import Image from "@lucide/svelte/icons/image";
  import FileText from "@lucide/svelte/icons/file-text";
  import Mic from "@lucide/svelte/icons/mic";
  import ChartBar from "@lucide/svelte/icons/chart-bar";
  import type { SearchHit } from "$lib/backend";
  import { shortTime } from "$lib/format";
  import { eventMediaUrl } from "$lib/media";
  import Avatar from "./Avatar.svelte";
  import Highlight from "./Highlight.svelte";

  let {
    hits,
    showChat = true,
    onPick,
    onMore,
  }: {
    hits: SearchHit[];
    /** Global search: each result shows its chat. */
    showChat?: boolean;
    onPick: (hit: SearchHit) => void;
    /** Called when the end of the list comes into view. */
    onMore?: () => void;
  } = $props();

  let sentinel = $state<HTMLElement>();
  $effect(() => {
    if (!sentinel || !onMore) return;
    const more = onMore;
    const observer = new IntersectionObserver((e) => e[0].isIntersecting && more());
    observer.observe(sentinel);
    return () => observer.disconnect();
  });
</script>

<ul>
  {#each hits as hit (hit.event_id)}
    <li>
      <button onclick={() => onPick(hit)}>
        {#if showChat}
          <Avatar name={hit.room_name} mxc={hit.room_avatar} size={44} />
        {/if}
        <span class="body">
          <span class="line">
            <strong>{showChat ? hit.room_name : hit.is_own ? tr("You") : hit.sender_name}</strong>
            <span class="time">{shortTime(hit.timestamp)}</span>
          </span>
          <span class="snippet">
            {#if showChat && hit.room_name !== hit.sender_name}<span class="who">{hit.is_own ? tr("You") : hit.sender_name}:</span>{/if}
            {#if hit.kind === "image" || hit.kind === "video"}<Image size={14} />
            {:else if hit.kind === "file" || hit.kind === "audio"}<FileText size={14} />
            {:else if hit.kind === "voice"}<Mic size={14} />
            {:else if hit.kind === "poll"}<ChartBar size={14} />{/if}
            <Highlight text={hit.snippet} />
          </span>
        </span>
        {#if hit.media}
          <img class="thumb" src={eventMediaUrl(hit.media, true)} alt="" loading="lazy" />
        {/if}
      </button>
    </li>
  {/each}
</ul>
{#if onMore}<div bind:this={sentinel} class="sentinel"></div>{/if}

<style>
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  button {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 10px 12px;
    border: none;
    border-radius: 12px;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  button:hover {
    background: var(--surface-hover);
  }
  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .line {
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .line strong {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .time {
    color: var(--muted);
    font-size: 12px;
  }
  .snippet {
    display: -webkit-box;
    overflow: hidden;
    color: var(--muted);
    font-size: 14px;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow-wrap: anywhere;
  }
  .snippet :global(svg) {
    vertical-align: -2px;
  }
  .who {
    color: var(--text);
  }
  .thumb {
    width: 48px;
    height: 48px;
    flex: none;
    border-radius: 8px;
    object-fit: cover;
    background: var(--surface-2);
  }
  .sentinel {
    height: 1px;
  }
</style>
