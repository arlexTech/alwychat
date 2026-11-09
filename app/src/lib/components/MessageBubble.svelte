<script lang="ts">
  import { viewOnce } from "$lib/stores/viewOnce.svelte";
  import { tr, trn } from "$lib/i18n/index.svelte";
  import FileText from "@lucide/svelte/icons/file-text";
  import Download from "@lucide/svelte/icons/download";
  import Pin from "@lucide/svelte/icons/pin";
  import PinOff from "@lucide/svelte/icons/pin-off";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Forward from "@lucide/svelte/icons/forward";
  import SquareCheck from "@lucide/svelte/icons/square-check";
  import MediaImage from "./MediaImage.svelte";
  import History from "@lucide/svelte/icons/history";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import SmilePlus from "@lucide/svelte/icons/smile-plus";
  import Plus from "@lucide/svelte/icons/plus";
  import EmojiPicker from "./EmojiPicker.svelte";
  import ReactionsSheet from "./ReactionsSheet.svelte";
  import { members } from "$lib/stores/members.svelte";
  import Reply from "@lucide/svelte/icons/reply";
  import Ellipsis from "@lucide/svelte/icons/ellipsis";
  import MessagesSquare from "@lucide/svelte/icons/messages-square";
  import { fade, scale, slide } from "svelte/transition";
  import { isAndroid, openExternal, retrySend, sendDespiteIdentityChanges, type MessageEntry, type Reaction } from "$lib/backend";
  import MapPin from "@lucide/svelte/icons/map-pin";
  import { liveLocation, untilTime } from "$lib/stores/liveLocation.svelte";
  import { clockTime, messageStamp } from "$lib/format";
  import { eventMediaUrl, mediaUrl } from "$lib/media";
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
    onForward,
    onSelect,
    album = null,
    wide = false,
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
    /** Forward to another chat (absent when the message can't be forwarded). */
    onForward?: () => void;
    /** Start selecting several messages, this one first. */
    onSelect?: () => void;
    /** Whether it's pinned; null when we can't pin in this chat. */
    pinned?: boolean | null;
    onPin?: (pin: boolean) => void;
    /** Several photos drawn as one grid; `msg` is the first. */
    album?: MessageEntry[] | null;
    /** Wide chat (computer): Discord layout, everyone on the left under avatar, name and time,
     * no bubbles, so our messages aren't far from the others'. */
    wide?: boolean;
  } = $props();
  const own = $derived(msg.is_own && !wide);

  /** Grid cells: up to 4 photos, the last one showing "+N" when there are more. */
  const ALBUM_CELLS = 4;
  const albumMedia = $derived(
    (album ?? []).flatMap((m) => (m.content.kind === "image" ? [m.content.media] : [])),
  );
  /** Album photos' blurry previews, by media key. */
  const albumBlur = $derived(
    new Map((album ?? []).flatMap((m) => (m.content.kind === "image" ? [[m.content.media, m.content.blurhash] as const] : []))),
  );
  /** Read receipts sit under the album's last photo. */
  const seenBy = $derived(album?.at(-1)?.seen_by ?? msg.seen_by);

  const MENTION_LINK = /^https:\/\/matrix\.to\/#\/[@!#]/;
  const QUICK = ["❤️", "😂", "😮", "😢", "😡", "👍"];
  /** Largest side of an image/video preview in the timeline, in CSS px. */
  const PREVIEW_MAX = 300;

  let pickerOpen = $state(false);
  /** The full emoji picker, from the quick picker's "+". */
  let anyEmoji = $state(false);
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
      case "location":
      case "live_location":
        return "";
      case "audio":
        return "";
      case "sticker":
        return "";
      case "poll":
        return "";
      case "redacted":
        return tr("Message deleted");
      case "unable_to_decrypt":
        return tr("🔒 Can't read this message yet");
      case "unsupported":
        return tr("Unsupported message");
    }
  });

  /** Sanitised HTML from the core, when the message has a formatted version. */
  // Custom emoji (MSC2545) come as <img src="mxc://…">: loaded from the homeserver through the
  // app's media protocol (the core only lets mxc:// images through).
  // The SDK's own clean-up drops the `data-mx-emoticon` marker: an image whose alt text is
  // ":name:" is a custom emoji too (that's how they're written everywhere).
  const html = $derived(
    "html" in c && c.html
      ? c.html
          .replace(/<img(?![^>]*data-mx-emoticon)(?=[^>]*\balt=":[\p{L}\p{N}_-]+:")/gu, "<img data-mx-emoticon")
          .replace(/src="(mxc:\/\/[^"]+)"/g, (_, mxc: string) => `src="${mediaUrl(mxc, 96) ?? ""}" loading="lazy"`)
      : null,
  );
  /** Only custom emoji (and spaces): shown big, like a message of emoji. */
  const onlyCustomEmoji = $derived(
    !!html && html.includes("data-mx-emoticon") && !html.replace(/<img[^>]*>/g, "").replace(/<[^>]+>/g, "").replace(/&nbsp;/g, "").trim(),
  );

  /** Preview box for photos/videos/stickers, keeping the aspect ratio. */
  const box = $derived.by(() => {
    if (album && album.length > 1) return { w: PREVIEW_MAX, h: albumMedia.length === 2 ? PREVIEW_MAX / 2 : PREVIEW_MAX };
    if (c.kind !== "image" && c.kind !== "video" && c.kind !== "sticker") return null;
    const max = c.kind === "sticker" ? 160 : PREVIEW_MAX;
    // Without a size in the message (HEIC photos, some apps), the photo's own size once loaded;
    // until then a full-width 4:3 box (never the tiny 4×3 px "photo" it used to assume).
    const w0 = c.width ?? natural?.w ?? 4000;
    const h0 = c.height ?? natural?.h ?? 3000;
    // Very long screenshots and panoramas aren't drawn as thin strips: like Messenger, the
    // bubble keeps between 2.5:1 and 1:2.5 and crops the rest (the viewer shows it whole).
    const ratio = Math.min(2.5, Math.max(0.4, w0 / h0));
    const w = ratio >= 1 ? max : max * ratio;
    const h = ratio >= 1 ? max / ratio : max;
    // Small images (stickers, icons) aren't blown up past their own size.
    const k = Math.min(1, Math.max(w0, h0) / max);
    return { w: Math.max(80, Math.round(w * k)), h: Math.max(60, Math.round(h * k)) };
  });
  let mediaLoaded = $state(false);
  /** The loaded photo's size (used when the message doesn't give one). */
  let natural = $state<{ w: number; h: number } | null>(null);

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
  const big = $derived((msg.only_emojis || onlyCustomEmoji) && c.kind === "text" && !msg.reply_to);
  /** Photos, videos, albums and stickers are drawn without a bubble background (not a
   * view-once one: it's an envelope in a bubble). */
  const bare = $derived(
    ((c.kind === "image" || c.kind === "video") && !c.view_once && !text && !msg.reply_to) || c.kind === "sticker",
  );
  const remote = $derived(msg.item_id.startsWith("$"));
  const canReact = $derived(!muted && remote);
  const albumSize = $derived(album && album.length > 1 ? album.length : 0);
  const canSaveToGallery = $derived(albumSize > 0 || ((c.kind === "image" || c.kind === "video") && !muted));
  const hasMenu = $derived(
    msg.can_edit || msg.can_delete || (msg.is_edited && remote) || canSaveToGallery || !!onForward || !!onSelect || (!!onThread && remote && msg.can_reply),
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

  /** Double tap: a ❤️ (Messenger), added once (a second double tap doesn't remove it). */
  function heart() {
    if (!msg.reactions.some((r) => r.key === "❤️" && r.mine)) react("❤️");
  }

  // A double-tap reacts: single-tap actions wait a moment to be sure it isn't one.
  let tapTimer: ReturnType<typeof setTimeout> | undefined;
  function afterTap(action: () => void) {
    clearTimeout(tapTimer);
    tapTimer = setTimeout(action, 260);
  }

  // A tap (or click) on the bubble itself shows the message's date and time under it.
  let timeShown = $state(false);
  function toggleTime(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (swiping || target.closest("a, button, input, video, audio, [data-mx-spoiler]:not(.revealed)")) return;
    afterTap(() => (timeShown = !timeShown));
  }

  // Who reacted: long-press a reaction (touch) or right-click it; hovering shows the names.
  let reactionsFor = $state<string | null>(null);
  let chipPressed = false;
  let chipTimer: ReturnType<typeof setTimeout> | undefined;
  function chipPressStart(e: PointerEvent, key: string) {
    if (e.pointerType !== "touch") return;
    e.stopPropagation();
    chipTimer = setTimeout(() => {
      chipPressed = true;
      navigator.vibrate?.(10);
      reactionsFor = key;
    }, 450);
  }
  const chipPressEnd = () => clearTimeout(chipTimer);
  function reactedBy(r: Reaction): string {
    void members.load(roomId);
    const names = (r.senders ?? []).map((id) => {
      const m = members.get(roomId, id);
      return m.isMe ? tr("You") : m.name;
    });
    return names.length > 12 ? `${names.slice(0, 12).join(", ")}…` : names.join(", ");
  }

  /** Opens below the bubble when there's no room above it. */
  let anyBelow = $state(false);
  function openAnyEmoji(event: MouseEvent) {
    anyBelow = (event.currentTarget as HTMLElement).getBoundingClientRect().top < 340;
    anyEmoji = !anyEmoji;
  }
  function react(key: string) {
    pickerOpen = false;
    anyEmoji = false;
    onReact(key);
  }

  function pick(action: () => void) {
    menuOpen = false;
    action();
  }


  /** `geo:50.85,4.35;u=20` → the point. */
  function geoPoint(uri: string): { lat: number; lon: number } | null {
    const m = /^geo:(-?[\d.]+),(-?[\d.]+)/.exec(uri);
    return m ? { lat: Number(m[1]), lon: Number(m[2]) } : null;
  }
  function openMap(at: { lat: number; lon: number }) {
    // Android hands geo: links to the maps app; elsewhere OpenStreetMap in the browser.
    const url = isAndroid
      ? `geo:${at.lat},${at.lon}?q=${at.lat},${at.lon}`
      : `https://www.openstreetmap.org/?mlat=${at.lat}&mlon=${at.lon}#map=16/${at.lat}/${at.lon}`;
    void openExternal(url);
  }

  let retrying = $state(false);
  async function retry() {
    retrying = true;
    // Stopped by encryption resets: retrying alone would stop again; accept them first.
    const send = msg.identity_changed.length
      ? sendDespiteIdentityChanges(roomId, msg.item_id, msg.identity_changed)
      : retrySend(roomId, msg.item_id);
    await send.catch(() => {});
    retrying = false;
  }
  /** "Margot", from "@margot:server" (the bubble has no member list). */
  const changedNames = $derived(msg.identity_changed.map((id) => id.slice(1).split(":")[0]).join(", "));
</script>

<div class="row" class:own class:wide class:mine={msg.is_own} class:first class:last>
  {#if wide}
    <div class="avatar-slot">
      {#if first}
        <Avatar name={msg.sender_name} mxc={msg.sender_avatar} size={36} />
      {:else}
        <time class="gutter-time">{clockTime(msg.timestamp)}</time>
      {/if}
    </div>
  {:else if showSender && !msg.is_own}
    <!-- Only keeps the room: the photo is in `.line`, level with the bubble's bottom whatever
         sits under it (reactions, "seen by"). -->
    <div class="avatar-slot"></div>
  {/if}

  <div class="stack">
    {#if wide && first}
      <div class="head">
        <strong>{msg.sender_name}</strong>
        <time title={messageStamp(msg.timestamp)}>{messageStamp(msg.timestamp)}</time>
      </div>
    {:else if showSender && !msg.is_own && first}
      <span class="sender">{msg.sender_name}</span>
    {/if}

    <div class="line">
      {#if !wide && showSender && !msg.is_own && last}
        <span class="side-avatar"><Avatar name={msg.sender_name} mxc={msg.sender_avatar} size={28} /></span>
      {/if}
      {#if swipeX > 0}
        <span class="swipe-reply" style:opacity={Math.min(1, swipeX / SWIPE_REPLY)} style:scale={swipeX >= SWIPE_REPLY ? 1.15 : 0.8 + (swipeX / SWIPE_REPLY) * 0.2}><Reply size={16} /></span>
      {/if}
      <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
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
        onclick={toggleTime}
        ondblclick={() => (clearTimeout(tapTimer), canReact && heart())}
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
            <span>{msg.reply_to.text ?? tr("Loading…")}</span>
          </button>
        {/if}

        {#if album && album.length > 1}
          <div class="album" class:two={albumMedia.length === 2} class:three={albumMedia.length === 3} style:width="{box?.w}px" style:aspect-ratio="{box?.w} / {box?.h}">
            {#each albumMedia.slice(0, ALBUM_CELLS) as media, i (media)}
              <button class="cell" onclick={() => afterTap(() => onOpenMedia(i))} aria-label={tr("Open photo {value} of {length}", { value: i + 1, length: albumMedia.length })}>
                <MediaImage src={eventMediaUrl(media, true)} blurhash={albumBlur.get(media)} lazy />
                {#if i === ALBUM_CELLS - 1 && albumMedia.length > ALBUM_CELLS}
                  <span class="more">+{albumMedia.length - ALBUM_CELLS}</span>
                {/if}
              </button>
            {/each}
          </div>
        {:else if (c.kind === "image" || c.kind === "video") && c.view_once}
          {@const opened = viewOnce.opened.has(msg.item_id)}
          <button
            class="view-once"
            class:closed={msg.is_own || opened}
            disabled={msg.is_own || opened || !!msg.send_state}
            onclick={() => afterTap(() => onOpenMedia(0))}
          >
            <span class="once-icon">1</span>
            <span class="file-meta">
              <strong>{c.kind === "video" ? tr("Video") : tr("Photo")}</strong>
              <small>
                {#if msg.is_own}{tr("View once · deleted when opened")}
                {:else if opened}{tr("Opened")}
                {:else}{tr("View once · tap to open")}{/if}
              </small>
            </span>
          </button>
        {:else if box && (c.kind === "image" || c.kind === "video" || c.kind === "sticker")}
          <button
            class="media"
            class:loaded={mediaLoaded}
            style:width="{box.w}px"
            style:aspect-ratio="{box.w} / {box.h}"
            onclick={() => c.kind !== "sticker" && afterTap(() => onOpenMedia(0))}
            aria-label={c.kind === "video" ? tr("Play video") : c.kind === "image" ? tr("Open photo") : c.body}
          >
            {#if c.kind !== "video" || c.has_thumbnail}
              <MediaImage
                src={eventMediaUrl(c.media, true)}
                blurhash={"blurhash" in c ? c.blurhash : null}
                alt={c.kind === "sticker" ? c.body : ""}
                onload={(w, h) => ((mediaLoaded = true), (natural = { w, h }))}
              />
            {/if}
            {#if c.kind === "video"}
              <span class="play">▶</span>
              {#if c.duration_ms}<span class="duration">{formatDuration(c.duration_ms)}</span>{/if}
            {/if}
          </button>
        {:else if c.kind === "location"}
          {@const at = geoPoint(c.geo_uri)}
          <button class="location" onclick={() => at && afterTap(() => openMap(at))} title={tr("Open in maps")}>
            <span class="file-icon"><MapPin size={26} /></span>
            <span class="file-meta">
              <strong>{c.description && !c.description.startsWith("Location: geo:") && !c.description.startsWith("geo:") ? c.description : tr("Location")}</strong>
              <small>{at ? tr("{value}, {value2} · Open in maps", { value: at.lat.toFixed(5), value2: at.lon.toFixed(5) }) : c.geo_uri}</small>
            </span>
          </button>
        {:else if c.kind === "live_location"}
          {@const at = c.geo_uri ? geoPoint(c.geo_uri) : null}
          {@const live = c.is_live && c.ends_at > Date.now()}
          <div class="live-location">
            <button class="location" disabled={!at} onclick={() => at && afterTap(() => openMap(at))} title={tr("Open in maps")}>
              <span class="file-icon" class:live><MapPin size={26} /></span>
              <span class="file-meta">
                <strong>{live ? tr("Live location") : tr("Live location ended")}</strong>
                <small>
                  {#if !at}{tr("Waiting for the first position…")}
                  {:else if live}{c.ends_at - Date.now() > 20 * 3600_000 ? tr("Until stopped") : tr("Until {time}", { time: untilTime(c.ends_at) })}{c.updated ? tr(" · updated {time}", { time: clockTime(c.updated) }) : ""}{tr(" · Open in maps")}
                  {:else}{c.updated ? tr("Last position at {time}", { time: clockTime(c.updated) }) : tr("Last position")}{tr(" · Open in maps")}{/if}
                </small>
              </span>
            </button>
            {#if live && msg.is_own}
              <button class="stop-live" onclick={() => liveLocation.stop(roomId)}>{tr("Stop sharing")}</button>
            {/if}
          </div>
        {:else if c.kind === "poll"}
          <PollView {roomId} pollId={msg.item_id} poll={c} />
        {:else if c.kind === "audio"}
          <VoicePlayer media={c.media} durationMs={c.duration_ms} waveform={c.waveform} own={msg.is_own} />
        {:else if c.kind === "file"}
          <button class="file" onclick={() => afterTap(onSaveFile)} title={tr("Open or save")}>
            <span class="file-icon"><FileText size={26} /></span>
            <span class="file-meta">
              <strong>{c.filename}</strong>
              <small>{tr("{value}Tap to open or save", { value: c.size != null ? `${formatSize(c.size)} · ` : "" })}</small>
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
          <button class="unlock" onclick={onUnlock}>{tr("Unlock")}</button>
        {/if}
        {#if msg.is_edited}
          <button class="edited" onclick={onHistory} disabled={!remote}>{tr("(edited)")}</button>
        {/if}
      </div>

      {#if canReact || msg.can_reply || hasMenu}
        <div class="actions" class:open={menuOpen || pickerOpen}>
          {#if canReact}
            <button onclick={() => ((pickerOpen = !pickerOpen), (menuOpen = false), (anyEmoji = false))} aria-label={tr("React")}><SmilePlus size={18} /></button>
          {/if}
          {#if msg.can_reply}
            <button onclick={onReply} aria-label={tr("Reply")}><Reply size={18} /></button>
          {/if}
          {#if hasMenu}
            <button onclick={() => ((menuOpen = !menuOpen), (pickerOpen = false))} aria-label={tr("More")}><Ellipsis size={18} /></button>
          {/if}
        </div>
      {/if}
      <!-- Anchored to the bubble's outer edge so they always open into the chat, never under the list. -->
      {#if pickerOpen || menuOpen}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="outside" onclick={() => ((menuOpen = false), (pickerOpen = false), (anyEmoji = false))}></div>
        
      {/if}
      {#if pickerOpen}
        <div class="picker" transition:scale={{ duration: 140, start: 0.8 }}>
          {#each QUICK as key (key)}
            <button onclick={() => react(key)}>{key}</button>
          {/each}
          <button class="more" class:on={anyEmoji} onclick={openAnyEmoji} aria-label={tr("Other reaction")}><Plus size={20} /></button>
        </div>
        {#if anyEmoji}
          <div class="any-emoji" class:below={anyBelow} transition:scale={{ duration: 140, start: 0.9 }}><EmojiPicker onPick={react} /></div>
        {/if}
      {/if}
      {#if menuOpen}
        <div class="menu" transition:fade={{ duration: 100 }}>
          {#if canSaveToGallery}
            <button onclick={() => pick(onSaveToGallery)}><Download size={16} />{albumSize ? tr("Save all ({albumSize})", { albumSize: albumSize }) : tr("Save")}</button>
          {/if}
          {#if pinned !== null && remote && onPin}
            <button onclick={() => pick(() => onPin(!pinned))}>
              {#if pinned}<PinOff size={16} />{tr("Unpin")}{:else}<Pin size={16} />{tr("Pin")}{/if}
            </button>
          {/if}
          {#if onThread && remote && msg.can_reply}
            <button onclick={() => pick(onThread)}><MessagesSquare size={16} />{tr("Reply in thread")}</button>
          {/if}
          {#if onForward}<button onclick={() => pick(onForward)}><Forward size={16} />{tr("Forward")}</button>{/if}
          {#if onSelect}<button onclick={() => pick(onSelect)}><SquareCheck size={16} />{tr("Select")}</button>{/if}
          {#if msg.can_edit}<button onclick={() => pick(onEdit)}><Pencil size={16} />{tr("Edit")}</button>{/if}
          {#if msg.is_edited && remote}<button onclick={() => pick(onHistory)}><History size={16} />{tr("Edit history")}</button>{/if}
          {#if msg.can_delete}<button class="danger" onclick={() => pick(onDelete)}><Trash2 size={16} />{tr("Delete")}</button>{/if}
        </div>
      {/if}
    </div>

    {#if msg.reactions.length}
      <div class="reactions">
        {#each msg.reactions as r (r.key)}
          <button
            class="chip"
            class:mine={r.mine}
            title={reactedBy(r)}
            onclick={() => (chipPressed ? (chipPressed = false) : react(r.key))}
            oncontextmenu={(e) => (e.preventDefault(), (reactionsFor = r.key))}
            onpointerdown={(e) => chipPressStart(e, r.key)}
            onpointerup={chipPressEnd}
            onpointercancel={chipPressEnd}
            onpointerleave={chipPressEnd}
            in:scale={{ duration: 180, start: 0.5 }}
          >
            {r.key}{#if r.count > 1}<span>{r.count}</span>{/if}
          </button>
        {/each}
      </div>
    {/if}

    {#if reactionsFor !== null}
      <ReactionsSheet {roomId} reactions={msg.reactions} initial={reactionsFor || null} onClose={() => (reactionsFor = null)} />
    {/if}

    {#if msg.thread && msg.thread.replies > 0 && onThread}
      {@const t = msg.thread}
      <button class="thread" onclick={onThread}>
        <MessagesSquare size={15} />
        <strong>{trn(t.replies, "{n} reply", "{n} replies")}</strong>
        {#if t.latest_sender}
          <Avatar name={t.latest_sender} mxc={t.latest_avatar} size={16} />
          <span>{t.latest_text}</span>
        {/if}
      </button>
    {/if}

    {#if timeShown}
      <span class="when" transition:slide={{ duration: 140 }}>{messageStamp(msg.timestamp)}</span>
    {/if}

    {#if msg.send_state === "failed" && msg.identity_changed.length}
      <span class="status error">
        {tr("Not sent: {names} reset their encryption (new device or lost keys). Make sure it's really them. ·", { names: changedNames })}
        <button class="status-action" disabled={retrying} onclick={retry}>{retrying ? tr("Sending…") : tr("Send anyway")}</button> ·
        <button class="status-action" onclick={onDelete}>{tr("Delete")}</button>
      </span>
    {:else if msg.send_state === "failed"}
      <span class="status error">
        {tr("Not sent ·")}
        <button class="status-action" disabled={retrying} onclick={retry}>{retrying ? tr("Retrying…") : tr("Retry")}</button> ·
        <button class="status-action" onclick={onDelete}>{tr("Delete")}</button>
      </span>
    {/if}

    {#if seenBy.length}
      <div class="seen" title={tr("Seen by {value}", { value: seenBy.map((r) => r.name).join(', ') })}>
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
  .side-avatar {
    position: absolute;
    bottom: 0;
    left: -36px;
    display: flex;
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
    background: var(--incoming, var(--surface-2));
    overflow-wrap: anywhere;
    line-height: 1.35;
    -webkit-user-select: text;
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
    background: var(--bubble, var(--accent));
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
  .picker .more {
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  .picker .more:hover {
    transform: none;
    background: var(--surface);
  }
  .picker .more.on {
    background: var(--accent);
    color: #fff;
  }
  .any-emoji {
    position: absolute;
    bottom: calc(100% + 54px);
    left: 0;
    z-index: 5;
    width: min(340px, 86vw);
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 16px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.4);
  }
  .any-emoji.below {
    top: calc(100% + 6px);
    bottom: auto;
  }
  .own .any-emoji {
    left: auto;
    right: 0;
  }
  .any-emoji :global(.picker) {
    border-top: none;
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
  .status-action {
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    font-weight: 600;
    text-decoration: underline;
    cursor: pointer;
  }
  .bubble.bare {
    padding: 0;
    background: none;
  }
  /* Custom emoji sit in the text like emoji; other inline images stay small. */
  .text :global(img[data-mx-emoticon]) {
    height: 1.5em;
    width: auto;
    max-width: 6em;
    vertical-align: -0.35em;
    object-fit: contain;
  }
  .text :global(img:not([data-mx-emoticon])) {
    max-width: 100%;
    max-height: 240px;
    border-radius: 8px;
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
  .bare .media {
    background-color: transparent;
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
  .live-location {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .file-icon.live {
    color: #31d158;
  }
  .stop-live {
    align-self: flex-start;
    padding: 5px 12px;
    border: none;
    border-radius: 999px;
    background: var(--danger);
    color: white;
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
  }
  .file,
  .location {
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
  /* A chat theme makes the surfaces see-through for its panels; these float over messages. */
  :global(.chat-pane.scenic) .menu,
  :global(.chat-pane.scenic) .picker,
  :global(.chat-pane.scenic) .any-emoji,
  :global(.chat-pane.scenic) .actions > button {
    --surface: #15181f;
    --surface-2: #1c2029;
    --surface-hover: #2a2f3b;
    --border: #2c3240;
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
  .when {
    padding: 2px 6px 4px;
    color: var(--muted);
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
  }
  .row.own .when {
    align-self: flex-end;
  }

  /* Wide (Discord) layout. */
  .row.wide {
    align-items: flex-start;
    gap: 12px;
    padding: 1px 20px;
  }
  .row.wide.first {
    margin-top: 14px;
  }
  .row.wide:hover {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .wide .avatar-slot {
    width: 36px;
  }
  .wide.first .avatar-slot {
    padding-top: 2px;
  }
  .gutter-time {
    display: block;
    padding-top: 2px;
    color: var(--muted);
    font-size: 0.68rem;
    font-variant-numeric: tabular-nums;
    text-align: right;
    opacity: 0;
  }
  .row.wide:hover .gutter-time {
    opacity: 1;
  }
  .wide .stack {
    flex: 1;
    max-width: min(860px, 100%);
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin-bottom: 1px;
  }
  .head strong {
    font-size: 0.95rem;
  }
  .mine .head strong {
    color: var(--accent);
  }
  .head time {
    color: var(--muted);
    font-size: 0.72rem;
  }
  .wide .bubble {
    padding: 0;
    border-radius: 0;
    background: none;
    color: inherit;
  }
  .wide .bubble.has-media {
    border-radius: 12px;
  }
  .wide .bubble.muted {
    border: none;
  }
  .wide .bubble.big {
    padding: 0;
  }
  /* No bubble edge to tuck under: reactions go below the message, not over its text. */
  .wide .reactions {
    margin: 4px 0 2px;
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
  .cell :global(img) {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.2s var(--ease);
  }
  .cell:hover :global(img) {
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
  .view-once {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 200px;
    padding: 10px 14px;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .view-once:disabled {
    cursor: default;
  }
  .once-icon {
    width: 34px;
    height: 34px;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: 50%;
    border: 2px dashed currentColor;
    font-weight: 700;
  }
  .view-once.closed .once-icon {
    border-style: solid;
    opacity: 0.6;
  }
</style>
