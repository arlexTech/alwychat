<script lang="ts">
  import { tick, untrack } from "svelte";
  import { fade, fly } from "svelte/transition";
  import {
    chatMediaHd,
    deleteMessage,
    editHistory,
    editMessage,
    markRead,
    saveMedia,
    sendFile,
    sendMessage,
    setChatMediaHd,
    setTyping,
    toggleReaction,
    type MessageEntry,
    type Revision,
    type RoomSummary,
  } from "$lib/backend";
  import { clockTime, dayLabel, shortTime } from "$lib/format";
  import { chat } from "$lib/stores/chat.svelte";
  import { encryption } from "$lib/stores/encryption.svelte";
  import Avatar from "./Avatar.svelte";
  import MediaViewer from "./MediaViewer.svelte";
  import MentionSuggestions from "./MentionSuggestions.svelte";
  import { MentionAutocomplete } from "$lib/mentions.svelte";
  import { saveToGallery } from "$lib/save";
  import { onBack as closeOnBack } from "$lib/back.svelte";
  import MessageBubble from "./MessageBubble.svelte";

  let { room, onBack }: { room: RoomSummary; onBack: () => void } = $props();

  /** Messages from the same sender closer than this are drawn as one run. */
  const GROUP_GAP_MS = 5 * 60_000;
  /** Until per-chat quick reactions (com.alwy.chat.room.quick_reaction) land. */
  const QUICK_REACTION = "❤️";

  /** Our typing notice is refreshed this often while typing, and dropped after this much idle. */
  const TYPING_REFRESH_MS = 4_000;
  const TYPING_IDLE_MS = 5_000;

  let draft = $state("");
  const mention = new MentionAutocomplete();
  let replyTo = $state<MessageEntry | null>(null);
  /** The message being edited: the composer then replaces its text instead of sending. */
  let editing = $state<MessageEntry | null>(null);
  let sendError = $state<string | null>(null);
  let toast = $state<string | null>(null);
  type ViewerItem = { media: string; kind: "image" | "video"; caption: string | null };
  let viewer = $state<{ items: ViewerItem[]; index: number } | null>(null);
  /** Files picked, dropped or pasted, waiting for Send. */
  let pending = $state<{ file: File; url: string | null }[]>([]);
  let dragging = $state(false);
  let sendingFiles = $state(false);
  let fileInput: HTMLInputElement;
  let chatMenuOpen = $state(false);
  /** This chat's HD choice: null = app default. */
  let chatHd = $state<boolean | null>(null);
  let confirmDelete = $state<MessageEntry | null>(null);
  let history = $state<{ revisions: Revision[] | null; error: string | null } | null>(null);

  // Android back closes these before the chat itself.
  $effect(() => {
    if (viewer) return closeOnBack(() => (viewer = null));
  });
  $effect(() => {
    if (history) return closeOnBack(() => (history = null));
  });
  $effect(() => {
    if (chatMenuOpen) return closeOnBack(() => (chatMenuOpen = false));
  });
  $effect(() => {
    if (editing) return closeOnBack(cancelEdit);
  });
  $effect(() => {
    if (replyTo) return closeOnBack(() => (replyTo = null));
  });
  let scroller: HTMLDivElement;
  let input: HTMLTextAreaElement;

  $effect(() => {
    const id = room.id;
    draft = "";
    untrack(() => mention.reset(id, !room.is_dm));
    replyTo = null;
    editing = null;
    sendError = null;
    untrack(() => {
      clearPending();
      chatMenuOpen = false;
      chat.open(id);
    });
    chatHd = null;
    chatMediaHd(id).then((hd) => room.id === id && (chatHd = hd)).catch(() => {});
    return () => stopTyping(id);
  });

  // Typing notices: on while there's fresh input, off after a pause, on send, or on leaving.
  let typingSentAt = 0;
  let typingIdle: ReturnType<typeof setTimeout> | undefined;
  function onInput() {
    autosize();
    mention.update(input.value, input.selectionStart ?? input.value.length);
    if (editing) return;
    if (!draft.trim()) return stopTyping(room.id);
    const now = Date.now();
    if (now - typingSentAt > TYPING_REFRESH_MS) {
      typingSentAt = now;
      setTyping(room.id, true).catch(() => {});
    }
    clearTimeout(typingIdle);
    typingIdle = setTimeout(() => stopTyping(room.id), TYPING_IDLE_MS);
  }
  function stopTyping(roomId: string) {
    clearTimeout(typingIdle);
    if (!typingSentAt) return;
    typingSentAt = 0;
    setTyping(roomId, false).catch(() => {});
  }

  function flash(message: string) {
    toast = message;
    setTimeout(() => toast === message && (toast = null), 3500);
  }

  /** Photos from one sender closer than this are shown as one album, even without a tag. */
  const BURST_GAP_MS = 60_000;

  type Row =
    | {
        kind: "message";
        key: string;
        msg: MessageEntry;
        first: boolean;
        last: boolean;
        /** Several photos shown as one grid (com.alwy.chat.gallery, or a burst). */
        album: MessageEntry[] | null;
      }
    | { kind: "day"; key: string; label: string }
    | { kind: "notice"; key: string; text: string }
    | { kind: "unread"; key: string }
    | { kind: "start"; key: string };

  const rows = $derived.by(() => {
    const out: Row[] = [];
    for (const e of chat.entries) {
      switch (e.type) {
        case "message": {
          const prev = out.at(-1);
          if (prev?.kind === "message" && joinsAlbum(prev.album?.at(-1) ?? prev.msg, e)) {
            prev.album = [...(prev.album ?? [prev.msg]), e];
            break;
          }
          const joins =
            prev?.kind === "message" &&
            prev.msg.sender === e.sender &&
            e.timestamp - prev.msg.timestamp < GROUP_GAP_MS;
          if (joins) prev.last = false;
          out.push({ kind: "message", key: e.key, msg: e, first: !joins, last: true, album: null });
          break;
        }
        case "date_divider":
          out.push({ kind: "day", key: e.key, label: dayLabel(e.timestamp) });
          break;
        case "notice":
          out.push({ kind: "notice", key: e.key, text: e.text });
          break;
        case "read_marker":
          out.push({ kind: "unread", key: e.key });
          break;
        case "timeline_start":
          out.push({ kind: "start", key: e.key });
          break;
      }
    }
    // A read marker right at the bottom says nothing.
    if (out.at(-1)?.kind === "unread") out.pop();
    return out;
  });

  function joinsAlbum(a: MessageEntry, b: MessageEntry) {
    const ca = a.content;
    const cb = b.content;
    if (ca.kind !== "image" || cb.kind !== "image" || a.sender !== b.sender || b.reply_to) return false;
    if (ca.gallery && cb.gallery) return ca.gallery.id === cb.gallery.id;
    return !ca.caption && !cb.caption && b.timestamp - a.timestamp < BURST_GAP_MS;
  }

  // The list is laid out bottom-up (column-reverse): scrollTop 0 is the newest message,
  // and the browser keeps that anchor while older messages are prepended.
  function onScroll() {
    const fromTop = scroller.scrollHeight + scroller.scrollTop - scroller.clientHeight;
    if (fromTop < 400) chat.older();
  }

  // Read receipts: whenever new content arrives while we're at the bottom and focused.
  let readTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    chat.entries.length;
    const id = room.id;
    clearTimeout(readTimer);
    readTimer = setTimeout(() => {
      if (document.hasFocus() && scroller && Math.abs(scroller.scrollTop) < 40) {
        markRead(id).catch(() => {});
      }
    }, 600);
    return () => clearTimeout(readTimer);
  });

  async function send(event?: SubmitEvent) {
    event?.preventDefault();
    const body = draft.trim();
    if (pending.length && !editing) return sendAttachments(body);
    if (!body) return;
    if (editing) return saveEdit(editing, body);
    stopTyping(room.id);
    const reply = replyTo?.item_id ?? null;
    const mentions = mention.picked;
    mention.clear();
    draft = "";
    replyTo = null;
    sendError = null;
    await tick();
    autosize();
    scroller.scrollTo({ top: 0, behavior: "smooth" });
    try {
      await sendMessage(room.id, body, reply, mentions);
    } catch (e) {
      sendError = String(e);
      draft ||= body;
    }
  }

  // ── Attachments ──────────────────────────────────────────────────────────
  function addFiles(files: Iterable<File>) {
    for (const file of files) {
      const url = file.type.startsWith("image/") ? URL.createObjectURL(file) : null;
      pending.push({ file, url });
    }
    input?.focus();
  }

  function removePending(i: number) {
    const [p] = pending.splice(i, 1);
    if (p?.url) URL.revokeObjectURL(p.url);
  }

  function clearPending() {
    for (const p of pending) if (p.url) URL.revokeObjectURL(p.url);
    pending = [];
  }

  /** Photos go out as one album; a single file takes the text as caption, else it's sent first. */
  async function sendAttachments(text: string) {
    const files = pending.map((p) => p.file);
    clearPending();
    draft = "";
    sendError = null;
    stopTyping(room.id);
    await tick();
    autosize();
    scroller.scrollTo({ top: 0, behavior: "smooth" });
    sendingFiles = true;
    try {
      if (text && files.length > 1) await sendMessage(room.id, text, null);
      const photos = files.filter((f) => f.type.startsWith("image/") && f.type !== "image/gif").length;
      const galleryId = photos > 1 ? crypto.randomUUID() : null;
      let index = 0;
      // One at a time: the send queue keeps them in order and the album stays together.
      for (const file of files) {
        const isPhoto = file.type.startsWith("image/") && file.type !== "image/gif";
        const gallery = galleryId && isPhoto ? { id: galleryId, index: index++, count: photos } : null;
        await sendFile(room.id, file, { caption: files.length === 1 ? text : null, gallery });
      }
    } catch (e) {
      sendError = String(e);
    } finally {
      sendingFiles = false;
    }
  }

  function onPaste(event: ClipboardEvent) {
    const files = [...(event.clipboardData?.files ?? [])];
    if (files.length) {
      event.preventDefault();
      addFiles(files);
    }
  }

  function onDragOver(event: DragEvent) {
    if (!event.dataTransfer?.types.includes("Files")) return;
    event.preventDefault();
    dragging = true;
  }

  function onDrop(event: DragEvent) {
    event.preventDefault();
    dragging = false;
    if (event.dataTransfer?.files.length) addFiles(event.dataTransfer.files);
  }

  async function chooseHd(hd: boolean | null) {
    chatMenuOpen = false;
    const previous = chatHd;
    chatHd = hd;
    try {
      await setChatMediaHd(room.id, hd);
    } catch (e) {
      chatHd = previous;
      flash(`Couldn't save: ${e}`);
    }
  }

  async function saveEdit(msg: MessageEntry, body: string) {
    const mentions = mention.picked;
    mention.clear();
    editing = null;
    draft = "";
    await tick();
    autosize();
    if (body === editableText(msg)) return;
    try {
      await editMessage(room.id, msg.item_id, body, mentions);
    } catch (e) {
      sendError = String(e);
    }
  }

  function startEdit(msg: MessageEntry) {
    replyTo = null;
    editing = msg;
    draft = editableText(msg);
    tick().then(() => {
      autosize();
      input.focus();
      input.setSelectionRange(draft.length, draft.length);
    });
  }

  function cancelEdit() {
    mention.clear();
    editing = null;
    draft = "";
    tick().then(autosize);
  }

  function editableText(msg: MessageEntry) {
    const c = msg.content;
    return "body" in c ? c.body : "";
  }

  async function doDelete(msg: MessageEntry) {
    confirmDelete = null;
    try {
      await deleteMessage(room.id, msg.item_id);
    } catch (e) {
      sendError = String(e);
    }
  }

  async function showHistory(msg: MessageEntry) {
    history = { revisions: null, error: null };
    try {
      history = { revisions: await editHistory(room.id, msg.item_id), error: null };
    } catch (e) {
      history = { revisions: [], error: String(e) };
    }
  }

  function openMedia(msgs: MessageEntry[], index: number) {
    const items: ViewerItem[] = [];
    for (const m of msgs) {
      const c = m.content;
      if (c.kind === "image" || c.kind === "video") items.push({ media: c.media, kind: c.kind, caption: c.caption });
    }
    if (items.length) viewer = { items, index: Math.min(index, items.length - 1) };
  }

  async function saveVisualMedia(msgs: MessageEntry[]) {
    const media = msgs.flatMap((m) =>
      m.content.kind === "image" || m.content.kind === "video" ? [m.content.media] : [],
    );
    flash(media.length > 1 ? `Saving ${media.length} items…` : "Saving…");
    flash(await saveToGallery(media));
  }

  async function saveFile(msg: MessageEntry) {
    const c = msg.content;
    if (c.kind !== "file") return;
    try {
      const path = await saveMedia(c.media, false);
      flash(`Saved to ${path.replace(/^\/home\/[^/]+/, "~")}`);
    } catch (e) {
      flash(`Couldn't save: ${e}`);
    }
  }

  function pickMention(s?: Parameters<MentionAutocomplete["pick"]>[1]) {
    const next = mention.pick(draft, s);
    draft = next.draft;
    tick().then(() => {
      input.setSelectionRange(next.caret, next.caret);
      autosize();
    });
  }

  function onKeydown(event: KeyboardEvent) {
    if (mention.open && !event.isComposing) {
      const keys: Record<string, () => void> = {
        ArrowDown: () => mention.move(1),
        ArrowUp: () => mention.move(-1),
        Enter: () => pickMention(),
        Tab: () => pickMention(),
        Escape: () => (mention.query = null),
      };
      if (event.key in keys && !(event.key === "Enter" && event.shiftKey)) {
        event.preventDefault();
        keys[event.key]();
        return;
      }
    }
    if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      send();
    } else if (event.key === "Escape") {
      if (editing) cancelEdit();
      else replyTo = null;
    } else if (event.key === "ArrowUp" && !draft) {
      // Like Element/Telegram: ↑ in an empty composer edits your last message.
      const last = rows.findLast((r) => r.kind === "message" && r.msg.can_edit);
      if (last?.kind === "message") {
        event.preventDefault();
        startEdit(last.msg);
      }
    }
  }

  function autosize() {
    input.style.height = "auto";
    input.style.height = `${Math.min(input.scrollHeight, 160)}px`;
  }

  function react(msg: MessageEntry, key: string) {
    toggleReaction(room.id, msg.item_id, key).catch((e) => (sendError = String(e)));
  }

  function startReply(msg: MessageEntry) {
    replyTo = msg;
    input.focus();
  }

  function jumpTo(eventId: string) {
    const el = scroller.querySelector<HTMLElement>(`[data-event="${CSS.escape(eventId)}"]`);
    if (!el) return;
    el.scrollIntoView({ behavior: "smooth", block: "center" });
    el.animate([{ background: "rgb(123 92 255 / 0.25)" }, { background: "transparent" }], {
      duration: 1200,
      easing: "ease-out",
    });
  }

  function replyText(msg: MessageEntry) {
    const c = msg.content;
    if ("body" in c && c.body) return c.body;
    if ("label" in c) return c.label;
    if (c.kind === "image") return "📷 Photo";
    if (c.kind === "video") return "🎬 Video";
    if (c.kind === "file") return `📄 ${c.filename}`;
    return "Message";
  }

  /** "14:32" today, "Tue 14:32" / "12 Mar 14:32" before. */
  function revisionTime(ms: number) {
    const day = shortTime(ms);
    const time = clockTime(ms);
    return day === time ? time : `${day} ${time}`;
  }

  const typingText = $derived.by(() => {
    const t = chat.typing;
    if (t.length === 0) return null;
    if (room.is_dm) return "typing";
    if (t.length === 1) return `${t[0]} is typing`;
    if (t.length === 2) return `${t[0]} and ${t[1]} are typing`;
    return "Several people are typing";
  });
