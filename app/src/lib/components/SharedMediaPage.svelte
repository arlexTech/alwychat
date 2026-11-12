<script lang="ts">
  import { peek, remember } from "$lib/cache";
  import { tr } from "$lib/i18n/index.svelte";
  import { onMount } from "svelte";
  import FileText from "@lucide/svelte/icons/file-text";
  import Mic from "@lucide/svelte/icons/mic";
  import Play from "@lucide/svelte/icons/play";
  import Link from "@lucide/svelte/icons/link";
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import { fileAction, openExternal, searchStatus, sharedMedia, type KindFilter, type SharedItem } from "$lib/backend";
  import { formatDuration, formatSize, shortTime } from "$lib/format";
  import { eventMediaUrl } from "$lib/media";
  import FileSheet from "./FileSheet.svelte";
  import MediaViewer from "./MediaViewer.svelte";
  import Page from "./Page.svelte";
  import { onBack } from "$lib/back.svelte";

  let {
    roomId,
    onShow,
    onClose,
  }: { roomId: string; onShow: (eventId: string) => void; onClose: () => void } = $props();

  const TABS: { id: KindFilter; label: string }[] = [
    { id: "media", label: tr("Photos & videos") },
    { id: "files", label: tr("Files") },
    { id: "links", label: tr("Links") },
  ];
  const PAGE = 60;

  let tab = $state<KindFilter>("media");
  let items = $state<SharedItem[]>([]);
  let loading = $state(false);
  let done = $state(false);
  /** The chat's whole history is indexed (else older items are still coming). */
  let complete = $state(true);
  let error = $state<string | null>(null);
  let viewer = $state<number | null>(null);
  let fileFor = $state<SharedItem | null>(null);
  let toast = $state<string | null>(null);
  let generation = 0;

  // Android back closes the file sheet before the page.
  $effect(() => {
    if (fileFor) return onBack(() => (fileFor = null));
  });

  async function load(reset: boolean) {
    if (!reset && (done || loading)) return;
    const g = reset ? ++generation : generation;
    const kind = tab;
    loading = true;
    error = null;
    try {
      const before = reset ? null : (items.at(-1)?.timestamp ?? null);
      const page = await sharedMedia(roomId, kind, before);
      if (g !== generation) return;
      items = reset ? remember(`shared:${roomId}:${kind}`, page) : [...items, ...page];
      done = page.length < PAGE;
    } catch (e) {
      if (g === generation) error = String(e);
    } finally {
      if (g === generation) loading = false;
    }
  }

  function show(kind: KindFilter) {
    tab = kind;
    items = peek<SharedItem[]>(`shared:${roomId}:${kind}`) ?? [];
    done = false;
    void load(true);
  }

  onMount(() => {
    items = peek<SharedItem[]>(`shared:${roomId}:${tab}`) ?? [];
    void load(true);
    // History is indexed in the background: refresh the first page until this chat is done.
    const timer = setInterval(async () => {
      complete = (await searchStatus(roomId).catch(() => null))?.chat_done ?? true;
      if (!complete && items.length < PAGE) void load(true);
    }, 4_000);
    return () => clearInterval(timer);
  });

  let sentinel = $state<HTMLElement>();
  $effect(() => {
    if (!sentinel) return;
    const observer = new IntersectionObserver((e) => e[0].isIntersecting && load(false));
    observer.observe(sentinel);
    return () => observer.disconnect();
  });

  const visual = $derived(items.filter((i) => i.media && (i.kind === "image" || i.kind === "video")));

  function flash(message: string) {
    toast = message;
    setTimeout(() => toast === message && (toast = null), 3000);
  }

  async function onFile(item: SharedItem, action: "open" | "save_as" | "downloads") {
    if (!item.media) return;
    if (action === "open") flash(tr("Opening…"));
    try {
      const result = await fileAction(item.media, action);
      if (result) flash(result.replace(/^Saved to \/home\/[^/]+/, "Saved to ~"));
    } catch (e) {
      flash(tr("Couldn't {value}: {e}", { value: action === "open" ? "open" : "save", e: e }));
    }
  }

  const host = (url: string) => {
    try {
      return new URL(url).hostname.replace(/^www\./, "");
    } catch {
      return url;
    }
  };
</script>

