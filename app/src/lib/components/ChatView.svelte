<script lang="ts">
  import { tick, untrack } from "svelte";
  import { fade, fly } from "svelte/transition";
  import {
    deleteMessage,
    editHistory,
    editMessage,
    markRead,
    saveMedia,
    sendMessage,
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
  let replyTo = $state<MessageEntry | null>(null);
  /** The message being edited: the composer then replaces its text instead of sending. */
  let editing = $state<MessageEntry | null>(null);
  let sendError = $state<string | null>(null);
  let toast = $state<string | null>(null);
  let viewer = $state<{ media: string; kind: "image" | "video"; caption: string | null } | null>(null);
  let confirmDelete = $state<MessageEntry | null>(null);
  let history = $state<{ revisions: Revision[] | null; error: string | null } | null>(null);
  let scroller: HTMLDivElement;
  let input: HTMLTextAreaElement;

  $effect(() => {
    const id = room.id;
    draft = "";
    replyTo = null;
    editing = null;
    sendError = null;
    untrack(() => chat.open(id));
    return () => stopTyping(id);
  });

  // Typing notices: on while there's fresh input, off after a pause, on send, or on leaving.
  let typingSentAt = 0;
  let typingIdle: ReturnType<typeof setTimeout> | undefined;
  function onInput() {
    autosize();
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

  type Row =
    | { kind: "message"; key: string; msg: MessageEntry; first: boolean; last: boolean }
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
          const joins =
            prev?.kind === "message" &&
            prev.msg.sender === e.sender &&
            e.timestamp - prev.msg.timestamp < GROUP_GAP_MS;
          if (joins) prev.last = false;
          out.push({ kind: "message", key: e.key, msg: e, first: !joins, last: true });
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
    if (!body) return;
    if (editing) return saveEdit(editing, body);
    stopTyping(room.id);
    const reply = replyTo?.item_id ?? null;
    draft = "";
    replyTo = null;
    sendError = null;
    await tick();
    autosize();
    scroller.scrollTo({ top: 0, behavior: "smooth" });
    try {
      await sendMessage(room.id, body, reply);
    } catch (e) {
      sendError = String(e);
      draft ||= body;
    }
  }

  async function saveEdit(msg: MessageEntry, body: string) {
    editing = null;
    draft = "";
    await tick();
    autosize();
    if (body === editableText(msg)) return;
    try {
      await editMessage(room.id, msg.item_id, body);
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

  function openMedia(msg: MessageEntry) {
    const c = msg.content;
    if (c.kind === "image" || c.kind === "video") {
      viewer = { media: c.media, kind: c.kind, caption: c.caption };
    }
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

  function onKeydown(event: KeyboardEvent) {
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
</header>

<div class="scroller" bind:this={scroller} onscroll={onScroll}>
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
            onOpenMedia={() => openMedia(row.msg)}
            onSaveFile={() => saveFile(row.msg)}
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
  {#if sendError}<p class="send-error">{sendError}</p>{/if}
  <div class="bar">
    <textarea
      bind:this={input}
      bind:value={draft}
      oninput={onInput}
      onblur={() => stopTyping(room.id)}
      onkeydown={onKeydown}
      rows="1"
      placeholder="Message"
    ></textarea>
    {#if draft.trim() || editing}
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
  <MediaViewer {...viewer} onClose={() => (viewer = null)} />
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
</style>