</script>

<header class="chat-header">
  <button class="back" onclick={onBack} aria-label="Back">‹</button>
  <Avatar name={room.name} mxc={room.avatar} size={40} />
  <div class="title">
    <strong>{room.name}</strong>
    {#if room.is_encrypted}<span class="sub">🔒 End-to-end encrypted</span>{/if}
  </div>
  <button class="chat-menu-button" onclick={() => (chatMenuOpen = !chatMenuOpen)} aria-label="Chat options">⋯</button>
  {#if chatMenuOpen}
    <div class="chat-menu" transition:fade={{ duration: 100 }}>
      <p>Photos sent in this chat</p>
      <button class:active={chatHd === null} onclick={() => chooseHd(null)}>App default</button>
      <button class:active={chatHd === true} onclick={() => chooseHd(true)}>Always HD</button>
      <button class:active={chatHd === false} onclick={() => chooseHd(false)}>Always compressed</button>
    </div>
  {/if}
</header>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="scroller"
  bind:this={scroller}
  onscroll={onScroll}
  ondragover={onDragOver}
  ondragleave={(e) => e.currentTarget === e.target && (dragging = false)}
  ondrop={onDrop}
>
  {#if dragging}
    <div class="drop-hint" transition:fade={{ duration: 120 }}>Drop to send</div>
  {/if}
  <div class="timeline">
    {#if chat.error}
      <p class="center error">{chat.error}</p>
    {/if}
    {#if chat.loadingOlder}
      <div class="spinner" aria-label="Loading older messages"></div>
    {/if}

    {#each rows as row (row.key)}
      {#if row.kind === "message"}
        <div data-event={row.msg.item_id}>
          <MessageBubble
            msg={row.msg}
            first={row.first}
            last={row.last}
            showSender={!room.is_dm}
            quickReaction={QUICK_REACTION}
            onReact={(key) => react(row.msg, key)}
            onReply={() => startReply(row.msg)}
            onJumpTo={jumpTo}
            onUnlock={() => (encryption.sheetOpen = true)}
            onEdit={() => startEdit(row.msg)}
            onDelete={() => (confirmDelete = row.msg)}
            onHistory={() => showHistory(row.msg)}
            album={row.album}
            onOpenMedia={(i) => openMedia(row.album ?? [row.msg], i)}
            onSaveFile={() => saveFile(row.msg)}
            onSaveToGallery={() => saveVisualMedia(row.album ?? [row.msg])}
          />
        </div>
      {:else if row.kind === "day"}
        <div class="day"><span>{row.label}</span></div>
      {:else if row.kind === "notice"}
        <p class="center notice">{row.text}</p>
      {:else if row.kind === "unread"}
        <div class="unread"><span>New messages</span></div>
      {:else if row.kind === "start"}
        <p class="center notice">This is the beginning of {room.name} ✨</p>
      {/if}
    {/each}

    {#if chat.loaded && rows.length === 0 && chat.atStart}
      <p class="center notice">No messages yet. Say hi 👋</p>
    {/if}

    {#if typingText}
      <div class="typing" transition:fade={{ duration: 150 }}>
        <span class="dots"><i></i><i></i><i></i></span>
        {typingText}
      </div>
    {/if}
  </div>
</div>

<form class="composer" onsubmit={send}>
  {#if editing}
    <div class="replying editing" transition:fly={{ y: 8, duration: 160 }}>
      <div>
        <strong>Editing message</strong>
        <span>{editableText(editing)}</span>
      </div>
      <button type="button" onclick={cancelEdit} aria-label="Cancel edit">×</button>
    </div>
  {:else if replyTo}
    <div class="replying" transition:fly={{ y: 8, duration: 160 }}>
      <div>
        <strong>Replying to {replyTo.is_own ? "yourself" : replyTo.sender_name}</strong>
        <span>{replyText(replyTo)}</span>
      </div>
      <button type="button" onclick={() => (replyTo = null)} aria-label="Cancel reply">×</button>
    </div>
  {/if}
  {#if pending.length}
    <div class="pending" transition:fly={{ y: 8, duration: 160 }}>
      {#each pending as p, i (p)}
        <div class="pending-item">
          {#if p.url}<img src={p.url} alt="" />{:else}<span class="pending-file">📄<small>{p.file.name}</small></span>{/if}
          <button type="button" onclick={() => removePending(i)} aria-label="Remove {p.file.name}">×</button>
        </div>
      {/each}
    </div>
  {/if}
  {#if mention.open}
    <MentionSuggestions suggestions={mention.suggestions} index={mention.index} onPick={pickMention} />
  {/if}
  {#if sendingFiles}<p class="send-status">Sending…</p>{/if}
  {#if sendError}<p class="send-error">{sendError}</p>{/if}
  <div class="bar">
    <button type="button" class="attach" onclick={() => fileInput.click()} aria-label="Attach photos or files">📎</button>
    <input
      bind:this={fileInput}
      type="file"
      multiple
      hidden
      onchange={(e) => {
        addFiles(e.currentTarget.files ?? []);
        e.currentTarget.value = "";
      }}
    />
    <textarea
      bind:this={input}
      bind:value={draft}
      oninput={onInput}
      onblur={() => (stopTyping(room.id), (mention.query = null))}
      onclick={() => mention.update(input.value, input.selectionStart ?? input.value.length)}
      onpaste={onPaste}
      onkeydown={onKeydown}
      rows="1"
      placeholder={pending.length ? "Add a caption" : "Message"}
    ></textarea>
    {#if draft.trim() || editing || pending.length}
      <button class="send" aria-label={editing ? "Save edit" : "Send"} transition:fly={{ x: 8, duration: 140 }}
        >{editing ? "✓" : "➤"}</button
      >
    {:else}
      <button
        type="button"
        class="quick"
        aria-label="Send {QUICK_REACTION}"
        onclick={() => sendMessage(room.id, QUICK_REACTION, null).catch((e) => (sendError = String(e)))}
      >{QUICK_REACTION}</button>
    {/if}
  </div>
</form>

{#if toast}<div class="toast" transition:fly={{ y: 12, duration: 180 }}>{toast}</div>{/if}

{#if viewer}
  <MediaViewer items={viewer.items} index={viewer.index} onClose={() => (viewer = null)} />
{/if}

{#if confirmDelete}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="backdrop" transition:fade={{ duration: 120 }} onclick={() => (confirmDelete = null)}></div>
  <div class="dialog" role="alertdialog" aria-labelledby="del-title" transition:fly={{ y: 16, duration: 180 }}>
    <h3 id="del-title">Delete this message?</h3>
    <p>It will be removed for everyone in this chat.</p>
    <div class="dialog-buttons">
      <button onclick={() => (confirmDelete = null)}>Cancel</button>
      <button class="danger" onclick={() => doDelete(confirmDelete!)}>Delete</button>
    </div>
  </div>
{/if}

{#if history}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="backdrop" transition:fade={{ duration: 120 }} onclick={() => (history = null)}></div>
  <div class="dialog" role="dialog" aria-labelledby="hist-title" transition:fly={{ y: 16, duration: 180 }}>
    <h3 id="hist-title">Edit history</h3>
    {#if history.error}
      <p class="error">{history.error}</p>
    {:else if !history.revisions}
      <p>Loading…</p>
    {:else}
      <ol class="revisions">
        {#each history.revisions as rev, i (i)}
          <li class:current={i === history.revisions.length - 1}>
            <small>{i === 0 ? "Original" : `Edit ${i}`}{rev.timestamp ? ` · ${revisionTime(rev.timestamp)}` : ""}</small>
            <span>{rev.text}</span>
          </li>
        {/each}
      </ol>
    {/if}
    <div class="dialog-buttons">
      <button onclick={() => (history = null)}>Close</button>
    </div>
  </div>
{/if}

<style>
  .chat-header {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }
  .title {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .title strong {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .sub {
    font-size: 0.8rem;
    color: var(--muted);
  }
  .back {
    display: none;
    border: none;
    background: none;
    font-size: 2rem;
    line-height: 1;
    cursor: pointer;
  }
  @media (max-width: 720px) {
    .back {
      display: block;
    }
  }

  .scroller {
    flex: 1;
    display: flex;
    flex-direction: column-reverse;
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  .timeline {
    padding: 12px 0 8px;
  }
  .center {
    margin: 10px 24px;
    text-align: center;
  }
  .notice {
    font-size: 0.82rem;
    color: var(--muted);
  }
  .error {
    color: var(--danger);
  }
  .day,
  .unread {
    display: flex;
    justify-content: center;
    margin: 16px 0 6px;
    font-size: 0.78rem;
    color: var(--muted);
  }
  .day span {
    padding: 3px 12px;
    border-radius: 999px;
    background: var(--surface-2);
  }
  .unread {
    align-items: center;
    gap: 10px;
    padding: 0 16px;
    color: var(--accent-b);
  }
  .unread::before,
  .unread::after {
    content: "";
    flex: 1;
    height: 1px;
    background: currentColor;
    opacity: 0.4;
  }
  .spinner {
    width: 24px;
    height: 24px;
    margin: 8px auto;
    border-radius: 50%;
    border: 3px solid var(--border);
    border-top-color: var(--accent-b);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .composer {
    padding: 8px 12px 12px;
    border-top: 1px solid var(--border);
  }
  .replying {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 4px 8px;
    padding: 6px 10px;
    border-left: 3px solid var(--accent-a);
    border-radius: 8px;
    background: var(--surface-2);
    font-size: 0.85rem;
  }
  .replying div {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .replying span {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    color: var(--muted);
  }
  .replying.editing {
    border-left-color: var(--accent-b);
  }
  .replying button {
    border: none;
    background: none;
    font-size: 1.3rem;
    color: var(--muted);
    cursor: pointer;
  }
  .send-error {
    margin: 0 8px 6px;
    font-size: 0.8rem;
    color: var(--danger);
  }
  .bar {
    display: flex;
    align-items: flex-end;
    gap: 8px;
  }
  textarea {
    flex: 1;
    max-height: 160px;
    padding: 10px 16px;
    border: none;
    border-radius: 20px;
    background: var(--surface-2);
    color: inherit;
    font: inherit;
    line-height: 1.35;
    resize: none;
    outline: none;
  }
  textarea:focus {
    box-shadow: 0 0 0 2px var(--accent-a);
  }
  .send,
  .quick {
    flex: none;
    width: 40px;
    height: 40px;
    border: none;
    border-radius: 50%;
    cursor: pointer;
    transition: transform 0.12s var(--ease);
  }
  .send {
    background: var(--accent);
    color: white;
    font-size: 1.1rem;
  }
  .quick {
    background: none;
    font-size: 1.6rem;
  }
  .send:active,
  .quick:active {
    transform: scale(0.85);
  }
  .typing {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 20px 2px;
    font-size: 0.8rem;
    color: var(--muted);
  }
  .dots {
    display: flex;
    gap: 3px;
    padding: 8px 10px;
    border-radius: 14px;
    background: var(--surface-2);
  }
  .dots i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--muted);
    animation: bounce 1.2s infinite ease-in-out;
  }
  .dots i:nth-child(2) {
    animation-delay: 0.15s;
  }
  .dots i:nth-child(3) {
    animation-delay: 0.3s;
  }
  @keyframes bounce {
    0%,
    60%,
    100% {
      transform: translateY(0);
      opacity: 0.5;
    }
    30% {
      transform: translateY(-4px);
      opacity: 1;
    }
  }
  .toast {
    position: fixed;
    bottom: 84px;
    left: 50%;
    z-index: 55;
    padding: 10px 16px;
    border-radius: 999px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.4);
    font-size: 0.88rem;
    translate: -50% 0;
  }
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: rgb(0 0 0 / 0.55);
  }
  .dialog {
    position: fixed;
    left: 50%;
    top: 50%;
    z-index: 51;
    width: min(420px, calc(100vw - 32px));
    max-height: 80vh;
    overflow-y: auto;
    padding: 20px;
    border: 1px solid var(--border);
    border-radius: 18px;
    background: var(--surface);
    box-shadow: 0 24px 64px rgb(0 0 0 / 0.5);
    translate: -50% -50%;
  }
  .dialog h3 {
    margin: 0 0 8px;
  }
  .dialog p {
    margin: 0;
    color: var(--muted);
  }
  .dialog-buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }
  .dialog-buttons button {
    padding: 9px 16px;
    border: none;
    border-radius: 999px;
    background: var(--surface-2);
    cursor: pointer;
  }
  .dialog-buttons .danger {
    background: var(--danger);
    color: white;
  }
  .revisions {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 12px 0 0;
    padding: 0;
    list-style: none;
  }
  .revisions li {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px;
    border-radius: 10px;
    background: var(--surface-2);
    white-space: pre-wrap;
  }
  .revisions li.current {
    outline: 1px solid var(--accent-a);
  }
  .revisions small {
    color: var(--muted);
  }
  .chat-menu-button {
    margin-left: auto;
    width: 36px;
    height: 36px;
    border: none;
    border-radius: 50%;
    background: none;
    font-size: 1.3rem;
    color: var(--muted);
    cursor: pointer;
  }
  .chat-menu-button:hover {
    background: var(--surface-2);
  }
  .chat-menu {
    position: absolute;
    top: 56px;
    right: 12px;
    z-index: 20;
    display: flex;
    flex-direction: column;
    min-width: 220px;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface-2);
    box-shadow: 0 12px 32px rgb(0 0 0 / 0.45);
  }
  .chat-menu p {
    margin: 4px 10px 6px;
    font-size: 0.78rem;
    color: var(--muted);
  }
  .chat-menu button {
    padding: 8px 10px;
    border: none;
    border-radius: 8px;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .chat-menu button:hover {
    background: var(--surface-hover);
  }
  .chat-menu button.active::after {
    content: " ✓";
    color: var(--accent-b);
  }
  .scroller {
    position: relative;
  }
  .drop-hint {
    position: sticky;
    inset: 0;
    z-index: 10;
    display: grid;
    place-items: center;
    min-height: 100%;
    border: 2px dashed var(--accent-a);
    border-radius: 16px;
    margin: 8px;
    background: rgb(123 92 255 / 0.12);
    font-size: 1.1rem;
    pointer-events: none;
  }
  .attach {
    flex: none;
    width: 40px;
    height: 40px;
    border: none;
    border-radius: 50%;
    background: none;
    font-size: 1.25rem;
    cursor: pointer;
  }
  .attach:hover {
    background: var(--surface-2);
  }
  .pending {
    display: flex;
    gap: 8px;
    margin: 0 4px 8px;
    overflow-x: auto;
  }
  .pending-item {
    position: relative;
    flex: none;
    width: 72px;
    height: 72px;
    border-radius: 12px;
    overflow: hidden;
    background: var(--surface-2);
  }
  .pending-item img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .pending-file {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    padding: 4px;
    font-size: 1.4rem;
  }
  .pending-file small {
    width: 100%;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 0.65rem;
    text-align: center;
  }
  .pending-item button {
    position: absolute;
    top: 3px;
    right: 3px;
    width: 20px;
    height: 20px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: rgb(0 0 0 / 0.6);
    color: white;
    font-size: 0.85rem;
    line-height: 1;
    cursor: pointer;
  }
  .send-status {
    margin: 0 8px 6px;
    font-size: 0.8rem;
    color: var(--muted);
  }
</style>
