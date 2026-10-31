<script lang="ts">
  import FileText from "@lucide/svelte/icons/file-text";
  import Download from "@lucide/svelte/icons/download";
  import Pin from "@lucide/svelte/icons/pin";
  import PinOff from "@lucide/svelte/icons/pin-off";
  import Pencil from "@lucide/svelte/icons/pencil";
  import History from "@lucide/svelte/icons/history";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import SmilePlus from "@lucide/svelte/icons/smile-plus";
  import Reply from "@lucide/svelte/icons/reply";
  import Ellipsis from "@lucide/svelte/icons/ellipsis";
  import MessagesSquare from "@lucide/svelte/icons/messages-square";
  import { fade, scale } from "svelte/transition";
  import { openExternal, type MessageEntry } from "$lib/backend";
  import { clockTime } from "$lib/format";
  import { eventMediaUrl } from "$lib/media";
  import { onBack } from "$lib/back.svelte";
  import Avatar from "./Avatar.svelte";
  import VoicePlayer from "./VoicePlayer.svelte";
  import LinkPreview from "./LinkPreview.svelte";
  import PollView from "./PollView.svelte";
  import { formatDuration, formatSize } from "$lib/format";

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
    onSaveToGallery,
    pinned = null,
    onPin,
    roomId,
    onLongPress,
    onThread,
    album = null,
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
    /** Index of the photo/video opened (always 0 outside albums). */
    onOpenMedia: (index: number) => void;
    onSaveFile: () => void;
    /** Save this message's photo/video, or the whole album, to the gallery. */
    onSaveToGallery: () => void;
    roomId: string;
    /** Touch long-press: the chat shows its action sheet (instead of the small menu). */
    onLongPress?: () => void;
    /** Open the thread on this message (absent inside a thread). */
    onThread?: () => void;
    /** Whether it's pinned; null when we can't pin in this chat. */
    pinned?: boolean | null;
    onPin?: (pin: boolean) => void;
    /** Several photos drawn as one grid; `msg` is the first. */
    album?: MessageEntry[] | null;
  } = $props();

  /** Grid cells: up to 4 photos, the last one showing "+N" when there are more. */
  const ALBUM_CELLS = 4;
  const albumMedia = $derived(
    (album ?? []).flatMap((m) => (m.content.kind === "image" ? [m.content.media] : [])),
  );
  /** Read receipts sit under the album's last photo. */
  const seenBy = $derived(album?.at(-1)?.seen_by ?? msg.seen_by);

  const MENTION_LINK = /^https:\/\/matrix\.to\/#\/[@!#]/;
  const QUICK = ["❤️", "😂", "😮", "😢", "😡", "👍"];
  /** Largest side of an image/video preview in the timeline, in CSS px. */
  const PREVIEW_MAX = 300;

  let pickerOpen = $state(false);
  $effect(() => {
    if (menuOpen || pickerOpen) return onBack(() => ((menuOpen = false), (pickerOpen = false)));
  });
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
      case "audio":
        return "";
      case "sticker":
        return "";
      case "poll":
        return "";
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
    if (album && album.length > 1) return { w: PREVIEW_MAX, h: albumMedia.length === 2 ? PREVIEW_MAX / 2 : PREVIEW_MAX };
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
      // Mention pills point to matrix.to: no browser trip for those (profiles come later).
      if (!MENTION_LINK.test(link.href)) openExternal(link.href).catch(() => {});
    }
  }

  const muted = $derived(["redacted", "unable_to_decrypt", "unsupported"].includes(c.kind));
  const big = $derived(msg.only_emojis && c.kind === "text" && !msg.reply_to);
  /** Photos, videos, albums and stickers are drawn without a bubble background. */
  const bare = $derived(
    ((c.kind === "image" || c.kind === "video") && !text && !msg.reply_to) || c.kind === "sticker",
  );
  const remote = $derived(msg.item_id.startsWith("$"));
  const canReact = $derived(!muted && remote);
  const albumSize = $derived(album && album.length > 1 ? album.length : 0);
  const canSaveToGallery = $derived(albumSize > 0 || ((c.kind === "image" || c.kind === "video") && !muted));
  const hasMenu = $derived(
    msg.can_edit || msg.can_delete || (msg.is_edited && remote) || canSaveToGallery || (!!onThread && remote && msg.can_reply),
  );

  // Right-click (desktop) or long-press (touch) opens the same menu as ⋯.
  let pressTimer: ReturnType<typeof setTimeout> | undefined;
  let pressOrigin = { x: 0, y: 0 };
  function openMenu(event: Event) {
    if (!hasMenu) return;
    event.preventDefault();
    pickerOpen = false;
    menuOpen = true;
  }
  function pressStart(event: PointerEvent) {
    if (event.pointerType !== "touch") return;
    pressOrigin = { x: event.clientX, y: event.clientY };
    if (onLongPress || hasMenu) {
      pressTimer = setTimeout(() => {
        navigator.vibrate?.(10);
        if (onLongPress) onLongPress();
        else openMenu(event);
      }, 450);
    }
  }
  function pressEnd() {
    clearTimeout(pressTimer);
    if (swiping) {
      if (swipeX >= SWIPE_REPLY) onReply();
      swiping = false;
      swipeX = 0;
    }
  }

  // Swipe right to reply (touch). Horizontal moves are ours; vertical ones scroll the timeline.
  const SWIPE_REPLY = 56;
  const SWIPE_MAX = 80;
  let swiping = $state(false);
  let swipeX = $state(0);
  function pressMove(event: PointerEvent) {
    if (event.pointerType !== "touch") return;
    const dx = event.clientX - pressOrigin.x;
    const dy = event.clientY - pressOrigin.y;
    if (Math.hypot(dx, dy) > 10) clearTimeout(pressTimer);
    if (!swiping && msg.can_reply && dx > 12 && dx > Math.abs(dy) * 1.5) {
      swiping = true;
      (event.currentTarget as Element).setPointerCapture(event.pointerId);
    }
    if (!swiping) return;
    // Rubber band past the max, a tick when crossing the threshold.
    const next = dx <= SWIPE_MAX ? Math.max(0, dx) : SWIPE_MAX + (dx - SWIPE_MAX) * 0.2;
    if (swipeX < SWIPE_REPLY && next >= SWIPE_REPLY) navigator.vibrate?.(8);
    swipeX = next;
  }

  // A double-tap reacts: single-tap actions wait a moment to be sure it isn't one.
  let tapTimer: ReturnType<typeof setTimeout> | undefined;
  function afterTap(action: () => void) {
    clearTimeout(tapTimer);
    tapTimer = setTimeout(action, 260);
  }

  function react(key: string) {
    pickerOpen = false;
    onReact(key);
  }

  function pick(action: () => void) {
    menuOpen = false;
    action();
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
      {#if swipeX > 0}
        <span class="swipe-reply" style:opacity={Math.min(1, swipeX / SWIPE_REPLY)} style:scale={swipeX >= SWIPE_REPLY ? 1.15 : 0.8 + (swipeX / SWIPE_REPLY) * 0.2}><Reply size={16} /></span>
      {/if}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="bubble"
        class:big
        class:bare
        class:muted
        class:mentions-me={msg.mentions_me && !msg.is_own}
        class:has-media={box}
        class:sending={msg.send_state === "sending"}
        class:failed={msg.send_state === "failed"}
        title={clockTime(msg.timestamp)}
        ondblclick={() => (clearTimeout(tapTimer), canReact && react(quickReaction))}
        class:swiping
        style:transform={swipeX ? `translateX(${swipeX}px)` : undefined}
        oncontextmenu={openMenu}
        onpointerdown={pressStart}
        onpointerup={pressEnd}
        onpointercancel={pressEnd}
        onpointermove={pressMove}
      >
        {#if msg.reply_to}
          <button class="quote" onclick={() => onJumpTo(msg.reply_to!.event_id)}>
            <strong>{msg.reply_to.sender_name ?? "…"}</strong>
            <span>{msg.reply_to.text ?? "Loading…"}</span>
          </button>
        {/if}

        {#if album && album.length > 1}
          <div class="album" class:two={albumMedia.length === 2} class:three={albumMedia.length === 3} style:width="{box?.w}px" style:aspect-ratio="{box?.w} / {box?.h}">
            {#each albumMedia.slice(0, ALBUM_CELLS) as media, i (media)}
              <button class="cell" onclick={() => afterTap(() => onOpenMedia(i))} aria-label="Open photo {i + 1} of {albumMedia.length}">
                <img src={eventMediaUrl(media, true)} alt="" decoding="async" loading="lazy" />
                {#if i === ALBUM_CELLS - 1 && albumMedia.length > ALBUM_CELLS}
                  <span class="more">+{albumMedia.length - ALBUM_CELLS}</span>
                {/if}
              </button>
            {/each}
          </div>
        {:else if box && (c.kind === "image" || c.kind === "video" || c.kind === "sticker")}
          <button
            class="media"
            class:loaded={mediaLoaded}
            style:width="{box.w}px"
            style:aspect-ratio="{box.w} / {box.h}"
            onclick={() => c.kind !== "sticker" && afterTap(() => onOpenMedia(0))}
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
        {:else if c.kind === "poll"}
          <PollView {roomId} pollId={msg.item_id} poll={c} />
        {:else if c.kind === "audio"}
          <VoicePlayer media={c.media} durationMs={c.duration_ms} waveform={c.waveform} own={msg.is_own} />
        {:else if c.kind === "file"}
          <button class="file" onclick={() => afterTap(onSaveFile)} title="Open or save">
            <span class="file-icon"><FileText size={26} /></span>
            <span class="file-meta">
              <strong>{c.filename}</strong>
              <small>{c.size != null ? `${formatSize(c.size)} · ` : ""}Tap to open or save</small>
            </span>
          </button>
        {/if}

        {#if text || html}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <span class="text" class:rich={html} class:caption={box} onclick={onTextClick}>
            {#if html}{#if c.kind === "emote"}* {msg.sender_name} {/if}{@html html}{:else}{text}{/if}
          </span>
        {/if}
        {#if (c.kind === "text" || c.kind === "notice") && c.link}
          <LinkPreview url={c.link} />
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
            <button onclick={() => ((pickerOpen = !pickerOpen), (menuOpen = false))} aria-label="React"><SmilePlus size={18} /></button>
          {/if}
          {#if msg.can_reply}
            <button onclick={onReply} aria-label="Reply"><Reply size={18} /></button>
          {/if}
          {#if hasMenu}
            <button onclick={() => ((menuOpen = !menuOpen), (pickerOpen = false))} aria-label="More"><Ellipsis size={18} /></button>
          {/if}
        </div>
      {/if}
      <!-- Anchored to the bubble's outer edge so they always open into the chat, never under the list. -->
      {#if pickerOpen || menuOpen}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="outside" onclick={() => ((menuOpen = false), (pickerOpen = false))}></div>
        
      {/if}
      {#if pickerOpen}
        <div class="picker" transition:scale={{ duration: 140, start: 0.8 }}>
          {#each QUICK as key (key)}
            <button onclick={() => react(key)}>{key}</button>
          {/each}
        </div>
      {/if}
      {#if menuOpen}
        <div class="menu" transition:fade={{ duration: 100 }}>
          {#if canSaveToGallery}
            <button onclick={() => pick(onSaveToGallery)}><Download size={16} />{albumSize ? `Save all (${albumSize})` : "Save"}</button>
          {/if}
          {#if pinned !== null && remote && onPin}
            <button onclick={() => pick(() => onPin(!pinned))}>
              {#if pinned}<PinOff size={16} />Unpin{:else}<Pin size={16} />Pin{/if}
            </button>
          {/if}
          {#if onThread && remote && msg.can_reply}
            <button onclick={() => pick(onThread)}><MessagesSquare size={16} />Reply in thread</button>
          {/if}
          {#if msg.can_edit}<button onclick={() => pick(onEdit)}><Pencil size={16} />Edit</button>{/if}
          {#if msg.is_edited && remote}<button onclick={() => pick(onHistory)}><History size={16} />Edit history</button>{/if}
          {#if msg.can_delete}<button class="danger" onclick={() => pick(onDelete)}><Trash2 size={16} />Delete</button>{/if}
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

    {#if msg.thread && msg.thread.replies > 0 && onThread}
      {@const t = msg.thread}
      <button class="thread" onclick={onThread}>
        <MessagesSquare size={15} />
        <strong>{t.replies} {t.replies === 1 ? "reply" : "replies"}</strong>
        {#if t.latest_sender}
          <Avatar name={t.latest_sender} mxc={t.latest_avatar} size={16} />
          <span>{t.latest_text}</span>
        {/if}
      </button>
    {/if}

    {#if msg.send_state === "failed"}
      <span class="status error">Not sent</span>
    {/if}

    {#if seenBy.length}
      <div class="seen" title="Seen by {seenBy.map((r) => r.name).join(', ')}">
        {#each seenBy.slice(0, 5) as r (r.name)}
          <span in:scale={{ duration: 200, start: 0.4 }}><Avatar name={r.name} mxc={r.avatar} size={14} /></span>
        {/each}
        {#if seenBy.length > 5}<small>+{seenBy.length - 5}</small>{/if}
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
    touch-action: pan-y;
    transition:
      opacity 0.2s var(--ease),
      transform 0.25s var(--ease);
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
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
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

  .thread {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: min(320px, 100%);
    margin: 2px 6px 4px;
    padding: 5px 10px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface);
    color: inherit;
    font-size: 0.82rem;
    text-align: left;
    cursor: pointer;
  }
  .thread strong {
    flex: none;
    color: var(--accent-b, var(--text));
  }
  .thread span {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    color: var(--muted);
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
  /* Width from `box`, but never wider than the bubble (phones); aspect-ratio keeps the shape. */
  .media,
  .album {
    max-width: 100%;
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
    display: grid;
    color: var(--accent-a);
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
  /* Touch screens: no hover, and a tap would leave the buttons stuck on. Long-press opens them. */
  @media (hover: none) {
    .actions:not(.open) {
      display: none;
    }
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
    display: flex;
    align-items: center;
    gap: 10px;
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
  /* minmax(0, …): tall photos must not stretch their row past the album's box. */
  .album {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    grid-auto-rows: minmax(0, 1fr);
    gap: 3px;
    border-radius: 14px;
    overflow: hidden;
  }
  .album.two {
    grid-template-rows: minmax(0, 1fr);
  }
  /* Three photos: one wide on top, two below (Messenger). */
  .album.three .cell:first-child {
    grid-column: span 2;
  }
  .cell {
    position: relative;
    padding: 0;
    border: none;
    background: var(--surface-hover);
    cursor: pointer;
    overflow: hidden;
  }
  .cell img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.2s var(--ease);
  }
  .cell:hover img {
    transform: scale(1.04);
  }
  .more {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.5);
    color: white;
    font-size: 1.5rem;
    font-weight: 600;
  }
  .bubble.swiping {
    transition: none;
  }
  .swipe-reply {
    position: absolute;
    left: 0;
    top: 50%;
    translate: 0 -50%;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--surface-2);
    color: var(--text);
    pointer-events: none;
    transition: scale 0.15s var(--ease);
  }
  /* Mention pills (links to matrix.to users/rooms in formatted bodies). */
  .bubble :global(a[href^="https://matrix.to/#/@"]),
  .bubble :global(a[href^="https://matrix.to/#/!"]),
  .bubble :global(a[href^="https://matrix.to/#/#"]) {
    padding: 0 6px;
    border-radius: 999px;
    background: rgb(123 92 255 / 0.22);
    color: inherit;
    font-weight: 600;
    text-decoration: none;
  }
  .bubble.mentions-me {
    box-shadow: 0 0 0 2px var(--accent-b);
  }
  /* Tap anywhere else to close the open menu. */
  .outside {
    position: fixed;
    inset: 0;
    z-index: 5;
  }
  /* Touch: a long-press opens the actions, not the WebView's text selection. */
  @media (hover: none) {
    .bubble {
      -webkit-user-select: none;
      user-select: none;
      -webkit-touch-callout: none;
    }
  }
</style>
