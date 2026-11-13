<script lang="ts">
  import { prefetch } from "$lib/cache";
  import { tr, trn } from "$lib/i18n/index.svelte";
  import CallDock from "./CallDock.svelte";
  import FileSheet from "./FileSheet.svelte";
  import MessageActions, { type Actions } from "./MessageActions.svelte";
  import ForwardPage from "./ForwardPage.svelte";
  import ChartBar from "@lucide/svelte/icons/chart-bar";
  import PollCreator from "./PollCreator.svelte";
  import Pin from "@lucide/svelte/icons/pin";
  import PinOff from "@lucide/svelte/icons/pin-off";
  import List from "@lucide/svelte/icons/list";
  import Page from "./Page.svelte";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import { chatList } from "$lib/stores/chatList.svelte";
  import { nav } from "$lib/stores/nav.svelte";
  import Lock from "@lucide/svelte/icons/lock";
  import Phone from "@lucide/svelte/icons/phone";
  import Video from "@lucide/svelte/icons/video";
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import SendHorizontal from "@lucide/svelte/icons/send-horizontal";
  import Image from "@lucide/svelte/icons/image";
  import Paperclip from "@lucide/svelte/icons/paperclip";
  import Check from "@lucide/svelte/icons/check";
  import Mic from "@lucide/svelte/icons/mic";
  import Bold from "@lucide/svelte/icons/bold";
  import Camera from "@lucide/svelte/icons/camera";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import CirclePlus from "@lucide/svelte/icons/circle-plus";
  import Code from "@lucide/svelte/icons/code";
  import Italic from "@lucide/svelte/icons/italic";
  import LinkIcon from "@lucide/svelte/icons/link";
  import ListOrdered from "@lucide/svelte/icons/list-ordered";
  import MapPin from "@lucide/svelte/icons/map-pin";
  import SmilePlus from "@lucide/svelte/icons/smile-plus";
  import SquareCode from "@lucide/svelte/icons/square-code";
  import Strikethrough from "@lucide/svelte/icons/strikethrough";
  import TextQuote from "@lucide/svelte/icons/text-quote";
  import Type from "@lucide/svelte/icons/type";
  import X from "@lucide/svelte/icons/x";
  import Reply from "@lucide/svelte/icons/reply";
  import CopyIcon from "@lucide/svelte/icons/copy";
  import ForwardIcon from "@lucide/svelte/icons/forward";
  import Download from "@lucide/svelte/icons/download";
  import { SvelteSet } from "svelte/reactivity";
  import PhotoEditor from "./PhotoEditor.svelte";
  import CameraView from "./CameraView.svelte";
  import { viewOnce } from "$lib/stores/viewOnce.svelte";
  import Timer from "@lucide/svelte/icons/timer";
  import CountdownStrip from "./CountdownStrip.svelte";
  import { emotes } from "$lib/stores/emotes.svelte";
  import Pencil from "@lucide/svelte/icons/pencil";
  import EmojiPicker from "./EmojiPicker.svelte";
  import { geoUri, liveLocation, untilTime } from "$lib/stores/liveLocation.svelte";
  import { tick, untrack } from "svelte";
  import { fade, fly } from "svelte/transition";
  import {
    chatMediaHd,
    roomDetails,
    deleteMessage,
    discardPicked,
    saveBlob,
    sendSticker,
    type Emote,
    editHistory,
    isAndroid,
    touchKeyboard,
    setSecureScreen,
    takeKeyboardContent,
    editMessage,
    markRead,
    hasTitlebar,
    pickFiles,
    capture,
    sendLocation,
    saveMedia,
    sendFile,
    pinMessage,
    forwardMessages,
    fileAction,
    sendMessage,
    sendPicked,
    sendVoice,
    setChatMediaHd,
    setTyping,
    toggleReaction,
    type MessageEntry,
    type Picked,
    type Revision,
    type RoomSummary,
    joinRoom,
  } from "$lib/backend";
  import { clockTime, dayLabel, shortTime } from "$lib/format";
  import { chat } from "$lib/stores/chat.svelte";
  import { layouts } from "$lib/stores/layout.svelte";
  import { friendlyError } from "$lib/errors";
  import { drafts } from "$lib/stores/drafts.svelte";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { share } from "$lib/stores/share.svelte";
  import { playEffect } from "$lib/effects";
  import ChatSearchPage from "./ChatSearchPage.svelte";
  import SharedMediaPage from "./SharedMediaPage.svelte";
  import { encryption } from "$lib/stores/encryption.svelte";
  import Avatar from "./Avatar.svelte";
  import { saved } from "$lib/stores/saved.svelte";
  import MediaViewer from "./MediaViewer.svelte";
  import MentionSuggestions from "./MentionSuggestions.svelte";
  import { MentionAutocomplete } from "$lib/mentions.svelte";
  import { pickedMediaUrl } from "$lib/media";
  import { probeVideo } from "$lib/videoMeta";
  import { VoiceRecorder } from "$lib/recorder.svelte";
  import { presence } from "$lib/stores/presence.svelte";
  import { profiles } from "$lib/stores/profiles.svelte";
  import { call } from "$lib/stores/call.svelte";
  import { DEFAULT_QUICK_REACTION } from "$lib/themes";
  import ChatSettingsPage from "./ChatSettingsPage.svelte";
  import CustomizePage from "./CustomizePage.svelte";
  import TimelineStrip from "./TimelineStrip.svelte";
  import { saveToGallery } from "$lib/save";
  import { onBack as closeOnBack } from "$lib/back.svelte";
  import MessageBubble from "./MessageBubble.svelte";
  import ChatView from "./ChatView.svelte";
  import { ThreadTimeline } from "$lib/stores/thread.svelte";

  let {
    room,
    onBack,
    thread = null,
  }: {
    room: RoomSummary;
    onBack: () => void;
    /** Show the thread started on this message instead of the chat. */
    thread?: string | null;
  } = $props();

  // The chat's timeline, or the thread's. Commands that take a room ID act on the thread
  // when given its root (`target`); room-wide ones (typing, pins, settings) keep `room.id`.
  const tl = untrack(() => (thread ? new ThreadTimeline(room.id, thread) : chat));
  const target = $derived(thread ?? room.id);
  $effect(() => () => {
    if (tl instanceof ThreadTimeline) tl.close();
  });

  /** Upgraded chat: join its new version (if needed) and open it. */
  let movingOn = $state(false);
  let moveError = $state<string | null>(null);
  async function goToSuccessor() {
    const next = room.successor;
    if (!next) return;
    moveError = null;
    if (!chatList.rooms.some((r) => r.id === next && !r.is_invite)) {
      movingOn = true;
      // Servers that may know the new room: its own and this one's (room IDs before room
      // version 12 name the server that made them).
      const server = (id: string) => id.split(":").slice(1).join(":");
      const via = [...new Set([server(next), server(room.id)].filter(Boolean))];
      try {
        await joinRoom(next, via);
      } catch (e) {
        moveError = String(e);
        return;
      } finally {
        movingOn = false;
      }
    }
    nav.openChat(next);
  }

  /** The thread open over this chat. */
  let threadRoot = $state<string | null>(null);
  $effect(() => {
    if (threadRoot) return closeOnBack(() => (threadRoot = null));
  });

  /** Messages from the same sender closer than this are drawn as one run. */
  const GROUP_GAP_MS = 5 * 60_000;
  /** Until per-chat quick reactions (com.alwy.chat.room.quick_reaction) land. */
  // ── Voice messages ───────────────────────────────────────────────────────
  const recorder = new VoiceRecorder();
  $effect(() => {
    if (recorder.recording) return closeOnBack(() => recorder.cancel());
  });

  async function startVoice() {
    sendError = null;
    try {
      await recorder.start();
    } catch (e) {
      sendError = e instanceof DOMException && e.name === "NotAllowedError"
        ? tr("AlwyChat isn't allowed to use the microphone")
        : tr("Couldn't start recording: {e}", { e: e });
    }
  }

  async function finishVoice() {
    const recording = await recorder.stop();
    if (!recording) return;
    if (recording.durationMs < 800) {
      sendError = tr("Too short: tap the microphone, speak, then send");
      return;
    }
    sendingFiles = true;
    try {
      await sendVoice(target, recording);
    } catch (e) {
      sendError = String(e);
    } finally {
      sendingFiles = false;
    }
  }

  const recordClock = $derived.by(() => {
    const s = Math.floor(recorder.elapsedMs / 1000);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  });

  // 1:1 chats: "Online" / "Last seen 5 min ago" under the name, refreshed while open.
  $effect(() => {
    const user = room.dm_user;
    if (!user) return;
    void presence.refresh([user]);
    void profiles.refresh([user]);
    const timer = setInterval(() => (presence.refresh([user]), profiles.refresh([user])), 30_000);
    return () => clearInterval(timer);
  });
  // "📚 Studying · Online · 22:14 their time": status and local time from their profile.
  const status = $derived(
    room.is_dm
      ? [profiles.statusLabel(room.dm_user), presence.label(room.dm_user), profiles.localTime(room.dm_user)]
          .filter(Boolean)
          .join(" · ") || null
      : null,
  );

  // A 1:1 chat is titled with the nickname given to the other person, if any.
  // A name given to the chat (shared, Messenger's "chat name") wins; else the person's nickname.
  const title = $derived(room.has_name ? room.name : (room.dm_user && chat.style?.nicknames[room.dm_user]) || room.name);
  // The chat's own quick reaction (shared by its members), 🔥 until someone picks one.
  const QUICK_REACTION = $derived(chat.style?.quick_reaction ?? DEFAULT_QUICK_REACTION);

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
  /** Attachments waiting in the composer: a browser File (desktop) or a picked file (Android). */
  type Pending = { name: string; mime: string; url: string | null; file?: File; picked?: Picked; viewOnce?: boolean };
  const canViewOnce = (p: Pending) => (p.mime.startsWith("image/") && p.mime !== "image/gif") || p.mime.startsWith("video/");
  let pending = $state<Pending[]>([]);
  let dragging = $state(false);
  let sendingFiles = $state(false);
  // The composer is swapped out while recording a voice message, so this can change.
  let fileInput = $state<HTMLInputElement>() as HTMLInputElement;
  let chatMenuOpen = $state(false);
  let customizeOpen = $state(false);
  let historyOpen = $state(false);
  $effect(() => {
    if (historyOpen) return closeOnBack(() => (historyOpen = false));
  });
  $effect(() => {
    if (customizeOpen) return closeOnBack(() => (customizeOpen = false));
  });
  /** This chat's HD choice: null = app default. */
  let chatHd = $state<boolean | null>(null);
  /** Messages to delete once confirmed (one, or a selection). */
  let confirmDelete = $state<MessageEntry[] | null>(null);
  let history = $state<{ revisions: Revision[] | null; error: string | null } | null>(null);

  // Android back closes these before the chat itself.
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
  // Discord layout when the chat is wide on a landscape screen (computer): bubbles left and
  // right would be far apart.
  let chatWidth = $state(0);
  let landscape = $state(true);
  const layout = $derived(layouts.of(room.id));
  const wide = $derived(layout === "left" || (layout === "auto" && chatWidth >= 760 && landscape));
  $effect(() => {
    const observer = new ResizeObserver(([entry]) => {
      chatWidth = entry.contentRect.width;
      landscape = innerWidth > innerHeight;
    });
    observer.observe(scroller);
    return () => observer.disconnect();
  });
  // The composer is swapped out while recording a voice message, so this can change.
  let input = $state<HTMLTextAreaElement>() as HTMLTextAreaElement;

  /** The draft's reply target, until that message is loaded. */
  let draftReply: string | null = null;

  // Only a different chat resets the view: the chat list sends a fresh copy of the same chat
  // whenever it changes (unread count, latest message…), which used to close chat settings
  // and reset the composer right after opening a chat.
  const roomId = $derived(room.id);
  $effect(() => {
    const id = roomId;
    threadRoot = null;
    if (thread) {
      untrack(() => mention.reset(id, !room.is_dm));
      return;
    }
    const saved = untrack(() => drafts.get(id));
    draft = saved?.text ?? "";
    draftReply = saved?.reply_to ?? null;
    tick().then(autosize);
    untrack(() => mention.reset(id, !room.is_dm));
    replyTo = null;
    editing = null;
    sendError = null;
    untrack(() => {
      recorder.cancel();
      clearPending();
      chatMenuOpen = false;
      chat.open(id);
    });
    chatHd = null;
    chatMediaHd(id).then((hd) => room.id === id && (chatHd = hd)).catch(() => {});
    // Chat settings read the power levels from the server: fetch them while the chat is read,
    // so the settings page opens filled in.
    const warm = thread ? undefined : setTimeout(() => prefetch(`details:${id}`, () => roomDetails(id)), 1200);
    return () => {
      clearTimeout(warm);
      stopTyping(id);
      drafts.flush(id);
    };
  });

  // Message effects play once, for messages under a couple of minutes old (Element does the same).
  const EFFECT_MAX_AGE_MS = 120_000;
  const effectsPlayed = new Set<string>();
  $effect(() => {
    if (thread || !tl.loaded || !prefs.effects) return;
    for (const e of tl.entries.slice(-8)) {
      if (e.type !== "message" || effectsPlayed.has(e.key)) continue;
      const c = e.content;
      if ((c.kind !== "text" && c.kind !== "emote") || !c.effect) continue;
      effectsPlayed.add(e.key);
      if (Date.now() - e.timestamp < EFFECT_MAX_AGE_MS) playEffect(c.effect);
    }
  });

  // The composer is the chat's draft (not while editing a sent message).
  $effect(() => {
    const text = draft;
    const reply = replyTo?.item_id ?? draftReply;
    if (editing || thread) return;
    untrack(() => drafts.set(room.id, text, reply));
  });
  $effect(() => {
    if (!draftReply || !tl.loaded) return;
    const id = draftReply;
    const entry = tl.entries.find((e) => e.type === "message" && e.item_id === id);
    if (entry?.type === "message") replyTo = entry;
    draftReply = null;
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
    | { kind: "notice"; key: string; text: string; action?: "theme" }
    | { kind: "unread"; key: string }
    | { kind: "start"; key: string };

  // Disappearing messages: an expired one leaves the view on time, whether or not its deletion
  // reached the server yet (only messages sent since the timer was turned on).
  let clock = $state(Date.now());
  $effect(() => {
    if (!chat.style?.disappearing) return;
    const t = setInterval(() => (clock = Date.now()), 15_000);
    return () => clearInterval(t);
  });
  const expired = (msg: MessageEntry) => {
    const d = chat.style?.disappearing;
    return !!d && msg.timestamp >= d.since && msg.timestamp + d.timer_ms <= clock;
  };

  /** "5 min", "1 h", "1 day"… */
  function disappearLabel(ms: number) {
    if (ms < 3_600_000) return tr("{n} min", { n: Math.round(ms / 60_000) });
    if (ms < 86_400_000) return tr("{n} h", { n: Math.round(ms / 3_600_000) });
    return trn(Math.round(ms / 86_400_000), "{n} day", "{n} days");
  }

  const rows = $derived.by(() => {
    const out: Row[] = [];
    for (const e of tl.entries) {
      switch (e.type) {
        case "message": {
          if (expired(e)) break;
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
          out.push({ kind: "message", key: e.key, msg: withNickname(e), first: !joins, last: true, album: null });
          break;
        }
        case "date_divider":
          out.push({ kind: "day", key: e.key, label: dayLabel(e.timestamp) });
          break;
        case "notice": {
          // "Alex changed the chat theme" twice in a row (trying colours): show it once.
          const prev = out.at(-1);
          if (prev?.kind === "notice" && prev.text === e.text) prev.key = e.key;
          else out.push({ kind: "notice", key: e.key, text: e.text, action: e.action });
          break;
        }
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
    // Rows that didn't change keep their object, so only what changed is drawn again (a new
    // message used to update every bubble of the chat).
    const kept = new Map<string, Row>();
    for (let i = 0; i < out.length; i++) {
      const old = lastRows.get(out[i].key);
      if (old && sameRow(old, out[i])) out[i] = old;
      kept.set(out[i].key, out[i]);
    }
    lastRows = kept;
    return out;
  });
  let lastRows = new Map<string, Row>();
  function sameRow(a: Row, b: Row) {
    if (a.kind === "message" && b.kind === "message")
      return (
        a.msg === b.msg &&
        a.first === b.first &&
        a.last === b.last &&
        a.album?.length === b.album?.length &&
        (a.album ?? []).every((m, i) => m === b.album![i])
      );
    if (a.kind === "day" && b.kind === "day") return a.label === b.label;
    if (a.kind === "notice" && b.kind === "notice") return a.text === b.text && a.action === b.action;
    return a.kind === b.kind;
  }

  // Only the latest rows are drawn; scrolling up draws more of what's loaded, then loads
  // older messages. A long chat opens and updates as fast as a short one.
  const WINDOW = 120;
  /** Rows in the first frame of a chat: about a screenful, so it shows at once; the rest of
   *  the window follows two frames later. */
  const FIRST_PAINT = 24;
  let limit = $state(FIRST_PAINT);
  let widen = $state(true);
  const shown = $derived(rows.length > limit ? rows.slice(-limit) : rows);
  $effect(() => {
    void room.id;
    void tl.focused;
    atBottom = true;
    limit = FIRST_PAINT;
    widen = true;
  });
  $effect(() => {
    if (!widen || !rows.length) return;
    let frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(() => {
        widen = false;
        limit = Math.max(limit, WINDOW);
      });
    });
    return () => cancelAnimationFrame(frame);
  });

  /** Show the nickname this chat gave the sender, if any. */
  /** The same renamed copy each time (a new object would redraw the bubble). */
  const nicknamed = new WeakMap<MessageEntry, { nick: string; msg: MessageEntry }>();
  function withNickname(e: MessageEntry): MessageEntry {
    const nick = chat.style?.nicknames[e.sender];
    if (!nick || e.is_own) return e;
    const known = nicknamed.get(e);
    if (known?.nick === nick) return known.msg;
    const msg = { ...e, sender_name: nick };
    nicknamed.set(e, { nick, msg });
    return msg;
  }

  function joinsAlbum(a: MessageEntry, b: MessageEntry) {
    const ca = a.content;
    const cb = b.content;
    if (ca.kind !== "image" || cb.kind !== "image" || a.sender !== b.sender || b.reply_to) return false;
    // A view-once photo stays on its own (an album would show it).
    if (ca.view_once || cb.view_once) return false;
    if (ca.gallery && cb.gallery) return ca.gallery.id === cb.gallery.id;
    return !ca.caption && !cb.caption && b.timestamp - a.timestamp < BURST_GAP_MS;
  }

  // The list is laid out bottom-up (column-reverse): scrollTop 0 is the newest message,
  // and the browser keeps that anchor while older messages are prepended.
  /** Scrolled up far enough to offer the way back down; the newest entry seen then. */
  let scrolledUp = $state(false);
  let lastAtBottom = $state<string | null>(null);
  const lastKey = $derived(tl.entries.at(-1)?.key ?? null);
  const newBelow = $derived(scrolledUp && lastAtBottom !== null && lastKey !== lastAtBottom);
  function toBottom() {
    scroller.scrollTo({ top: 0, behavior: "smooth" });
    // Some webviews skip smooth scrolling in a reversed list: land there anyway.
    setTimeout(() => {
      if (Math.abs(scroller.scrollTop) > 50) scroller.scrollTop = 0;
      onScroll();
    }, 700);
  }

  // Staying at the newest message: a bottom-up list should keep its place at the bottom when a
  // message is added, but not every webview does (WebKitGTK on Linux leaves the view where it
  // was, so a message we just sent sat below the fold). So: if we were at the bottom, or we just
  // sent something, go back to the bottom whenever the timeline changes.
  let atBottom = true;
  let followUntil = 0;
  /** We just sent something: show it when it lands, wherever we were. */
  function followNewest() {
    followUntil = Date.now() + 4000;
  }
  $effect(() => {
    void tl.entries;
    void shown.length;
    // Not while looking at an older part of the chat (search result, timeline strip).
    if (!scroller || tl.focused) return;
    if ((atBottom || Date.now() < followUntil) && Math.abs(scroller.scrollTop) > 1) scroller.scrollTop = 0;
  });

  function onScroll() {
    atBottom = Math.abs(scroller.scrollTop) < 40;
    // Back down at the newest: what we scrolled past is read now.
    if (Math.abs(scroller.scrollTop) < 40 && (room.unread_messages > 0 || room.marked_unread)) readSoon(300);
    const up = Math.abs(scroller.scrollTop) > 600;
    if (up && !scrolledUp) lastAtBottom = lastKey;
    scrolledUp = up;
    const fromTop = scroller.scrollHeight + scroller.scrollTop - scroller.clientHeight;
    // Near the top of what's drawn: more of what's loaded first, then older messages.
    if (limit < rows.length) {
      if (fromTop < 1500) limit += WINDOW;
    } else if (fromTop < 400) tl.older();
    // Back at the newest: let go of the rows far above.
    if (limit > WINDOW && Math.abs(scroller.scrollTop) < 40) limit = WINDOW;
    // Opened on an older message: newer ones load near the bottom.
    if (tl.focused && Math.abs(scroller.scrollTop) < 400) tl.newer();
  }

  // Read receipts: whenever we may have seen something new — content arrived, the chat came
  // on screen (back from the background, a notification tapped while Android was still bringing
  // the app up), we scrolled down to the newest, or the list still counts it unread — and we're
  // at the bottom and looking. Checking only when content arrived left chats unread when the
  // app wasn't "visible" yet at that moment.
  let readTimer: ReturnType<typeof setTimeout> | undefined;
  function readSoon(delay = 600) {
    clearTimeout(readTimer);
    readTimer = setTimeout(readNow, delay);
  }
  function readNow() {
    // A thread with no replies yet has nothing to read (the root belongs to the chat).
    const last = tl.entries.findLast((e) => e.type === "message");
    if (thread && (!last || (last.type === "message" && last.item_id === thread))) return;
    // Android's WebView doesn't always report focus (back from a notification…): there,
    // being on screen is what counts.
    const looking = isAndroid ? document.visibilityState === "visible" : document.hasFocus();
    if (looking && scroller && Math.abs(scroller.scrollTop) < 40) {
      markRead(target).catch(() => {});
    }
  }
  $effect(() => {
    tl.entries.length;
    // Still counted unread in the list (e.g. its receipt went to an older message).
    room.unread_messages;
    room.marked_unread;
    readSoon();
    return () => clearTimeout(readTimer);
  });
  $effect(() => {
    const again = () => document.visibilityState === "visible" && readSoon(300);
    document.addEventListener("visibilitychange", again);
    window.addEventListener("focus", again);
    return () => {
      document.removeEventListener("visibilitychange", again);
      window.removeEventListener("focus", again);
    };
  });

  /** Buttons next to the field keep the focus in it (on a phone, losing it closes the keyboard). */
  /** Android shrinks the page while its keyboard is open (MainActivity's insets). */
  const keyboardOpen = () => !isAndroid || window.innerHeight < screen.height * 0.75;
  // Send and the quick reaction keep the keyboard as it is: open stays open (several messages
  // in a row), closed stays closed (Android's back gesture hides it but leaves the field
  // focused, and it would pop up again).
  function keepFocus(event: MouseEvent) {
    if (document.activeElement !== input) return;
    if (keyboardOpen()) event.preventDefault();
    else input.blur();
  }

  async function send(event?: SubmitEvent) {
    event?.preventDefault();
    const body = draft.trim();
    if (pending.length && !editing) return sendAttachments(body);
    if (editing) return saveEdit(editing, body);
    if (!body) return;
    if (tl.focused) await tl.latest();
    stopTyping(room.id);
    const reply = replyTo?.item_id ?? null;
    const mentions = mention.mentionsFor(body);
    mention.clear();
    draft = "";
    replyTo = null;
    sendError = null;
    await tick();
    autosize();
    scroller.scrollTo({ top: 0, behavior: "smooth" });
    followNewest();
    try {
      await sendMessage(target, body, reply, mentions, emotes.usedIn(body));
    } catch (e) {
      sendError = String(e);
      draft ||= body;
    }
  }

  // ── Attachments ──────────────────────────────────────────────────────────
  function addFiles(files: Iterable<File>) {
    for (const file of files) {
      const url = file.type.startsWith("image/") ? URL.createObjectURL(file) : null;
      pending.push({ name: file.name, mime: file.type, url, file });
    }
    input?.focus();
  }

  // Shared from another app into this chat: it waits in the composer for a caption and Send.
  $effect(() => {
    if (thread || share.ready?.roomId !== room.id) return;
    const shared = untrack(() => share.take(room.id));
    if (!shared) return;
    for (const picked of shared.files) {
      const url = picked.mime.startsWith("image/") ? pickedMediaUrl(picked.path) : null;
      pending.push({ name: picked.name, mime: picked.mime, url, picked });
    }
    if (shared.text) draft = draft.trim() ? `${draft}\n${shared.text}` : shared.text;
  });

  // Android: an image, GIF or sticker inserted from the keyboard (Gboard's clipboard or GIF
  // panel) waits in this composer like a picked photo. Only the chat being typed in takes it.
  $effect(() => {
    if (!isAndroid) return;
    const onContent = async () => {
      if (document.activeElement !== input) return;
      const content = await takeKeyboardContent().catch(() => null);
      for (const picked of content?.files ?? []) {
        const url = picked.mime.startsWith("image/") ? pickedMediaUrl(picked.path) : null;
        pending.push({ name: picked.name, mime: picked.mime, url, picked });
      }
    };
    window.addEventListener("alwychat-keyboard-content", onContent);
    return () => window.removeEventListener("alwychat-keyboard-content", onContent);
  });

  /** Android: the system photo picker or document picker, several items at once. */
  async function pick(kind: "media" | "files") {
    try {
      for (const picked of await pickFiles(kind)) {
        const url = picked.mime.startsWith("image/") ? pickedMediaUrl(picked.path) : null;
        pending.push({ name: picked.name, mime: picked.mime, url, picked });
      }
    } catch (e) {
      sendError = String(e);
    }
  }

  // ── Editing a photo before sending (optional) ──
  /** The waiting photo being edited (its place in `pending`). */
  let editingPhoto = $state<number | null>(null);
  const editable = (p: Pending) => !!p.url && p.mime.startsWith("image/") && p.mime !== "image/gif";

  async function photoEdited(i: number, jpeg: Blob) {
    editingPhoto = null;
    const old = pending[i];
    if (!old) return;
    const name = old.name.replace(/\.[^.]+$/, "") + ".jpg";
    try {
      let next: Pending;
      if (isAndroid) {
        // The Android webview can't hand raw bytes over: saved as a waiting file instead.
        const picked = await saveBlob(name, jpeg);
        next = { name, mime: "image/jpeg", url: pickedMediaUrl(picked.path), picked };
      } else {
        const file = new File([jpeg], name, { type: "image/jpeg" });
        next = { name, mime: "image/jpeg", url: URL.createObjectURL(file), file };
      }
      pending[i] = next;
      if (old.file && old.url) URL.revokeObjectURL(old.url);
      if (old.picked) void discardPicked(old.picked);
    } catch (e) {
      sendError = String(e);
    }
  }

  function removePending(i: number) {
    const [p] = pending.splice(i, 1);
    if (p?.file && p.url) URL.revokeObjectURL(p.url);
    if (p?.picked) void discardPicked(p.picked);
  }

  function clearPending() {
    for (const p of pending) if (p.file && p.url) URL.revokeObjectURL(p.url);
    pending = [];
  }

  /** Photos go out as one album; a single file takes the text as caption, else it's sent first. */
  async function sendAttachments(text: string) {
    const files = [...pending];
    clearPending();
    draft = "";
    sendError = null;
    stopTyping(room.id);
    await tick();
    autosize();
    scroller.scrollTo({ top: 0, behavior: "smooth" });
    followNewest();
    sendingFiles = true;
    try {
      if (text && files.length > 1) await sendMessage(target, text, null);
      const isPhoto = (f: Pending) => f.mime.startsWith("image/") && f.mime !== "image/gif" && !f.viewOnce;
      const photos = files.filter(isPhoto).length;
      const galleryId = photos > 1 ? crypto.randomUUID() : null;
      let index = 0;
      // One at a time: the send queue keeps them in order and the album stays together.
      for (const f of files) {
        const gallery = galleryId && isPhoto(f) ? { id: galleryId, index: index++, count: photos } : null;
        // Videos: their shape, length and a preview frame, so every app shows them properly.
        let video = null;
        if (f.mime.startsWith("video/")) {
          const src = f.picked ? pickedMediaUrl(f.picked.path) : f.file ? URL.createObjectURL(f.file) : null;
          video = src ? await probeVideo(src) : null;
          if (src?.startsWith("blob:")) URL.revokeObjectURL(src);
        }
        const meta = { caption: files.length === 1 ? text : null, gallery, viewOnce: !!f.viewOnce, video };
        if (f.picked) await sendPicked(target, f.picked, meta);
        else if (f.file) await sendFile(target, f.file, meta);
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
    const previous = chatHd;
    chatHd = hd;
    try {
      await setChatMediaHd(room.id, hd);
    } catch (e) {
      chatHd = previous;
      flash(tr("Couldn't save: {e}", { e: e }));
    }
  }

  async function saveEdit(msg: MessageEntry, body: string) {
    const mentions = mention.mentionsFor(body);
    mention.clear();
    editing = null;
    draft = "";
    await tick();
    autosize();
    if (body === editableText(msg)) return;
    // Text can't be emptied (that's deleting); a photo's caption can.
    if (!body && !("caption" in msg.content)) return;
    try {
      await editMessage(target, msg.item_id, body, mentions);
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
      // The message being edited stays in sight (the rest of the chat dims around it).
      document.querySelector(`[data-event="${CSS.escape(msg.item_id)}"]`)?.scrollIntoView({ block: "nearest", behavior: "smooth" });
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
    if ("caption" in c) return c.caption ?? "";
    return "body" in c ? c.body : "";
  }

  async function doDelete(msgs: MessageEntry[]) {
    confirmDelete = null;
    for (const msg of msgs) {
      try {
        await deleteMessage(target, msg.item_id);
      } catch (e) {
        sendError = String(e);
      }
    }
  }

  // ── Selecting several messages (Copy, Forward, Save, Delete together) ──
  /** Selected rows (their first message's ID); null = not selecting. */
  let selected = $state<SvelteSet<string> | null>(null);
  $effect(() => {
    if (selected) return closeOnBack(() => (selected = null));
  });
  /** The selected messages in timeline order (an album counts with all its photos). */
  const selection = $derived.by((): MessageEntry[] => {
    if (!selected) return [];
    return rows.flatMap((r) => (r.kind === "message" && selected!.has(r.msg.item_id) ? (r.album ?? [r.msg]) : []));
  });
  function startSelect(msg: MessageEntry) {
    selected = new SvelteSet([msg.item_id]);
  }
  function toggleSelect(msg: MessageEntry) {
    if (!selected) return;
    if (selected.has(msg.item_id)) selected.delete(msg.item_id);
    else selected.add(msg.item_id);
    if (!selected.size) selected = null;
  }
  const selectable = (m: MessageEntry) => m.item_id.startsWith("$") && !["redacted", "unable_to_decrypt"].includes(m.content.kind);
  const selectionActions = $derived.by(() => {
    const items = selection;
    const texts = items.map(messageText).filter((t): t is string => !!t);
    const forwardable =
      items.length > 0 &&
      items.every((m) => m.item_id.startsWith("$") && !["redacted", "unable_to_decrypt", "unsupported", "poll", "sticker"].includes(m.content.kind));
    const visual = items.filter((m) => m.content.kind === "image" || m.content.kind === "video");
    return {
      copy: texts.length ? () => copyText(texts.join("\n\n")) : undefined,
      forward: forwardable ? () => (forwardIds = items.map((m) => m.item_id)) : undefined,
      save: visual.length ? () => saveVisualMedia(visual) : undefined,
      delete: items.length && items.every((m) => m.can_delete) ? () => (confirmDelete = items) : undefined,
    };
  });
  /** Run a selection action, then leave selecting (Forward closes it once sent). */
  function selectionDo(action: (() => void) | undefined) {
    if (!action) return;
    action();
    selected = null;
  }

  async function showHistory(msg: MessageEntry) {
    history = { revisions: null, error: null };
    try {
      history = { revisions: await editHistory(target, msg.item_id), error: null };
    } catch (e) {
      history = { revisions: [], error: String(e) };
    }
  }

  /** The view-once message on screen (Android: no screenshots meanwhile). */
  let onceOpen = $state(false);
  $effect(() => {
    if (!isAndroid || !onceOpen) return;
    void setSecureScreen(true).catch(() => {});
    return () => void setSecureScreen(!!prefs.settings?.hide_in_switcher).catch(() => {});
  });

  function openMedia(msgs: MessageEntry[], index: number) {
    const once = msgs.length === 1 ? msgs[0] : null;
    const oc = once?.content;
    if (once && (oc?.kind === "image" || oc?.kind === "video") && oc.view_once) {
      // Others' only, once: shown now, then never again (the sender's app deletes it).
      if (once.is_own || viewOnce.opened.has(once.item_id)) return;
      viewOnce.open(target, once.item_id);
      onceOpen = true;
      viewer = { items: [{ media: oc.media, kind: oc.kind, caption: oc.caption }], index: 0 };
      return;
    }
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
    flash(media.length > 1 ? trn(media.length, "Saving {n} item…", "Saving {n} items…") : tr("Saving…"));
    flash(await saveToGallery(media));
  }

  async function saveFile(msg: MessageEntry) {
    const c = msg.content;
    if (c.kind !== "file") return;
    try {
      const path = await saveMedia(c.media, false);
      flash(tr("Saved to {value}", { value: path.replace(/^\/home\/[^/]+/, "~") }));
    } catch (e) {
      flash(tr("Couldn't save: {e}", { e: e }));
    }
  }

  /** The file whose Open / Save sheet is shown. */
  let fileFor = $state<MessageEntry | null>(null);
  $effect(() => {
    if (fileFor) return closeOnBack(() => (fileFor = null));
  });

  async function onFileAction(msg: MessageEntry, action: "open" | "save_as" | "downloads") {
    const c = msg.content;
    if (c.kind !== "file") return;
    if (action === "open") flash(tr("Opening…"));
    try {
      const done = await fileAction(c.media, action);
      if (done) flash(done.replace(/^Saved to \/home\/[^/]+/, "Saved to ~"));
    } catch (e) {
      flash(tr("Couldn't {value}: {e}", { value: action === "open" ? "open" : "save", e: e }));
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
    // On-screen keyboards: Enter is always a new line (the send button sends); a hardware
    // keyboard plugged into a phone can still send with Ctrl+Enter.
    const enterSends = !touchKeyboard && (prefs.settings?.enter_sends ?? true);
    if (
      event.key === "Enter" &&
      !event.isComposing &&
      (enterSends ? !event.shiftKey : event.ctrlKey || event.metaKey)
    ) {
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
    // Empty: back to one row. Measuring here could catch the field while it's briefly
    // narrow (send button leaving, left buttons coming back) with the placeholder wrapped
    // over two lines, and that height would stick (Android).
    if (!input.value) {
      input.style.height = "";
      return;
    }
    input.style.height = "auto";
    input.style.height = `${Math.min(input.scrollHeight, 160)}px`;
  }

  // The field's width changes when the buttons around it fold or unfold (typing, attachments,
  // editing) and with the window: measure again then, and once more after the buttons'
  // 140 ms slide.
  $effect(() => {
    void [typingMode, toolsShown, pending.length, editing, emojiOpen];
    if (!input) return;
    tick().then(autosize);
    const later = setTimeout(autosize, 220);
    return () => clearTimeout(later);
  });
  $effect(() => {
    const onResize = () => input && autosize();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  });

  function react(msg: MessageEntry, key: string) {
    toggleReaction(target, msg.item_id, key).catch((e) => (sendError = String(e)));
  }

  // Long-press action sheet (touch) and forwarding.
  let actionFor = $state<{ msg: MessageEntry; album: MessageEntry[] | null; link?: string | null } | null>(null);
  let forwardIds = $state<string[] | null>(null);
  $effect(() => {
    if (actionFor) return closeOnBack(() => (actionFor = null));
  });
  $effect(() => {
    if (forwardIds) return closeOnBack(() => (forwardIds = null));
  });

  function messageText(msg: MessageEntry) {
    const c = msg.content;
    if ("body" in c && c.body && c.kind !== "sticker") return c.body;
    if ("caption" in c && c.caption) return c.caption;
    return null;
  }

  async function copyText(text: string) {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // Older WebViews: the legacy copy command.
      const area = document.createElement("textarea");
      area.value = text;
      document.body.append(area);
      area.select();
      document.execCommand("copy");
      area.remove();
    }
    flash(tr("Copied"));
  }

  function actionsFor(msg: MessageEntry, album: MessageEntry[] | null, link: string | null = null): Actions {
    const items = album ?? [msg];
    const remote = items.every((m) => m.item_id.startsWith("$"));
    const c = msg.content;
    const text = messageText(msg);
    const forwardable =
      remote &&
      items.every((m) => !["redacted", "unable_to_decrypt", "unsupported", "poll", "sticker"].includes(m.content.kind));
    const visual = items.some((m) => m.content.kind === "image" || m.content.kind === "video");
    const pinned = pinnedIds.has(msg.item_id);
    return {
      react: (key) => react(msg, key),
      reply: msg.can_reply ? () => startReply(msg) : undefined,
      copy: text ? () => copyText(text) : undefined,
      copyLink: link ? () => copyText(link) : undefined,
      forward: forwardable ? () => (forwardIds = items.map((m) => m.item_id)) : undefined,
      thread: !thread && remote && msg.can_reply ? () => (threadRoot = msg.item_id) : undefined,
      pin: chat.pins.can_pin && remote ? { pinned, toggle: () => setPin(msg.item_id, !pinned) } : undefined,
      save: visual
        ? { label: items.length > 1 ? tr("Save all ({count})", { count: items.length }) : tr("Save to gallery"), run: () => saveVisualMedia(items) }
        : c.kind === "file"
          ? { label: tr("Save"), run: () => saveFile(msg) }
          : undefined,
      edit: msg.can_edit ? () => startEdit(msg) : undefined,
      history: msg.is_edited && remote ? () => showHistory(msg) : undefined,
      delete: msg.can_delete ? () => (confirmDelete = [msg]) : undefined,
      select: selectable(msg) ? () => startSelect(msg) : undefined,
    };
  }

  async function forwardTo(to: RoomSummary) {
    const ids = forwardIds;
    forwardIds = null;
    if (!ids) return;
    try {
      const sent = await forwardMessages(target, ids, to.id);
      flash(sent ? tr("Forwarded to {name}", { name: to.name }) : tr("Couldn't forward that message"));
    } catch (e) {
      flash(tr("Couldn't forward: {e}", { e: e }));
    }
  }

  function startReply(msg: MessageEntry) {
    replyTo = msg;
    input.focus();
  }

  const pinnedIds = $derived(new Set(chat.pins.messages.map((p) => p.event_id)));
  /** Which pin the banner shows; tapping it jumps there and moves on to the next (older) one. */
  let pinIndex = $state(0);
  const shownPin = $derived(chat.pins.messages[pinIndex % Math.max(1, chat.pins.messages.length)] ?? null);
  let pinsOpen = $state(false);
  let attachOpen = $state(false);
  let emojiOpen = $state(false);
  let formatting = $state(false);
  /** Messenger: while typing, the buttons left of the field fold into a chevron. */
  const typingMode = $derived(!!draft.trim());
  let toolsShown = $state(false);
  $effect(() => {
    if (emojiOpen) return closeOnBack(() => (emojiOpen = false));
  });

  function sheet(action: () => unknown) {
    attachOpen = false;
    void action();
  }

  function openFileInput(accept: string) {
    fileInput.accept = accept;
    fileInput.click();
  }

  // ── The in-app camera: its shot waits in the composer like a picked photo ──
  let cameraOpen = $state(false);

  async function addShot(blob: Blob, name: string) {
    try {
      if (isAndroid) {
        const picked = await saveBlob(name, blob);
        const url = picked.mime.startsWith("image/") ? pickedMediaUrl(picked.path) : null;
        pending.push({ name, mime: picked.mime, url, picked });
      } else {
        addFiles([new File([blob], name, { type: blob.type })]);
      }
    } catch (e) {
      sendError = String(e);
    }
  }

  /** Android: the camera app; the photo or video lands in the composer like a picked one. */
  async function takePhoto(kind: "photo" | "video") {
    try {
      for (const picked of await capture(kind)) {
        const url = picked.mime.startsWith("image/") ? pickedMediaUrl(picked.path) : null;
        pending.push({ name: picked.name, mime: picked.mime, url, picked });
      }
    } catch (e) {
      sendError = String(e);
    }
  }

  function toggleEmoji() {
    emojiOpen = !emojiOpen;
    // The phone's keyboard makes room for the panel.
    if (emojiOpen) input.blur();
    else input.focus();
  }

  // Custom emoji: the panel loads this chat's packs when it opens.
  $effect(() => {
    if (emojiOpen) void emotes.load(room.id);
  });
  /** A custom emoji goes in the text as `:code:` (sent as the image). */
  function insertCustom(e: Emote) {
    const before = draft.slice(0, input.selectionEnd ?? draft.length);
    insertEmoji(`${before && !/\s$/.test(before) ? " " : ""}:${e.shortcode}: `);
  }
  async function sticker(e: Emote) {
    emojiOpen = false;
    try {
      await sendSticker(target, e.url, e.body ?? e.shortcode, e.info);
    } catch (err) {
      sendError = String(err);
    }
  }

  function insertEmoji(emoji: string) {
    // After the selection, not over it (the formatting buttons leave text selected).
    const at = input.selectionEnd ?? draft.length;
    draft = draft.slice(0, at) + emoji + draft.slice(at);
    tick().then(() => {
      input.selectionStart = input.selectionEnd = at + emoji.length;
      autosize();
    });
  }

  /** Markdown around the selection (or at the cursor). */
  function wrap(before: string, after: string) {
    const start = input.selectionStart ?? draft.length;
    const end = input.selectionEnd ?? start;
    draft = draft.slice(0, start) + before + draft.slice(start, end) + after + draft.slice(end);
    tick().then(() => {
      input.focus();
      input.selectionStart = start + before.length;
      input.selectionEnd = end + before.length;
      autosize();
    });
  }

  /** Markdown at the start of each selected line (quote, lists). */
  function linePrefix(prefix: string) {
    const start = draft.lastIndexOf("\n", (input.selectionStart ?? draft.length) - 1) + 1;
    const end = input.selectionEnd ?? draft.length;
    const lines = draft.slice(start, end).split("\n");
    const block = lines.map((l, i) => (prefix === "1. " ? `${i + 1}. ` : prefix) + l).join("\n");
    draft = draft.slice(0, start) + block + draft.slice(end);
    tick().then(() => {
      input.focus();
      input.selectionStart = input.selectionEnd = start + block.length;
      autosize();
    });
  }

  type LocationDraft =
    | { status: "locating" }
    | { status: "error"; error: string }
    | { status: "ready"; coords: GeolocationCoordinates };
  const LIVE_DURATIONS = [
    { ms: 15 * 60_000, label: tr("15 min") },
    { ms: 3600_000, label: tr("1 hour") },
    { ms: 8 * 3600_000, label: tr("8 hours") },
    // "Until I stop": a day at most, so a forgotten share ends on its own.
    { ms: 24 * 3600_000, label: tr("Until I stop") },
  ];
  const mySharing = $derived(liveLocation.sharing(target));
  const untilLabel = (endsAt: number, open: boolean) => (open ? tr("until you stop") : `until ${untilTime(endsAt)}`);

  async function shareLive(ms: number) {
    const d = locationDraft;
    if (d?.status !== "ready") return;
    locationDraft = null;
    await liveLocation.share(target, ms, d.coords).catch((e) => (sendError = String(e)));
  }
  let locationDraft = $state<LocationDraft | null>(null);

  function startLocation() {
    if (!navigator.geolocation) {
      locationDraft = { status: "error", error: tr("Location isn't available on this device") };
      return;
    }
    locationDraft = { status: "locating" };
    navigator.geolocation.getCurrentPosition(
      (pos) => (locationDraft = { status: "ready", coords: pos.coords }),
      (e) =>
        (locationDraft = {
          status: "error",
          error: e.code === e.PERMISSION_DENIED ? tr("AlwyChat isn't allowed to use your location") : tr("Couldn't find your location ({message})", { message: e.message }),
        }),
      { enableHighAccuracy: true, timeout: 20_000 },
    );
  }

  async function sendLocationDraft() {
    const d = locationDraft;
    if (d?.status !== "ready") return;
    locationDraft = null;
    await sendLocation(target, geoUri(d.coords.latitude, d.coords.longitude, d.coords.accuracy)).catch((e) => (sendError = String(e)));
  }
  let pollOpen = $state(false);
  $effect(() => {
    if (attachOpen) return closeOnBack(() => (attachOpen = false));
  });
  $effect(() => {
    if (pollOpen) return closeOnBack(() => (pollOpen = false));
  });
  $effect(() => {
    if (pinsOpen) return closeOnBack(() => (pinsOpen = false));
  });

  async function setPin(eventId: string, pin: boolean) {
    try {
      await pinMessage(room.id, eventId, pin);
    } catch (e) {
      sendError = String(e);
    }
  }

  async function showPin(eventId: string) {
    if (!(await tl.reveal(eventId))) {
      sendError = tr("That message is too far back to show");
      return;
    }
    await tick();
    jumpTo(eventId);
  }

  function onPinBanner() {
    if (!shownPin) return;
    void showPin(shownPin.event_id);
    pinIndex = (pinIndex + 1) % chat.pins.messages.length;
  }

  let searchOpen = $state(false);
  let mediaOpen = $state(false);
  $effect(() => {
    if (searchOpen) return closeOnBack(() => (searchOpen = false));
  });
  $effect(() => {
    if (mediaOpen) return closeOnBack(() => (mediaOpen = false));
  });

  /** Show a search result or shared item: in the loaded messages if it's there, else around it. */
  async function showMessage(eventId: string) {
    searchOpen = false;
    mediaOpen = false;
    await tick();
    if (tl.entries.some((e) => e.type === "message" && e.item_id === eventId)) return jumpTo(eventId);
    chat.jump(room.id, eventId);
  }

  // A timeline opened on a message: bring it into view once loaded.
  let jumpedTo: string | null = null;
  $effect(() => {
    const target = tl.focused;
    if (!target || !tl.loaded || jumpedTo === target) return;
    jumpedTo = target;
    tick().then(() => jumpTo(target));
  });
  $effect(() => {
    if (!tl.focused) jumpedTo = null;
  });
  // Or found among the latest messages.
  $effect(() => {
    const target = tl.revealed;
    if (!target) return;
    tl.revealed = null;
    tick().then(() => jumpTo(target));
  });

  async function jumpTo(eventId: string) {
    // Its row (a photo inside an album is found by the album's row), drawn if it wasn't.
    const at = rows.findIndex((r) => r.kind === "message" && (r.msg.item_id === eventId || r.album?.some((m) => m.item_id === eventId)));
    const row = rows[at];
    if (row?.kind !== "message") return;
    if (rows.length - at > limit) {
      limit = rows.length - at + 20;
      await tick();
    }
    const el = scroller.querySelector<HTMLElement>(`[data-event="${CSS.escape(row.msg.item_id)}"]`);
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
    if (c.kind === "image") return tr("📷 Photo");
    if (c.kind === "video") return tr("🎬 Video");
    if (c.kind === "file") return `📄 ${c.filename}`;
    if (c.kind === "audio") return c.is_voice ? tr("🎤 Voice message") : `🎵 ${c.filename}`;
    return tr("Message");
  }

  /** "14:32" today, "Tue 14:32" / "12 Mar 14:32" before. */
  function revisionTime(ms: number) {
    const day = shortTime(ms);
    const time = clockTime(ms);
    return day === time ? time : `${day} ${time}`;
  }

  const typingText = $derived.by(() => {
    const t = chat.typing;
    if (t.length === 0 || thread) return null;
    if (room.is_dm) return "typing";
    if (t.length === 1) return tr("{value} is typing", { value: t[0] });
    if (t.length === 2) return tr("{value} and {value2} are typing", { value: t[0], value2: t[1] });
    return tr("Several people are typing");
  });
</script>

<header class="chat-header" data-tauri-drag-region="deep">
  {#if selected}
  {@const a = selectionActions}
  <button class="back" onclick={() => (selected = null)} aria-label={tr("Cancel")}><X size={24} /></button>
  <strong class="title select-count">{trn(selected.size, "{n} selected", "{n} selected")}</strong>
  <button class="chat-menu-button" disabled={!a.copy} onclick={() => selectionDo(a.copy)} aria-label={tr("Copy text")} title={tr("Copy text")}><CopyIcon size={20} /></button>
  <button class="chat-menu-button" disabled={!a.forward} onclick={() => selectionDo(a.forward)} aria-label={tr("Forward")} title={tr("Forward")}><ForwardIcon size={20} /></button>
  <button class="chat-menu-button" disabled={!a.save} onclick={() => selectionDo(a.save)} aria-label={tr("Save to gallery")} title={tr("Save to gallery")}><Download size={20} /></button>
  <button class="chat-menu-button danger" disabled={!a.delete} onclick={() => selectionDo(a.delete)} aria-label={tr("Delete")} title={tr("Delete")}><Trash2 size={20} /></button>
  {:else}
  <button class="back" onclick={onBack} aria-label={tr("Back")}><ChevronLeft size={26} /></button>
  {#if thread}
  <span class="title thread-title">
    <strong>{tr("Thread")}</strong>
    <span class="sub">{title}</span>
  </span>
  {:else}
  <button class="who" class:fit={hasTitlebar} onclick={() => (chatMenuOpen = true)} aria-label={tr("Chat settings")}>
    <Avatar name={room.name} mxc={room.avatar} size={40} saved={saved.is(room.id)} />
    <span class="title">
      <strong>{title}</strong>
      {#if chat.style?.disappearing && !thread}
        <span class="sub"><Timer size={12} /> {tr("Disappearing messages: {time}", { time: disappearLabel(chat.style.disappearing.timer_ms) })}</span>
      {:else if status}
        <span class="sub" class:online={presence.online(room.dm_user)}>{status}</span>
      {:else if room.is_encrypted}
        <span class="sub"><Lock size={12} /> {tr("End-to-end encrypted")}</span>
      {/if}
    </span>
  </button>
  <button class="chat-menu-button" onclick={() => call.join(room, false)} aria-label={tr("Voice call")}><Phone size={20} /></button>
  <button class="chat-menu-button" onclick={() => call.join(room, true)} aria-label={tr("Video call")}><Video size={22} /></button>
  {/if}
  {/if}
</header>
{#if chat.style?.countdowns.length && !thread && !selected}
  <CountdownStrip items={chat.style.countdowns} onManage={() => (chatMenuOpen = true)} />
{/if}
{#if historyOpen && !thread}
  {#key room.id}<TimelineStrip roomId={room.id} onClose={() => (historyOpen = false)} onJump={(id) => void showMessage(id)} />{/key}
{/if}
{#if mySharing && !thread}
  <div class="live-bar" transition:fly={{ y: -6, duration: 140 }}>
    <span class="live-dot"></span>
    <span>{tr("Sharing your live location · {untilLabel}", { untilLabel: untilLabel(mySharing.endsAt, mySharing.endsAt - Date.now() > 20 * 3600_000) })}</span>
    <button type="button" onclick={() => liveLocation.stop(target)}>{tr("Stop")}</button>
  </div>
{/if}
{#if shownPin && !thread}
  <div class="pin-banner" transition:fade={{ duration: 150 }}>
    <button class="pin-main" onclick={onPinBanner}>
      <Pin size={16} />
      <span class="pin-text">
        <small>{tr("Pinned{value}", { value: chat.pins.messages.length > 1 ? ` · ${(pinIndex % chat.pins.messages.length) + 1}/${chat.pins.messages.length}` : "" })}</small>
        <span><strong>{shownPin.sender_name}:</strong> {shownPin.preview}</span>
      </span>
    </button>
    <button class="pin-list" onclick={() => (pinsOpen = true)} aria-label={tr("All pinned messages")}><List size={18} /></button>
  </div>
{/if}
{#if fileFor && fileFor.content.kind === "file"}
  {@const msg = fileFor}
  <FileSheet
    filename={fileFor.content.filename}
    onPick={(action) => onFileAction(msg, action)}
    onClose={() => (fileFor = null)}
  />
{/if}
{#if actionFor}
  <MessageActions
    msg={actionFor.msg}
    quickReaction={QUICK_REACTION}
    actions={actionsFor(actionFor.msg, actionFor.album, actionFor.link)}
    onClose={() => (actionFor = null)}
  />
{/if}
{#if forwardIds}
  <ForwardPage onPick={forwardTo} onClose={() => (forwardIds = null)} />
{/if}
{#if pollOpen}
  <PollCreator roomId={target} onClose={() => (pollOpen = false)} />
{/if}
{#if pinsOpen}
  <Page title={tr("Pinned messages")} onClose={() => (pinsOpen = false)}>
    <ul class="pins">
      {#each chat.pins.messages as p (p.event_id)}
        <li>
          <button class="pin-item" onclick={() => ((pinsOpen = false), void showPin(p.event_id))}>
            <strong>{p.sender_name}</strong>
            <span>{p.preview}</span>
          </button>
          {#if chat.pins.can_pin}
            <button class="unpin" onclick={() => setPin(p.event_id, false)} aria-label={tr("Unpin")}><PinOff size={18} /></button>
          {/if}
        </li>
      {/each}
    </ul>
  </Page>
{/if}
{#if room.in_call.length && call.roomId !== room.id && !thread}
  <button class="call-banner" onclick={() => call.join(room, false)} transition:fade={{ duration: 150 }}>
    <span class="call-dot"></span>
    <span>{tr("{length} in the call", { length: room.in_call.length })}</span>
    <strong>{tr("Join")}</strong>
  </button>
{/if}
{#if customizeOpen}
  <CustomizePage roomId={room.id} onClose={() => (customizeOpen = false)} />
{/if}
{#if chatMenuOpen}
  <ChatSettingsPage
    roomId={room.id}
    hd={chatHd}
    onChooseHd={chooseHd}
    onSharedMedia={() => ((chatMenuOpen = false), (mediaOpen = true))}
    onSearch={() => ((chatMenuOpen = false), (searchOpen = true))}
    onHistory={() => ((chatMenuOpen = false), (historyOpen = true))}
    onClose={() => (chatMenuOpen = false)}
    onLeft={() => ((chatMenuOpen = false), onBack?.())}
  />
{/if}

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
    <div class="drop-hint" transition:fade={{ duration: 120 }}>{tr("Drop to send")}</div>
  {/if}
  <div class="timeline" class:wide>
    {#if tl.error}
      <p class="center error">{friendlyError(tl.error)}</p>
    {/if}
    {#if tl.olderFailed}
      <p class="center error">
        {tr("Couldn't load older messages.")}
        <button class="retry" onclick={() => tl.older()}>{tr("Retry")}</button>
      </p>
    {/if}
    {#if tl.loadingOlder}
      <div class="spinner" aria-label={tr("Loading older messages")}></div>
    {/if}

    {#each shown as row (row.key)}
      {#if row.kind === "message"}
        {@const isSelected = !!selected?.has(row.msg.item_id)}
        <div data-event={row.msg.item_id} class="msg-row" class:selecting={!!selected} class:selected={isSelected}
          class:being-edited={editing?.item_id === row.msg.item_id} class:dimmed={!!editing && editing.item_id !== row.msg.item_id}>
          {#if selected}
            <!-- While selecting, a tap anywhere on the row picks it (the bubble's own taps wait). -->
            <button
              class="select-hit"
              disabled={!selectable(row.msg)}
              onclick={() => toggleSelect(row.msg)}
              aria-pressed={isSelected}
              aria-label={isSelected ? tr("Unselect") : tr("Select")}
            >
              <span class="select-mark">{#if isSelected}<Check size={14} strokeWidth={3} />{/if}</span>
            </button>
          {/if}
          <!-- One message that fails to draw mustn't take the whole conversation with it. -->
          <svelte:boundary onerror={(e) => console.warn("message", row.msg.item_id, e)}>
          <MessageBubble
            msg={row.msg}
            first={row.first}
            last={row.last}
            showSender={!room.is_dm}
            {wide}
            quickReaction={QUICK_REACTION}
            onReact={(key) => react(row.msg, key)}
            onReply={() => startReply(row.msg)}
            onJumpTo={jumpTo}
            onUnlock={() => (encryption.sheetOpen = true)}
            onEdit={() => startEdit(row.msg)}
            onDelete={() => (confirmDelete = [row.msg])}
            onHistory={() => showHistory(row.msg)}
            album={row.album}
            onOpenMedia={(i) => openMedia(row.album ?? [row.msg], i)}
            onSaveFile={() => (fileFor = row.msg)}
            onSaveToGallery={() => saveVisualMedia(row.album ?? [row.msg])}
            pinned={chat.pins.can_pin ? pinnedIds.has(row.msg.item_id) : null}
            onPin={(pin) => setPin(row.msg.item_id, pin)}
            roomId={target}
            onThread={thread ? undefined : () => (threadRoot = row.msg.item_id)}
            onForward={actionsFor(row.msg, row.album).forward}
            onSelect={selectable(row.msg) ? () => startSelect(row.msg) : undefined}
            onLongPress={(link) => (actionFor = { msg: row.msg, album: row.album, link })}
            onCopy={messageText(row.msg) ? () => copyText(messageText(row.msg)!) : undefined}
            onCopyLink={copyText}
          />
          {#if editing?.item_id === row.msg.item_id}<span class="edit-tag"><Pencil size={12} />{tr("Editing")}</span>{/if}
          {#snippet failed()}<p class="broken">{tr("Couldn't show this message")}</p>{/snippet}
          </svelte:boundary>
        </div>
      {:else if row.kind === "day"}
        <div class="day"><span>{row.label}</span></div>
      {:else if row.kind === "notice"}
        <p class="center notice">
          {row.text}{#if row.action === "theme" && !thread}
            · <button class="notice-link" onclick={() => (customizeOpen = true)}>{tr("Change")}</button>{/if}
        </p>
      {:else if row.kind === "unread"}
        <div class="unread"><span>{tr("New messages")}</span></div>
      {:else if row.kind === "start" && !thread}
        {#if room.predecessor}
          <p class="center notice">
            {tr("This chat continues an older version of it.")}
            <button class="link" onclick={() => room.predecessor && nav.openChat(room.predecessor)}>{tr("See older messages")}</button>
          </p>
        {:else}
          <p class="center notice">{tr("This is the beginning of {name} ✨", { name: room.name })}</p>
        {/if}
      {/if}
    {/each}

    {#if tl.loaded && rows.length === 0 && tl.atStart}
      <p class="center notice">{tr("No messages yet. Say hi 👋")}</p>
    {/if}

    {#if typingText}
      <div class="typing" transition:fade={{ duration: 150 }}>
        <span class="dots"><i></i><i></i><i></i></span>
        {typingText}
      </div>
    {/if}
  </div>
</div>

{#if !thread}<CallDock inChat />{/if}
{#if room.successor && !thread}
  <div class="moved">
    <p>{tr("This chat was upgraded. The conversation continues in its new version.")}</p>
    <button class="m-btn" disabled={movingOn} onclick={goToSuccessor}>{movingOn ? tr("Joining…") : tr("Go to the new chat")}</button>
    {#if moveError}<p class="m-error">{moveError}</p>{/if}
  </div>
{:else}
<form class="composer" onsubmit={send}>
  {#if editing}
    <div class="replying editing" transition:fly={{ y: 8, duration: 160 }}>
      <span class="mode"><Pencil size={16} /></span>
      <div>
        <strong>{"caption" in editing.content ? tr("Editing caption") : tr("Editing message")}</strong>
        <span>{editableText(editing)}</span>
      </div>
      <button type="button" onclick={cancelEdit} aria-label={tr("Cancel edit")}><X size={18} /></button>
    </div>
  {:else if replyTo}
    <div class="replying" transition:fly={{ y: 8, duration: 160 }}>
      <span class="mode"><Reply size={16} /></span>
      <div>
        <strong>{replyTo.is_own ? tr("Replying to yourself") : tr("Replying to {value}", { value: replyTo.sender_name })}</strong>
        <span>{replyText(replyTo)}</span>
      </div>
      <button type="button" onclick={() => (replyTo = null)} aria-label={tr("Cancel reply")}><X size={18} /></button>
    </div>
  {/if}
  {#if pending.length}
    <div class="pending" transition:fly={{ y: 8, duration: 160 }}>
      {#each pending as p, i (p)}
        <div class="pending-item">
          {#if p.url}<img src={p.url} alt="" />{:else}<span class="pending-file">{p.mime.startsWith("video/") ? "🎬" : "📄"}<small>{p.name}</small></span>{/if}
          <button type="button" onclick={() => removePending(i)} aria-label={tr("Remove {name}", { name: p.name })}>×</button>
          {#if editable(p)}
            <button type="button" class="edit-photo" onclick={() => (editingPhoto = i)} aria-label={tr("Edit photo")} title={tr("Edit photo")}><Pencil size={13} /></button>
          {/if}
          {#if canViewOnce(p)}
            <button
              type="button"
              class="view-once"
              class:on={p.viewOnce}
              onclick={() => (pending[i] = { ...p, viewOnce: !p.viewOnce })}
              aria-pressed={!!p.viewOnce}
              aria-label={tr("View once")}
              title={p.viewOnce ? tr("View once: on") : tr("View once: off")}
            >1</button>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
  {#if mention.open}
    <MentionSuggestions suggestions={mention.suggestions} index={mention.index} onPick={pickMention} />
  {/if}
  {#if sendingFiles}<p class="send-status">{tr("Sending…")}</p>{/if}
  {#if sendError}<p class="send-error">{friendlyError(sendError)}</p>{/if}
  {#if recorder.recording}
    <div class="bar recording" transition:fade={{ duration: 120 }}>
      <button type="button" class="attach" onclick={() => recorder.cancel()} aria-label={tr("Delete recording")}><Trash2 size={22} /></button>
      <div class="rec">
        <span class="rec-dot"></span>
        <span class="rec-time">{recordClock}</span>
        <div class="rec-wave" aria-hidden="true">
          {#each recorder.levels as level, i (i)}
            <span style:height="{Math.max(8, Math.round(level * 100))}%"></span>
          {/each}
        </div>
      </div>
      <button type="button" class="send" onclick={finishVoice} aria-label={tr("Send voice message")}><SendHorizontal size={20} /></button>
    </div>
  {:else}
  {#if formatting}
    <div class="format-bar" transition:fly={{ y: 6, duration: 140 }}>
      <button type="button" onclick={() => wrap("**", "**")} aria-label={tr("Bold")}><Bold size={18} /></button>
      <button type="button" onclick={() => wrap("*", "*")} aria-label={tr("Italic")}><Italic size={18} /></button>
      <button type="button" onclick={() => wrap("~~", "~~")} aria-label={tr("Strikethrough")}><Strikethrough size={18} /></button>
      <button type="button" onclick={() => wrap("`", "`")} aria-label={tr("Code")}><Code size={18} /></button>
      <button type="button" onclick={() => wrap("```\n", "\n```")} aria-label={tr("Code block")}><SquareCode size={18} /></button>
      <button type="button" onclick={() => linePrefix("> ")} aria-label={tr("Quote")}><TextQuote size={18} /></button>
      <button type="button" onclick={() => linePrefix("- ")} aria-label={tr("Bulleted list")}><List size={18} /></button>
      <button type="button" onclick={() => linePrefix("1. ")} aria-label={tr("Numbered list")}><ListOrdered size={18} /></button>
      <button type="button" onclick={() => wrap("[", "](https://)")} aria-label={tr("Link")}><LinkIcon size={18} /></button>
      <button type="button" class="close" onclick={() => (formatting = false)} aria-label={tr("Close formatting")}><X size={18} /></button>
    </div>
  {/if}
  {#if locationDraft}
    <div class="location-confirm" transition:fly={{ y: 6, duration: 140 }}>
      <div class="loc-head">
        <MapPin size={22} />
        <span>
          {#if locationDraft.status === "locating"}{tr("Finding your location…")}
          {:else if locationDraft.status === "error"}{locationDraft.error}
          {:else}{tr("Your location")}<small>{locationDraft.coords.latitude.toFixed(5)}, {locationDraft.coords.longitude.toFixed(5)}{locationDraft.coords.accuracy ? ` · ±${Math.round(locationDraft.coords.accuracy)} m` : ""}</small>{/if}
        </span>
        <button type="button" class="plain" onclick={() => (locationDraft = null)}>{tr("Cancel")}</button>
      </div>
      {#if locationDraft.status === "ready"}
        <button type="button" class="loc-option" onclick={sendLocationDraft}><MapPin size={18} /> {tr("Send my current location")}</button>
        <p class="loc-label">{tr("Share live location")}</p>
        <div class="loc-durations">
          {#each LIVE_DURATIONS as d (d.ms)}
            <button type="button" onclick={() => shareLive(d.ms)}>{d.label}</button>
          {/each}
        </div>
        <small class="loc-note">{tr("Live: your position updates while AlwyChat is open, until the time is up or you tap Stop.")}</small>
      {/if}
    </div>
  {/if}
  <div class="bar">
    {#if typingMode && !toolsShown}
      <button type="button" class="attach" onclick={() => (toolsShown = true)} aria-label={tr("Show more")}><ChevronRight size={26} /></button>
    {:else}
      <button type="button" class="attach plus" class:open={attachOpen} onclick={() => (attachOpen = !attachOpen)} aria-label={tr("More")} aria-expanded={attachOpen}>
        <CirclePlus size={26} />
      </button>
      {#if isAndroid}
        <button type="button" class="attach" onclick={() => (cameraOpen = true)} aria-label={tr("Take a photo")}><Camera size={24} /></button>
      {/if}
      <button type="button" class="attach" onclick={() => (isAndroid ? pick("media") : openFileInput("image/*,video/*"))} aria-label={tr("Photos and videos")}><Image size={24} /></button>
      <button type="button" class="attach mic" onclick={startVoice} aria-label={tr("Record a voice message")}><Mic size={24} /></button>
    {/if}
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
    <div class="field" class:editing={!!editing}>
      <textarea
        bind:this={input}
        bind:value={draft}
        oninput={() => ((toolsShown = false), onInput())}
        onfocus={() => (emojiOpen = false)}
        onblur={() => (stopTyping(room.id), (mention.query = null))}
        onclick={() => mention.update(input.value, input.selectionStart ?? input.value.length)}
        onpaste={onPaste}
        onkeydown={onKeydown}
        rows="1"
        enterkeyhint={touchKeyboard ? "enter" : "send"}
        placeholder={pending.length ? "Add a caption" : "Message"}
      ></textarea>
      <button type="button" class="emoji-toggle" class:on={emojiOpen} onclick={toggleEmoji} aria-label={tr("Emoji")}><SmilePlus size={22} /></button>
    </div>
    {#if draft.trim() || editing || pending.length}
      <!-- Never takes the focus: the keyboard stays open for the next message. -->
      <button class="send" class:editing={!!editing} aria-label={editing ? tr("Save edit") : tr("Send")} onmousedown={keepFocus} transition:fly={{ x: 8, duration: 140 }}
        >{#if editing}<Check size={20} />{:else}<SendHorizontal size={20} />{/if}</button
      >
    {:else}
      <button
        type="button"
        class="quick"
        aria-label={tr("Send {QUICK_REACTION}", { QUICK_REACTION: QUICK_REACTION })}
        onmousedown={keepFocus}
        onclick={() => sendMessage(target, QUICK_REACTION, null).catch((e) => (sendError = String(e)))}
      >{QUICK_REACTION}</button>
    {/if}
  </div>
  {#if emojiOpen}
    <div transition:fly={{ y: 20, duration: 160 }}><EmojiPicker onPick={insertEmoji} packs={emotes.packs} onCustom={insertCustom} onSticker={sticker} /></div>
  {/if}
  {/if}
</form>
{/if}

{#if attachOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="sheet-backdrop" transition:fade={{ duration: 120 }} onclick={() => (attachOpen = false)}></div>
  <div class="attach-sheet" role="menu" transition:fly={{ y: 40, duration: 200 }}>
    <span class="grabber"></span>
    {#if isAndroid}
      <button type="button" onclick={() => sheet(() => (cameraOpen = true))}><Camera size={22} />{tr("Take photo")}</button>
      <button type="button" onclick={() => sheet(() => takePhoto("video"))}><Video size={22} />{tr("Record video")}</button>
    {/if}
    <button type="button" onclick={() => sheet(() => (isAndroid ? pick("media") : openFileInput("image/*,video/*")))}><Image size={22} />{tr("Photo & video library")}</button>
    <button type="button" onclick={() => sheet(() => (isAndroid ? pick("files") : openFileInput("")))}><Paperclip size={22} />{tr("File")}</button>
    <button type="button" onclick={() => sheet(startLocation)}><MapPin size={22} />{tr("Share location")}</button>
    <button type="button" onclick={() => sheet(() => (pollOpen = true))}><ChartBar size={22} />{tr("Poll")}</button>
    <button type="button" onclick={() => sheet(() => (formatting = !formatting))}><Type size={22} />{formatting ? tr("Hide text formatting") : tr("Text formatting")}</button>
  </div>
{/if}

{#if toast}<div class="toast" transition:fly={{ y: 12, duration: 180 }}>{toast}</div>{/if}

{#if viewer}
  <MediaViewer items={viewer.items} index={viewer.index} onClose={() => ((viewer = null), (onceOpen = false))} />
{/if}
{#if searchOpen}
  <ChatSearchPage roomId={room.id} onPick={showMessage} onClose={() => (searchOpen = false)} />
{/if}
{#if mediaOpen}
  <SharedMediaPage roomId={room.id} onShow={showMessage} onClose={() => (mediaOpen = false)} />
{/if}
{#if threadRoot}
  {#key threadRoot}
    <div class="thread-layer" transition:fly={{ x: 48, duration: 200 }}>
      <ChatView {room} thread={threadRoot} onBack={() => (threadRoot = null)} />
    </div>
  {/key}
{/if}
{#if scrolledUp && !tl.focused && !thread}
  <button class="to-bottom" class:fresh={newBelow} onclick={toBottom} aria-label={newBelow ? tr("New messages below") : tr("Back to the newest messages")} transition:fade={{ duration: 150 }}>
    <ArrowDown size={20} />
    {#if newBelow}<span class="fresh-dot"></span>{/if}
  </button>
{/if}
{#if tl.focused}
  <button class="latest" onclick={() => tl.latest()} transition:fade={{ duration: 150 }}>
    <ArrowDown size={16} /> {tr("Latest")}
  </button>
{/if}

{#if cameraOpen}
  <CameraView onShot={(blob, name) => void addShot(blob, name)} onClose={() => (cameraOpen = false)} onFallback={isAndroid ? () => takePhoto("photo") : undefined} />
{/if}
{#if editingPhoto !== null && pending[editingPhoto]?.url}
  {@const i = editingPhoto}
  <PhotoEditor src={pending[i].url!} onCancel={() => (editingPhoto = null)} onDone={(jpeg) => photoEdited(i, jpeg)} />
{/if}

{#if confirmDelete}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="backdrop" transition:fade={{ duration: 120 }} onclick={() => (confirmDelete = null)}></div>
  <div class="dialog" role="alertdialog" aria-labelledby="del-title" transition:fly={{ y: 16, duration: 180 }}>
    <h3 id="del-title">{confirmDelete.length > 1 ? trn(confirmDelete.length, "Delete {n} message?", "Delete {n} messages?") : tr("Delete this message?")}</h3>
    <p>{tr("It will be removed for everyone in this chat.")}</p>
    <div class="dialog-buttons">
      <button onclick={() => (confirmDelete = null)}>{tr("Cancel")}</button>
      <button class="danger" onclick={() => doDelete(confirmDelete!)}>{tr("Delete")}</button>
    </div>
  </div>
{/if}

{#if history}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="backdrop" transition:fade={{ duration: 120 }} onclick={() => (history = null)}></div>
  <div class="dialog" role="dialog" aria-labelledby="hist-title" transition:fly={{ y: 16, duration: 180 }}>
    <h3 id="hist-title">{tr("Edit history")}</h3>
    {#if history.error}
      <p class="error">{history.error}</p>
    {:else if !history.revisions}
      <p>{tr("Loading…")}</p>
    {:else}
      <ol class="revisions">
        {#each history.revisions as rev, i (i)}
          <li class:current={i === history.revisions.length - 1}>
            <small>{i === 0 ? tr("Original") : tr("Edit {i}", { i: i })}{rev.timestamp ? ` · ${revisionTime(rev.timestamp)}` : ""}</small>
            <span>{rev.text}</span>
          </li>
        {/each}
      </ol>
    {/if}
    <div class="dialog-buttons">
      <button onclick={() => (history = null)}>{tr("Close")}</button>
    </div>
  </div>
{/if}

<style>
  /* A thread covers its chat (the chat pane is its positioning box). */
  .thread-layer {
    position: absolute;
    inset: 0;
    z-index: 20;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .thread-title {
    flex: 1;
  }
  /* Selecting messages: a tap target over each row, with its check circle on the left. */
  .msg-row {
    position: relative;
  }
  .msg-row.selecting {
    padding-left: 36px;
    transition: padding 0.18s var(--ease, ease);
  }
  /* Editing: the message stands out (pink band + "Editing" tag), everything else fades. */
  .msg-row {
    transition: opacity 0.2s var(--ease, ease);
  }
  .msg-row.dimmed {
    opacity: 0.3;
  }
  .msg-row.being-edited {
    border-radius: 12px;
    background: color-mix(in srgb, var(--accent-b) 16%, transparent);
  }
  .edit-tag {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    padding: 0 12px 4px;
    color: var(--accent-b);
    font-size: 0.75rem;
    font-weight: 600;
  }
  .msg-row.selected {
    background: color-mix(in srgb, var(--accent-a) 14%, transparent);
  }
  .select-hit {
    position: absolute;
    inset: 0;
    z-index: 3;
    display: flex;
    align-items: center;
    padding: 0 0 0 10px;
    border: 0;
    background: transparent;
    cursor: pointer;
  }
  .select-hit:disabled {
    cursor: default;
  }
  .select-mark {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 2px solid var(--muted);
    color: #fff;
  }
  .select-hit[aria-pressed="true"] .select-mark {
    border-color: transparent;
    background: var(--accent);
  }
  .select-hit:disabled .select-mark {
    opacity: 0.3;
  }
  .select-count {
    flex: 1;
    font-size: 1.05rem;
  }
  .chat-menu-button:disabled {
    opacity: 0.35;
  }
  .chat-menu-button.danger:not(:disabled) {
    color: var(--danger, #e5484d);
  }
  .chat-header {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px calc(16px + var(--window-controls)) 12px 16px;
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
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 0.8rem;
    color: var(--muted);
  }
  .sub.online {
    color: #31d158;
  }
  .call-banner {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 6px 12px 0;
    padding: 9px 14px;
    border: none;
    border-radius: 14px;
    background: color-mix(in srgb, #31d158 18%, var(--surface));
    text-align: left;
    cursor: pointer;
  }
  .call-banner span:nth-child(2) {
    flex: 1;
  }
  .call-banner strong {
    padding: 4px 12px;
    border-radius: 999px;
    background: #1f9d48;
    color: white;
  }
  .pin-banner {
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }
  .pin-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px 8px 16px;
    border: none;
    border-left: 3px solid var(--accent-a);
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .pin-main :global(svg) {
    flex: none;
    color: var(--accent-a);
  }
  .pin-text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    font-size: 0.85rem;
  }
  .pin-text small {
    color: var(--accent-a);
    font-weight: 600;
  }
  .pin-text span {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .pin-list,
  .unpin {
    flex: none;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    margin-right: 6px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .pins {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .pins li {
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--border);
  }
  .pin-item {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 4px;
    border: none;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .pin-item span {
    color: var(--muted);
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .call-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #31d158;
  }
  .back {
    display: none;
    place-items: center;
    width: 36px;
    height: 36px;
    margin-left: -6px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  @media (max-width: 720px) {
    .back {
      display: grid;
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
  /* Everyone on the left has no bubbles: over an illustrated theme or a photo, the text gets a
     soft dark panel behind it to stay readable. */
  :global(.chat-pane.scenic) .timeline.wide {
    margin: 8px 8px 0;
    border-radius: 18px;
    /* No backdrop-filter: it would trap fixed-position sheets and menus inside the panel. */
    background: rgb(8 10 18 / 0.62);
  }
  .retry {
    margin-left: 6px;
    padding: 2px 10px;
    border: 1px solid currentColor;
    border-radius: 999px;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .broken {
    margin: 2px 16px;
    color: var(--muted);
    font-size: 0.8rem;
    font-style: italic;
  }
  .center {
    margin: 10px 24px;
    text-align: center;
  }
  .notice {
    font-size: 0.82rem;
    color: var(--muted);
  }
  .notice-link {
    padding: 0;
    border: none;
    background: none;
    color: var(--accent-b);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
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
  /* Replying = violet with a reply arrow; editing = pink with a pencil, tinted, and the field
     outlined, so the two can't be mixed up at a glance. */
  .replying strong {
    color: var(--accent-a);
  }
  .mode {
    display: grid;
    flex: none;
    width: 30px;
    height: 30px;
    place-items: center;
    border-radius: 50%;
    background: color-mix(in srgb, var(--accent-a) 18%, transparent);
    color: var(--accent-a);
  }
  .replying.editing {
    border-left-color: transparent;
    background: var(--accent-b);
    color: white;
  }
  .replying.editing strong,
  .replying.editing span,
  .replying.editing button {
    color: white;
  }
  .replying.editing span {
    opacity: 0.85;
  }
  .replying.editing .mode {
    background: rgb(255 255 255 / 0.25);
    color: white;
  }
  .field.editing {
    border-radius: 20px;
    box-shadow: inset 0 0 0 1.5px var(--accent-b);
  }
  .replying button {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: none;
    border-radius: 50%;
    background: none;
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
  /* No scrollbar next to the field (it grows up to max-height, then scrolls silently). */
  textarea {
    scrollbar-width: none;
  }
  textarea::-webkit-scrollbar {
    display: none;
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
    display: grid;
    place-items: center;
    background: var(--accent);
    color: white;
  }
  .send.editing {
    background: var(--accent-b);
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
  .to-bottom {
    position: fixed;
    right: 16px;
    bottom: 84px;
    z-index: 5;
    display: grid;
    width: 42px;
    height: 42px;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: var(--surface-2);
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.3);
    color: var(--text);
    cursor: pointer;
  }
  .to-bottom.fresh {
    border-color: transparent;
    background: var(--accent);
    color: white;
  }
  .fresh-dot {
    position: absolute;
    top: 2px;
    right: 2px;
    width: 10px;
    height: 10px;
    border: 2px solid var(--bg);
    border-radius: 50%;
    background: var(--danger);
  }
  .latest {
    position: fixed;
    right: 16px;
    bottom: 84px;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    border: none;
    border-radius: 999px;
    background: var(--accent);
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.3);
    color: white;
    cursor: pointer;
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
  /* Computer apps: only the name is a button, the rest of the header moves the window. */
  .who.fit {
    flex: 0 1 auto;
    margin-right: auto;
  }
  .who {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 6px;
    border: none;
    border-radius: 14px;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .who:hover {
    background: var(--surface-2);
  }
  .chat-menu-button {
    width: 36px;
    height: 36px;
    border: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    padding: 0;
    background: none;
    color: var(--accent-a);
    cursor: pointer;
  }
  .chat-menu-button:hover {
    background: var(--surface-2);
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
    display: grid;
    place-items: center;
    padding: 0;
    border-radius: 50%;
    background: none;
    color: var(--accent-a);
    cursor: pointer;
  }
  .field {
    position: relative;
    display: flex;
    flex: 1;
    min-width: 0;
  }
  .field textarea {
    padding-right: 44px;
  }
  .emoji-toggle {
    position: absolute;
    right: 4px;
    bottom: 2px;
    display: grid;
    width: 36px;
    height: 36px;
    place-items: center;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--accent-a);
    cursor: pointer;
  }
  .emoji-toggle.on {
    color: var(--text);
  }
  .format-bar {
    display: flex;
    gap: 2px;
    margin-bottom: 6px;
    padding: 4px;
    overflow-x: auto;
    border-radius: 14px;
    background: var(--surface-2);
    scrollbar-width: none;
  }
  .format-bar button {
    display: grid;
    flex: none;
    width: 38px;
    height: 34px;
    place-items: center;
    border: none;
    border-radius: 9px;
    background: none;
    color: var(--text);
    cursor: pointer;
  }
  .format-bar button:hover {
    background: var(--surface-hover);
  }
  .format-bar .close {
    margin-left: auto;
    color: var(--muted);
  }
  .live-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 8px 12px 0;
    padding: 8px 8px 8px 14px;
    border-radius: 14px;
    background: color-mix(in srgb, #31d158 14%, var(--surface-2));
    font-size: 0.9rem;
  }
  .live-bar span:nth-child(2) {
    flex: 1;
  }
  .live-bar button {
    padding: 6px 14px;
    border: none;
    border-radius: 999px;
    background: var(--danger);
    color: white;
    font: inherit;
    cursor: pointer;
  }
  .live-dot {
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #31d158;
    animation: rec-pulse 1s ease-in-out infinite alternate;
  }
  .loc-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .loc-head > :global(svg) {
    flex: none;
    color: var(--accent-a);
  }
  .loc-head span {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .location-confirm .loc-option {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
    padding: 10px 14px;
    border-radius: 12px;
    background: var(--accent);
    color: white;
  }
  .loc-label {
    margin: 12px 0 6px;
    color: var(--muted);
    font-size: 0.85rem;
  }
  .loc-durations {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .location-confirm .loc-durations button {
    background: var(--surface);
    color: var(--text);
  }
  .loc-note {
    display: block;
    margin-top: 8px;
    color: var(--muted);
  }
  .location-confirm {
    display: flex;
    flex-direction: column;
    margin-bottom: 6px;
    padding: 10px 12px;
    border-radius: 14px;
    background: var(--surface-2);
  }

  .location-confirm small {
    color: var(--muted);
  }
  .location-confirm button {
    flex: none;
    padding: 7px 14px;
    border: none;
    border-radius: 999px;
    font: inherit;
    cursor: pointer;
  }
  .location-confirm .plain {
    background: none;
    color: var(--muted);
  }

  .sheet-backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgb(0 0 0 / 0.45);
  }
  .attach-sheet {
    position: fixed;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 41;
    display: flex;
    flex-direction: column;
    max-width: 560px;
    margin: 0 auto;
    padding: 8px 12px calc(14px + env(safe-area-inset-bottom));
    border-radius: 22px 22px 0 0;
    background: var(--surface);
    box-shadow: 0 -12px 40px rgb(0 0 0 / 0.35);
  }
  .grabber {
    align-self: center;
    width: 40px;
    height: 4px;
    margin: 4px 0 10px;
    border-radius: 2px;
    background: var(--border);
  }
  .attach-sheet button {
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 14px 12px;
    border: none;
    border-radius: 12px;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 1.02rem;
    text-align: left;
    cursor: pointer;
  }
  .attach-sheet button:hover {
    background: var(--surface-hover);
  }
  .attach-sheet :global(svg) {
    color: var(--muted);
  }
  .plus :global(svg) {
    transition: transform 0.2s var(--ease);
  }
  .plus.open :global(svg) {
    transform: rotate(45deg);
  }
  .attach:hover {
    background: var(--surface-2);
  }
  .bar.recording {
    align-items: center;
  }
  .rec {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 40px;
    padding: 0 14px;
    border-radius: 20px;
    background: var(--surface-2);
  }
  .rec-dot {
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--danger);
    animation: rec-pulse 1s ease-in-out infinite alternate;
  }
  @keyframes rec-pulse {
    to {
      opacity: 0.3;
    }
  }
  .rec-time {
    flex: none;
    font-variant-numeric: tabular-nums;
  }
  .rec-wave {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 2px;
    height: 24px;
    overflow: hidden;
  }
  .rec-wave span {
    flex: none;
    width: 3px;
    border-radius: 2px;
    background: var(--accent);
    transition: height 0.05s linear;
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
  .pending-item .view-once {
    top: auto;
    right: auto;
    bottom: 3px;
    left: 3px;
    font-size: 11px;
    font-weight: 700;
    border: 1.5px dashed rgb(255 255 255 / 0.85);
  }
  .pending-item .view-once.on {
    background: var(--accent);
    border-style: solid;
  }
  .pending-item .edit-photo {
    top: auto;
    bottom: 3px;
    display: grid;
    place-items: center;
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
  .moved {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 14px 16px calc(14px + env(safe-area-inset-bottom));
    border-top: 1px solid var(--border);
    color: var(--muted);
    text-align: center;
  }
  .moved p {
    margin: 0;
  }
</style>
