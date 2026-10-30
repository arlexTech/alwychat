<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { openExternal, type MessageEntry } from "$lib/backend";
  import { clockTime } from "$lib/format";
  import { eventMediaUrl } from "$lib/media";
  import Avatar from "./Avatar.svelte";

  let {
    msg,
    first,
    last,
    showSender,
    quickReaction,
    onReact,
    onReply,
    onJumpTo,
    onUnlock,
    onEdit,
    onDelete,
    onHistory,
    onOpenMedia,
    onSaveFile,
  }: {
    msg: MessageEntry;
    /** First / last of a run of messages from the same sender: drives corners, name and avatar. */
    first: boolean;
    last: boolean;
    /** Group chats show names and avatars of others. */
    showSender: boolean;
    quickReaction: string;
    onReact: (key: string) => void;
    onReply: () => void;
    onJumpTo: (eventId: string) => void;
    onUnlock: () => void;
    onEdit: () => void;
    onDelete: () => void;
    onHistory: () => void;
    onOpenMedia: () => void;
    onSaveFile: () => void;
  } = $props();

  const QUICK = ["❤️", "😂", "😮", "😢", "😡", "👍"];
  /** Largest side of an image/video preview in the timeline, in CSS px. */
  const PREVIEW_MAX = 300;

  let pickerOpen = $state(false);
  let menuOpen = $state(false);

  const c = $derived(msg.content);

  const text = $derived.by(() => {
    switch (c.kind) {
      case "text":
      case "notice":
        return c.body;
      case "emote":
        return `* ${msg.sender_name} ${c.body}`;
      case "image":
      case "video":
      case "file":
        return c.caption ?? "";
      case "media":
        return c.label;
      case "sticker":
        return "";
      case "poll":
        return `📊 ${c.question}`;
      case "redacted":
        return "Message deleted";
      case "unable_to_decrypt":
        return "🔒 Can't read this message yet";
      case "unsupported":
        return "Unsupported message";
    }
  });

  /** Sanitised HTML from the core, when the message has a formatted version. */
  const html = $derived("html" in c ? c.html : null);

  /** Preview box for photos/videos/stickers, keeping the aspect ratio. */
  const box = $derived.by(() => {
    if (c.kind !== "image" && c.kind !== "video" && c.kind !== "sticker") return null;
    const max = c.kind === "sticker" ? 160 : PREVIEW_MAX;
    const w = c.width ?? 4;
    const h = c.height ?? 3;
    const k = Math.min(1, max / Math.max(w, h));
    return { w: Math.max(80, Math.round(w * k)), h: Math.max(60, Math.round(h * k)) };
  });
  let mediaLoaded = $state(false);
  let mediaFailed = $state(false);

  // Links open in the browser; spoilers reveal on click.
  function onTextClick(event: MouseEvent) {
    const target = event.target as HTMLElement;
    const spoiler = target.closest<HTMLElement>("[data-mx-spoiler]");
    if (spoiler && !spoiler.classList.contains("revealed")) {
      event.preventDefault();
      spoiler.classList.add("revealed");
      return;
    }
    const link = target.closest<HTMLAnchorElement>("a[href]");
    if (link) {
      event.preventDefault();
      openExternal(link.href).catch(() => {});
    }
  }

  const muted = $derived(["redacted", "unable_to_decrypt", "unsupported"].includes(c.kind));
  const big = $derived(msg.only_emojis && c.kind === "text" && !msg.reply_to);
  /** Photos, videos and stickers are drawn without a bubble background. */
  const bare = $derived((c.kind === "image" || c.kind === "video") && !text && !msg.reply_to || c.kind === "sticker");
  const remote = $derived(msg.item_id.startsWith("$"));
  const canReact = $derived(!muted && remote);
  const hasMenu = $derived(msg.can_edit || msg.can_delete || (msg.is_edited && remote));

  function react(key: string) {
    pickerOpen = false;
    onReact(key);
  }

  function pick(action: () => void) {
    menuOpen = false;
    action();
  }

  function formatSize(bytes: number | null) {
    if (bytes == null) return "";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(0)} KB`;
    return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  }

  function formatDuration(ms: number | null) {
    if (ms == null) return "";
    const s = Math.round(ms / 1000);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  }
</script>

<div class="row" class:own={msg.is_own} class:first class:last>
  {#if showSender && !msg.is_own}
    <div class="avatar-slot">
      {#if last}<Avatar name={msg.sender_name} mxc={msg.sender_avatar} size={28} />{/if}
    </div>
  {/if}

  <div class="stack">
    {#if showSender && !msg.is_own && first}
      <span class="sender">{msg.sender_name}</span>
    {/if}

    <div class="line">
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="bubble"
        class:big
        class:bare
        class:muted
        class:has-media={box}
        class:sending={msg.send_state === "sending"}
        class:failed={msg.send_state === "failed"}
        title={clockTime(msg.timestamp)}
        ondblclick={() => canReact && react(quickReaction)}
      >
        {#if msg.reply_to}
          <button class="quote" onclick={() => onJumpTo(msg.reply_to!.event_id)}>
            <strong>{msg.reply_to.sender_name ?? "…"}</strong>
            <span>{msg.reply_to.text ?? "Loading…"}</span>
          </button>
        {/if}

        {#if box && (c.kind === "image" || c.kind === "video" || c.kind === "sticker")}
          <button
            class="media"
            class:loaded={mediaLoaded}
            style:width="{box.w}px"
            style:height="{box.h}px"
            onclick={() => c.kind !== "sticker" && onOpenMedia()}
            aria-label={c.kind === "video" ? "Play video" : c.kind === "image" ? "Open photo" : c.body}
          >
            {#if mediaFailed}
              <span class="media-error">{c.kind === "video" ? "🎬" : "📷"} Couldn't load</span>
            {:else if c.kind !== "video" || c.has_thumbnail}
              <img
                src={eventMediaUrl(c.media, true)}
                alt={c.kind === "sticker" ? c.body : ""}
                decoding="async"
                onload={() => (mediaLoaded = true)}
                onerror={() => (mediaFailed = true)}
              />
            {/if}
            {#if c.kind === "video"}
              <span class="play">▶</span>
              {#if c.duration_ms}<span class="duration">{formatDuration(c.duration_ms)}</span>{/if}
            {/if}
          </button>
        {:else if c.kind === "file"}
          <button class="file" onclick={onSaveFile} title="Save to Downloads">
            <span class="file-icon">📄</span>
            <span class="file-meta">
              <strong>{c.filename}</strong>
              <small>{formatSize(c.size)} · Save</small>
            </span>
          </button>
        {/if}

        {#if text || html}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <span class="text" class:rich={html} class:caption={box} onclick={onTextClick}>
            {#if html}{#if c.kind === "emote"}* {msg.sender_name} {/if}{@html html}{:else}{text}{/if}
          </span>
        {/if}
        {#if c.kind === "unable_to_decrypt"}
          <button class="unlock" onclick={onUnlock}>Unlock</button>
        {/if}
        {#if msg.is_edited}
          <button class="edited" onclick={onHistory} disabled={!remote}>(edited)</button>
        {/if}
      </div>

      {#if canReact || msg.can_reply || hasMenu}
        <div class="actions" class:open={menuOpen || pickerOpen}>
          {#if canReact}
            <button onclick={() => ((pickerOpen = !pickerOpen), (menuOpen = false))} aria-label="React">☺</button>
          {/if}
          {#if msg.can_reply}
            <button onclick={onReply} aria-label="Reply">↩</button>
          {/if}
          {#if hasMenu}
            <button onclick={() => ((menuOpen = !menuOpen), (pickerOpen = false))} aria-label="More">⋯</button>
          {/if}
        </div>
      {/if}
      <!-- Anchored to the bubble's outer edge so they always open into the chat, never under the list. -->
      {#if pickerOpen}
        <div class="picker" transition:scale={{ duration: 140, start: 0.8 }}>
          {#each QUICK as key (key)}
            <button onclick={() => react(key)}>{key}</button>
          {/each}
        </div>
      {/if}
      {#if menuOpen}
        <div class="menu" transition:fade={{ duration: 100 }}>
          {#if msg.can_edit}<button onclick={() => pick(onEdit)}>✏️ Edit</button>{/if}
          {#if msg.is_edited && remote}<button onclick={() => pick(onHistory)}>🕘 Edit history</button>{/if}
          {#if msg.can_delete}<button class="danger" onclick={() => pick(onDelete)}>🗑️ Delete</button>{/if}
        </div>
      {/if}
    </div>

    {#if msg.reactions.length}
      <div class="reactions">
        {#each msg.reactions as r (r.key)}
          <button class="chip" class:mine={r.mine} onclick={() => react(r.key)} in:scale={{ duration: 180, start: 0.5 }}>
            {r.key}{#if r.count > 1}<span>{r.count}</span>{/if}
          </button>
        {/each}
      </div>
    {/if}

    {#if msg.send_state === "failed"}
      <span class="status error">Not sent</span>
    {/if}

    {#if msg.seen_by.length}
      <div class="seen" title="Seen by {msg.seen_by.map((r) => r.name).join(', ')}">
        {#each msg.seen_by.slice(0, 5) as r (r.name)}
          <span in:scale={{ duration: 200, start: 0.4 }}><Avatar name={r.name} mxc={r.avatar} size={14} /></span>
        {/each}
        {#if msg.seen_by.length > 5}<small>+{msg.seen_by.length - 5}</small>{/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .row {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    padding: 1px 16px;
  }
  .row.first {
    margin-top: 8px;
  }
  .row.own {
    flex-direction: row-reverse;
  }
  .avatar-slot {
    flex: none;
    width: 28px;
  }
  .stack {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    max-width: min(560px, 75%);
    min-width: 0;
  }
  .own .stack {
    align-items: flex-end;
  }
  .sender {
    margin: 0 12px 2px;
    font-size: 0.78rem;
    color: var(--muted);
  }
  .line {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 100%;
  }
  .own .line {
    flex-direction: row-reverse;
  }

  /* Messenger corners: round outside, tighter where bubbles of a run touch. */
  .bubble {
    --r: 18px;
    --t: 5px;
    position: relative;
    min-width: 0;
    padding: 8px 12px;
    border-radius: var(--t) var(--r) var(--r) var(--t);
    background: var(--surface-2);
    overflow-wrap: anywhere;
    line-height: 1.35;
    user-select: text;
    transition: opacity 0.2s var(--ease);
  }
  .first .bubble {
    border-top-left-radius: var(--r);
  }
  .last .bubble {
    border-bottom-left-radius: var(--r);
  }
  .own .bubble {
    border-radius: var(--r) var(--t) var(--t) var(--r);
    background: var(--accent);
    background-attachment: fixed;
    color: white;
  }
  .own.first .bubble {
    border-top-right-radius: var(--r);
  }
  .own.last .bubble {
    border-bottom-right-radius: var(--r);
  }
  .bubble.big {
    padding: 0 4px;
    background: none;
    font-size: 2.6rem;
    line-height: 1.15;
  }
  .bubble.muted {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--muted);
    font-style: italic;
  }
  .bubble.sending {
    opacity: 0.6;
  }
  .bubble.failed {
    outline: 1px solid var(--danger);
  }
  .quote {
    display: flex;
    flex-direction: column;
    width: 100%;
    margin: 0 0 6px;
    padding: 4px 8px;
    border: none;
    border-left: 3px solid currentColor;
    border-radius: 6px;
    background: rgb(0 0 0 / 0.18);
    text-align: left;
    font-size: 0.85rem;
    font-style: normal;
    cursor: pointer;
    opacity: 0.85;
  }
  .quote span {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .text {
    white-space: pre-wrap;
  }
  /* Formatted messages: HTML carries its own line breaks. */
  .text.rich {
    white-space: normal;
  }
  .rich :global(p) {
    margin: 0;
  }
  .rich :global(p + p),
  .rich :global(ul),
  .rich :global(ol),
  .rich :global(pre),
  .rich :global(blockquote) {
    margin: 6px 0 0;
  }
  .rich :global(ul),
  .rich :global(ol) {
    padding-left: 20px;
  }
  .rich :global(a) {
    color: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .rich :global(code) {
    padding: 1px 5px;
    border-radius: 5px;
    background: rgb(0 0 0 / 0.25);
    font: 0.88em ui-monospace, "JetBrains Mono", monospace;
  }
  .rich :global(pre) {
    padding: 8px 10px;
    border-radius: 8px;
    background: rgb(0 0 0 / 0.25);
    overflow-x: auto;
    white-space: pre;
  }
  .rich :global(pre code) {
    padding: 0;
    background: none;
  }
  .rich :global(blockquote) {
    padding-left: 10px;
    border-left: 3px solid currentColor;
    opacity: 0.85;
  }
  .rich :global(h1),
  .rich :global(h2),
  .rich :global(h3) {
    margin: 4px 0;
    font-size: 1.1em;
  }
  .rich :global([data-mx-spoiler]) {
    border-radius: 4px;
    background: currentColor;
    cursor: pointer;
    transition: background 0.2s var(--ease);
  }
  .rich :global([data-mx-spoiler].revealed) {
    background: rgb(0 0 0 / 0.2);
    cursor: auto;
  }
  .edited {
    margin-left: 6px;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font-size: 0.72rem;
    opacity: 0.7;
    cursor: pointer;
  }
  .edited:disabled {
    cursor: default;
  }
  .unlock {
    margin-left: 8px;
    padding: 2px 10px;
    border: none;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font-size: 0.8rem;
    font-style: normal;
    cursor: pointer;
  }

  .actions {
    position: relative;
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity 0.15s var(--ease);
  }
  .line:hover .actions,
  .actions:focus-within {
    opacity: 1;
  }
  .actions > button {
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--muted);
    cursor: pointer;
  }
  .actions > button:hover {
    color: var(--text);
  }
  .picker {
    position: absolute;
    bottom: calc(100% + 6px);
    left: 0;
    z-index: 5;
    display: flex;
    gap: 2px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.4);
  }
  .own .picker {
    left: auto;
    right: 0;
  }
  .picker button {
    width: 36px;
    height: 36px;
    border: none;
    border-radius: 50%;
    background: none;
    font-size: 1.35rem;
    cursor: pointer;
    transition: transform 0.12s var(--ease);
  }
  .picker button:hover {
    transform: scale(1.3);
  }

  .reactions {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin: -6px 8px 2px;
    z-index: 1;
  }
  .chip {
    display: flex;
    align-items: center;
    gap: 3px;
    padding: 1px 7px;
    border: 2px solid var(--bg);
    border-radius: 999px;
    background: var(--surface-2);
    font-size: 0.9rem;
    cursor: pointer;
    transition: transform 0.12s var(--ease);
  }
  .chip:active {
    transform: scale(0.9);
  }
  .chip.mine {
    background: color-mix(in srgb, var(--accent-a) 35%, var(--surface-2));
  }
  .chip span {
    font-size: 0.75rem;
    color: var(--muted);
  }
  .status {
    margin: 2px 8px 0;
    font-size: 0.75rem;
  }
  .status.error {
    color: var(--danger);
  }
  .bubble.bare {
    padding: 0;
    background: none;
  }
  .bubble.has-media:not(.bare) {
    padding: 4px;
  }
  .bubble.has-media:not(.bare) .quote {
    margin: 4px 4px 6px;
    width: calc(100% - 8px);
  }
  .media {
    position: relative;
    display: block;
    padding: 0;
    border: none;
    border-radius: 14px;
    overflow: hidden;
    background: var(--surface-hover);
    cursor: pointer;
  }
  .media:not(.loaded) {
    background: linear-gradient(90deg, var(--surface-2), var(--surface-hover), var(--surface-2));
    background-size: 200% 100%;
    animation: shimmer 1.2s infinite linear;
  }
  @keyframes shimmer {
    to {
      background-position: -200% 0;
    }
  }
  .media img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity 0.25s var(--ease);
  }
  .media.loaded img {
    opacity: 1;
  }
  .bare .media {
    background-color: transparent;
  }
  .media-error {
    display: grid;
    place-items: center;
    height: 100%;
    font-size: 0.85rem;
    color: var(--muted);
  }
  .play {
    position: absolute;
    inset: 0;
    margin: auto;
    width: 48px;
    height: 48px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: rgb(0 0 0 / 0.5);
    color: white;
    font-size: 1.2rem;
  }
  .duration {
    position: absolute;
    right: 8px;
    bottom: 8px;
    padding: 1px 6px;
    border-radius: 6px;
    background: rgb(0 0 0 / 0.55);
    color: white;
    font-size: 0.75rem;
  }
  .text.caption {
    display: block;
    padding: 6px 8px 4px;
  }
  .file {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 200px;
    padding: 4px 2px;
    border: none;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .file-icon {
    font-size: 1.8rem;
  }
  .file-meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .file-meta strong {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .file-meta small {
    opacity: 0.75;
  }
  .actions.open {
    opacity: 1;
  }
  .menu {
    position: absolute;
    bottom: calc(100% + 6px);
    left: 0;
    z-index: 6;
    display: flex;
    flex-direction: column;
    min-width: 170px;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface-2);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.4);
  }
  .own .menu {
    left: auto;
    right: 0;
  }
  .menu button {
    padding: 8px 10px;
    border: none;
    border-radius: 8px;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--surface-hover);
  }
  .menu .danger {
    color: var(--danger);
  }
  .seen {
    display: flex;
    align-items: center;
    gap: 2px;
    margin: 3px 6px 0;
  }
  .seen small {
    font-size: 0.7rem;
    color: var(--muted);
  }
</style>