<Page title={tr("Shared media")} {onClose}>
  <nav class="tabs">
    {#each TABS as t (t.id)}
      <button class:active={tab === t.id} onclick={() => show(t.id)}>{t.label}</button>
    {/each}
  </nav>

  {#if error}
    <p class="note error">{error}</p>
  {:else if tab === "media"}
    <div class="grid">
      {#each visual as item, i (item.event_id)}
        <button class="cell" onclick={() => (viewer = i)} aria-label={item.kind === "video" ? tr("Play video") : tr("Open photo")}>
          <img src={eventMediaUrl(item.media!, true)} alt="" loading="lazy" decoding="async" />
          {#if item.kind === "video"}
            <span class="badge"><Play size={11} fill="currentColor" />{formatDuration(item.duration_ms)}</span>
          {/if}
        </button>
      {/each}
    </div>
  {:else}
    <ul class="list">
      {#each items as item (item.event_id)}
        <li>
          {#if tab === "files"}
            <button class="main" onclick={() => (fileFor = item)}>
              <span class="icon">{#if item.kind === "voice"}<Mic size={20} />{:else}<FileText size={20} />{/if}</span>
              <span class="text">
                <strong>{item.kind === "voice" ? tr("Voice message") : item.filename}</strong>
                <small>{[formatSize(item.size), item.sender_name, shortTime(item.timestamp)].filter(Boolean).join(" · ")}</small>
              </span>
            </button>
          {:else}
            <button class="main" onclick={() => item.url && openExternal(item.url)}>
              <span class="icon"><Link size={20} /></span>
              <span class="text">
                <strong>{item.url ? host(item.url) : tr("Link")}</strong>
                <small class="clamp">{item.text}</small>
                <small>{item.sender_name} · {shortTime(item.timestamp)}</small>
              </span>
            </button>
          {/if}
          <button class="show" onclick={() => onShow(item.event_id)} aria-label={tr("Show in chat")}><MessageSquare size={18} /></button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if !loading && !error && items.length === 0}
    <p class="note">
      {tab === "media" ? tr("No photos or videos yet.") : tab === "files" ? tr("No files yet.") : tr("No links yet.")}
    </p>
  {/if}
  {#if !complete}
    <p class="note small">{tr("Still reading older messages: more may appear.")}</p>
  {/if}
  <div bind:this={sentinel} class="sentinel"></div>
  {#if toast}<p class="toast">{toast}</p>{/if}
</Page>

{#if viewer !== null}
  <MediaViewer
    items={visual.map((i) => ({ media: i.media!, kind: i.kind as "image" | "video", caption: null }))}
    index={viewer}
    onClose={() => (viewer = null)}
    onShow={(i) => {
      const id = visual[i]?.event_id;
      viewer = null;
      if (id) onShow(id);
    }}
  />
{/if}
{#if fileFor}
  {@const item = fileFor}
  <FileSheet filename={item.filename ?? "File"} onPick={(action) => onFile(item, action)} onClose={() => (fileFor = null)} />
{/if}

<style>
  .tabs {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    margin-bottom: 12px;
    border-bottom: 1px solid var(--border);
  }
  .tabs button {
    padding: 10px 4px;
    border: none;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .tabs button.active {
    border-bottom-color: var(--accent-a);
    color: var(--text);
    font-weight: 600;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(110px, 30%), 1fr));
    gap: 3px;
  }
  .cell {
    position: relative;
    aspect-ratio: 1;
    padding: 0;
    overflow: hidden;
    border: none;
    background: var(--surface-2);
    cursor: pointer;
  }
  .cell img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .badge {
    position: absolute;
    right: 4px;
    bottom: 4px;
    display: flex;
    align-items: center;
    gap: 3px;
    padding: 2px 6px;
    border-radius: 999px;
    background: rgb(0 0 0 / 0.6);
    color: white;
    font-size: 11px;
  }
  .list {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .list li {
    display: flex;
    align-items: center;
  }
  .main {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 12px;
    min-width: 0;
    padding: 10px 8px;
    border: none;
    border-radius: 12px;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .main:hover,
  .show:hover {
    background: var(--surface-hover);
  }
  .icon {
    display: grid;
    flex: none;
    width: 42px;
    height: 42px;
    place-items: center;
    border-radius: 12px;
    background: var(--surface-2);
    color: var(--accent-a);
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .text strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .text small {
    color: var(--muted);
  }
  .clamp {
    display: -webkit-box;
    overflow: hidden;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow-wrap: anywhere;
  }
  .show {
    display: grid;
    width: 40px;
    height: 40px;
    place-items: center;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .note {
    padding: 16px 8px;
    color: var(--muted);
    text-align: center;
  }
  .note.small {
    font-size: 13px;
  }
  .note.error {
    color: var(--danger);
  }
  .sentinel {
    height: 1px;
  }
  .toast {
    position: fixed;
    left: 50%;
    bottom: 24px;
    padding: 10px 16px;
    border-radius: 12px;
    background: var(--surface-2);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.3);
    translate: -50% 0;
  }
</style>
