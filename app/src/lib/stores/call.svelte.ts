// The call we're in: MatrixRTC signalling in the core, audio/video with livekit-client here,
// or in Rust where the webview has no WebRTC (Linux desktop: `native`; pictures arrive as frames,
// see nativeVideo.svelte.ts).
import { loudness, playCue, type CallCue } from "$lib/callSounds";
import type { AudioCaptureOptions, LocalAudioTrack, LocalTrack, LocalVideoTrack, Participant, RemoteAudioTrack, RemoteParticipant, RemoteTrack, Room, Track as LkTrack } from "livekit-client";
import type { MatrixKeyProvider as KeyProvider } from "$lib/callKeys";
import {
  isAndroid,
  launchCall,
  onCallLink,
  type CallLink,
  callAudio,
  audioRoutes,
  callStartedAt,
  type AudioRoute,
  type AudioRoutes,
  callNotification,
  onCallAction,
  declineCall,
  joinCall,
  leaveCall,
  nativeCallJoin,
  nativeCallLeave,
  nativeCalls,
  nativeCallSetDeafened,
  nativeCallSetMic,
  nativeCallSetMutedForMe,
  nativeCallSetSoundboardOff,
  nativeCallSetSoundboardVolume,
  nativeCallSetVolume,
  nativeCallSetSetup,
  nativeCallSetCamera,
  nativeCallSetScreen,
  onCallSignal,
  callKeys,
  onCallKey,
  nativeCallPlaySound,
  type NativeAudioSetup,
  type NativeCallMember,
  roomMembers,
  ringUser,
  raiseHand,
  callReact,
  callAlert,
  isWeb,
  type CallSignal,
  type Member,
  type RoomSummary,
  backgroundCamera,
  pipAllowed,
  nativeCallScreenPreview,
  nativeCallScreenQuality,
  nativeCallScreenStats,
  nativeCallScreenSound,
  nativeCallSetScreenSound,
  nativeCallWatchScreen,
  isWindows,
  winScreenQuality,
  winScreenSound,
  winScreenSources,
  type WinShareEvent,
  type WinSound,
  type WinSource,
  type NativeScreenSound,
  type SoundChoice,
} from "$lib/backend";
import { MediaQuery } from "svelte/reactivity";
import { startRinging, stopRinging } from "$lib/ringtone";
import { nav } from "./nav.svelte";
import { chatList } from "./chatList.svelte";
import { nativeVideo } from "./nativeVideo.svelte";
import { WinShare, winShareSupported } from "./winShare";
import { tr } from "$lib/i18n/index.svelte";

// livekit-client is a large part of the app's code and only calls in the web view need it
// (never Linux, which has its own engine): it's loaded when the first such call starts.
type LiveKit = typeof import("livekit-client");
let Track!: LiveKit["Track"];
let engine: Promise<[LiveKit, typeof import("$lib/callKeys")]> | null = null;
function loadEngine() {
  engine ??= Promise.all([import("livekit-client"), import("$lib/callKeys")]);
  engine.catch(() => (engine = null));
  return engine.then((loaded) => ((Track = loaded[0].Track), loaded));
}

/** Phones choose cameras by facing mode (front/back) rather than by device. */
const PHONE = isAndroid || /iPhone|iPad/.test(navigator.userAgent);

type Ring = Extract<CallSignal, { kind: "ring" }>;
/** A 1:1 call nobody answers ends after this. */
const NO_ANSWER_MS = 45_000;
/** How long a ring into a call lasts (the core's ring lifetime). */
const INVITE_RING_MS = 30_000;

/** Someone we rang into the call. "calling": sent, no device said it rings (it may still: only
 * AlwyChat says so); "ringing": rings on `devices` of theirs. */
export interface Invite {
  status: "calling" | "ringing" | "declined" | "no_answer" | "failed";
  devices: string[];
}

export interface Tile {
  /** LiveKit identity: "@user:server:DEVICE". */
  id: string;
  userId: string;
  name: string;
  avatar: string | null;
  isMe: boolean;
  speaking: boolean;
  micOn: boolean;
  camera: LkTrack | null;
  screen: LkTrack | null;
  /** Native calls: frame keys of the camera / screen (drawn by NativeVideo.svelte). */
  cameraFrames: string | null;
  screenFrames: string | null;
  /** They share their screen, watched or not (see `call.watchScreen`). */
  sharing: boolean;
  /** Their connection is lost: they come back, or leave after LiveKit's timeout. */
  lost: boolean;
}

/** Someone rung into the call who hasn't joined yet. */
export interface Waiting {
  userId: string;
  name: string;
  avatar: string | null;
  /** "ringing": one of their devices said it rings (AlwyChat only; other apps never say). */
  status: "calling" | "ringing";
}

type Status = "connecting" | "connected" | "reconnecting";

/** Call preferences of this device (not synced). */
export interface CallPrefs {
  noiseSuppression: boolean;
  echoCancellation: boolean;
  autoGain: boolean;
  pushToTalk: boolean;
  /** Push-to-talk key on a keyboard (`KeyboardEvent.code`). */
  pttKey: string;
  /** What that key prints with the user's layout ("A" on AZERTY where QWERTY has Q). */
  pttKeyLabel: string | null;
  /** Chosen devices (null: the system default). Browser device IDs, or the native engine's
   * on the Linux desktop app (the two lists differ). */
  inputDevice: string | null;
  outputDevice: string | null;
  cameraDevice: string | null;
  /** Per-person volume, 0–1, by user ID (1 when absent). */
  volumes: Record<string, number>;
  /** People muted for us only (their volume is kept for when they're unmuted). */
  muted: string[];
  /** How loud soundboard sounds are for us, ours and others' (0–1). */
  soundboardVolume: number;
  /** The "Call sounds" slider, 0–1 (0 = off; callSounds.ts turns it into loudness). */
  callSoundLevel: number;
  /** How loud in-call reactions' sounds (Element Call's) are for us, 0–1 (0 = none). */
  reactionVolume: number;
  /** Android, Bluetooth headset: let go of its mic while ours is muted, so it plays in full
   * quality (its call mode is mono and muffled, for every app). */
  btListenMode: boolean;
  /** Android: show Bluetooth headsets' names (needs "Nearby devices"). */
  btNames: boolean;
  /** Android: going home during a call shrinks it into picture-in-picture. */
  pip: boolean;
  /** Screen sharing: the quality last used (fluid for games, balanced, or sharp text). */
  screenQuality: ScreenQuality;
  /** What a new share starts with: the last used quality, or always the same. */
  screenDefault: ScreenQuality | "last";
  /** Linux: the app whose sound was last picked for a window share (picked again if playing). */
  screenApp?: string;
  /** Screen sharing codec: AV1 = lightest stream; Auto = H.264 on the graphics card where known. */
  screenCodec: ScreenCodec;
  /** How much data a screen share may use (scales the quality's bitrate). */
  screenData: ScreenData;
}

export type ScreenCodec = "auto" | "av1" | "h264" | "vp8";
export type ScreenData = "low" | "normal" | "high";
/** Bitrate factors: the data choice, and AV1 needing ~40 % less for the same picture. */
const DATA_FACTOR: Record<ScreenData, number> = { low: 0.45, normal: 1, high: 1.5 };
export const CODEC_FACTOR = (c: ScreenCodec) => (c === "av1" ? 0.6 : 1);
/** What a share sends right now (its biggest layer). */
export interface ScreenStats {
  encoder: string;
  fps: number;
  width: number;
  height: number;
  /** "none", "cpu", "bandwidth", "other". */
  limit: string;
  mbps: number;
}

export type ScreenQuality = "speed_max" | "speed" | "balanced" | "quality" | "quality_max";
/** What each screen-share choice asks of the encoder; bigger screens are shrunk to fit. */
export const SCREEN_QUALITY: Record<ScreenQuality, { fps: number; maxBitrate: number; width: number; height: number; fluid: boolean }> = {
  speed_max: { fps: 60, maxBitrate: 6_000_000, width: 1280, height: 720, fluid: true },
  speed: { fps: 60, maxBitrate: 8_000_000, width: 1920, height: 1080, fluid: true },
  balanced: { fps: 30, maxBitrate: 5_000_000, width: 1920, height: 1080, fluid: true },
  quality: { fps: 30, maxBitrate: 6_000_000, width: 2560, height: 1440, fluid: false },
  quality_max: { fps: 15, maxBitrate: 6_000_000, width: 3840, height: 2160, fluid: false },
};

/** The share's quality steps, from speed (frame rate) to quality (sharpness), as offered. */
export const SCREEN_CHOICES: { id: ScreenQuality; label: () => string; hint: () => string }[] = [
  { id: "speed_max", label: () => tr("Max speed"), hint: () => tr("60 fps at 720p: the smoothest, even on a slow connection") },
  { id: "speed", label: () => tr("Speed"), hint: () => tr("60 fps at 1080p: games and videos") },
  { id: "balanced", label: () => tr("Balanced"), hint: () => tr("30 fps at 1080p: good for most things") },
  { id: "quality", label: () => tr("Quality"), hint: () => tr("30 fps at 1440p: sharper text and details") },
  { id: "quality_max", label: () => tr("Max quality"), hint: () => tr("15 fps at full size (up to 4K): crisp documents and code") },
];

/** Someone in a call or a voice channel, as the person menu shows them. */
export interface CallPerson {
  userId: string;
  name: string;
  avatar: string | null;
  isMe: boolean;
  /** We're in that call with them: volume and mute apply. */
  live: boolean;
}
/** Where call audio goes when one of these is connected (best first). */
const HEADSETS: AudioRoute["kind"][] = ["bluetooth", "wired", "usb"];
const PREFS_KEY = "alwychat.callPrefs";
function loadPrefs(): CallPrefs {
  const defaults: CallPrefs = {
    noiseSuppression: true,
    echoCancellation: true,
    autoGain: true,
    pushToTalk: false,
    pttKey: "Space",
    pttKeyLabel: null,
    inputDevice: null,
    outputDevice: null,
    cameraDevice: null,
    volumes: {},
    muted: [],
    soundboardVolume: 1,
    callSoundLevel: 0.5,
    reactionVolume: 1,
    btListenMode: false,
    btNames: false,
    pip: false,
    screenQuality: "balanced",
    screenDefault: "last",
    screenCodec: "auto",
    screenData: "normal",
  };
  try {
    const saved = { ...defaults, ...JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}") } as CallPrefs;
    // Before 5 steps: "smooth" / "text".
    const old: Record<string, ScreenQuality> = { smooth: "speed", text: "quality_max" };
    if (old[saved.screenQuality]) saved.screenQuality = old[saved.screenQuality];
    if (saved.screenDefault !== "last" && old[saved.screenDefault]) saved.screenDefault = old[saved.screenDefault];
    if (!SCREEN_QUALITY[saved.screenQuality]) saved.screenQuality = "balanced";
    if (saved.screenDefault !== "last" && !SCREEN_QUALITY[saved.screenDefault]) saved.screenDefault = "last";
    return saved;
  } catch {
    return defaults;
  }
}

/** "@alex:example.org:DEVICE" → "@alex:example.org" (user IDs contain a colon themselves). */
const userOf = (identity: string) => {
  const cut = identity.lastIndexOf(":");
  return cut > identity.indexOf(":") ? identity.slice(0, cut) : identity;
};

class CallStore {
  roomId = $state<string | null>(null);
  roomName = $state("");
  /** The call is in a 1:1 chat (nobody else to ring into it). */
  isDm = $state(false);
  status = $state<Status | null>(null);
  error = $state<string | null>(null);
  micOn = $state(true);
  camOn = $state(false);
  /** Which camera we film with on a phone (front = mirrored in our own view). */
  facing = $state<"user" | "environment">("user");
  /** Cameras this device has (known once ours was turned on), for the flip button. */
  cameraCount = $state(0);
  /** A phone (front/back cameras by facing mode) or a computer with several cameras. Not the
   * Linux engine (its camera is picked in the voice settings). */
  get canFlip() {
    return this.camOn && !this.#nativeIn && (PHONE || this.cameraCount > 1);
  }
  screenOn = $state(false);
  /** Nobody heard (and our mic off), Discord style. */
  deafened = $state(false);
  /** Android picture-in-picture: the app is the small call window (UI = call tiles only). */
  pip = $state(false);
  #minimizedBeforePip = false;
  /** Phone: where call audio can go (Android 12+: earpiece, speaker, headsets) and where it
   * goes now. */
  audioRoutes = $state<AudioRoute[]>([]);
  audioCurrent = $state<number | null>(null);
  /** Android 11 and older (no route list): the loudspeaker on or off. */
  #speakerOn = $state(false);
  #knownRoutes = new Set<number>();
  get audioNow(): AudioRoute | null {
    // Listening mode: the headset still is where the call goes, from the user's side.
    const id = this.#released ?? this.#wanted ?? this.audioCurrent;
    return this.audioRoutes.find((r) => r.id === id) ?? null;
  }
  /** Call audio on the loudspeaker. */
  get speaker() {
    return this.audioNow ? this.audioNow.kind === "speaker" : this.#speakerOn;
  }
  /** A headset (Bluetooth, wired, USB) is connected: the speaker button offers a choice. */
  get hasHeadset() {
    return this.audioRoutes.some((r) => HEADSETS.includes(r.kind));
  }
  /** Mic state to restore when undeafening. */
  #micBeforeDeafen = true;
  /** Who else is in the call (LiveKit identities) and who shares a screen, as last heard: null
   * until we're connected, so the people already there make no sound. */
  #present: Set<string> | null = null;
  /** Voices boosted above 100 % (by track SID), see `#boostTrack`. */
  #boosted = new Map<string, { source: MediaStreamAudioSourceNode; gain: GainNode }>();
  #boostContext: AudioContext | null = null;
  #sharing = new Set<string>();
  prefs = $state<CallPrefs>(loadPrefs());
  /** Push-to-talk: the talk button / key is held. */
  talking = $state(false);
  /** Shrunk to a pill so the chats stay usable during the call. */
  minimized = $state(false);
  /** The call's chat is open beside it (computer). */
  chatOpen = $state(false);
  #roomy = new MediaQuery("min-width: 900px");
  /** Wide enough for the call panel under the chat list (CallDock) instead of the pill. */
  get docked() {
    return this.#roomy.current;
  }
  get canShareScreen() {
    return !isAndroid && (this.native || !!navigator.mediaDevices?.getDisplayMedia);
  }
  /** Call on the left, its chat on the right (ChatsScreen docks the chat pane there). */
  get sideChat() {
    return this.active && !this.minimized && this.chatOpen && this.#roomy.current;
  }

  /** Discord's chat button: beside the call on a computer; on a phone the call shrinks to its
   * pill (tap it to come back) and the chat opens. */
  toggleChat() {
    if (!this.roomId) return;
    if (this.#roomy.current) {
      this.chatOpen = !this.chatOpen;
      if (this.chatOpen) nav.openChat(this.roomId);
    } else {
      this.minimized = true;
      nav.openChat(this.roomId);
    }
  }
  tiles = $state<Tile[]>([]);
  /** The person menu (right-click / tap on someone in a call): at the pointer, or a sheet. */
  menu = $state<{ person: CallPerson; x: number; y: number; sheet: boolean } | null>(null);
  /** Profile opened from the person menu (shown over the chats, the call minimised). */
  profile = $state<string | null>(null);
  /** Someone is calling us. */
  incoming = $state<Ring | null>(null);
  /** We're calling someone in a 1:1 chat and they haven't joined yet. */
  calling = $state(false);
  /** When the call really started for us (connected, and in a 1:1 call answered), for the
   * duration shown in the call screen, dock and pill. */
  startedAt = $state<number | null>(null);
  #clock = $state(Date.now());
  #clockTimer: ReturnType<typeof setInterval> | undefined;
  /** "4:07" / "1:02:33" since the call started, or null before. */
  get duration(): string | null {
    if (this.startedAt === null) return null;
    const total = Math.max(0, Math.floor((this.#clock - this.startedAt) / 1000));
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const sec = String(total % 60).padStart(2, "0");
    return h ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
  }
  #startClock() {
    if (this.startedAt !== null) return;
    this.startedAt = this.#clock = Date.now();
    clearInterval(this.#clockTimer);
    this.#clockTimer = setInterval(() => (this.#clock = Date.now()), 1000);
    // A group call or voice channel counts from when it began (the oldest join among the
    // people in it), not from when we joined; a 1:1 call from the answer, like a phone call.
    const roomId = this.roomId;
    if (!roomId || this.isDm) return;
    const earlier = (at: number | null) => {
      // Only ever earlier: someone leaving never makes the time jump.
      if (at && this.roomId === roomId && this.startedAt !== null && at < this.startedAt) {
        this.startedAt = Math.max(at, Date.now() - 7 * 24 * 3600_000);
      }
    };
    void callStartedAt(roomId).then(earlier, () => {});
    // The others' memberships may only just have arrived.
    setTimeout(() => void callStartedAt(roomId).then(earlier, () => {}), 4000);
  }
  /** Their devices that said our ring rings there (AlwyChat only: other apps don't tell). */
  ringingDevices = $state<string[]>([]);
  /** People rung into this call, by user ID (until they join). */
  invites = $state<Record<string, Invite>>({});

  /** Who we're ringing and who hasn't joined yet: the other person of a 1:1 call we started,
   * and people rung in with "Ring someone". Shown as tiles in the call. */
  get waiting(): Waiting[] {
    const out: Waiting[] = [];
    const person = (userId: string) => this.people.find((m) => m.user_id === userId);
    if (this.calling) {
      for (const m of this.people) {
        if (m.is_me || m.user_id === chatList.userId) continue;
        out.push({ userId: m.user_id, name: m.name, avatar: m.avatar, status: this.ringingDevices.length ? "ringing" : "calling" });
      }
    }
    for (const [userId, invite] of Object.entries(this.invites)) {
      if (invite.status !== "calling" && invite.status !== "ringing") continue;
      if (out.some((w) => w.userId === userId)) continue;
      const m = person(userId);
      out.push({ userId, name: m?.name ?? userId.slice(1).split(":")[0], avatar: m?.avatar ?? null, status: invite.status });
    }
    return out;
  }
  #inviteTimers = new Map<string, ReturnType<typeof setTimeout>>();
  /** The "Ring someone" panel is open in the call view. */
  ringPanel = $state(false);
  /** The call room's members (for ringing someone in). */
  people = $state<Member[]>([]);
  #ringEnd: ReturnType<typeof setTimeout> | undefined;
  #noAnswer: ReturnType<typeof setTimeout> | undefined;
  #listening = false;
  /** Short message after a call ends on its own ("No answer", "Call declined"). */
  notice = $state<string | null>(null);

  #tell(message: string) {
    this.notice = message;
    setTimeout(() => this.notice === message && (this.notice = null), 4000);
  }

  /** Calls run in Rust (Linux desktop): no volume slider yet (mute for me only). */
  native = $state(false);
  /** Connected through the native engine. */
  #nativeIn = false;
  #nativeMembers: NativeCallMember[] = [];
  get #connected() {
    return !!this.#lk || this.#nativeIn;
  }

  /** Listen for rings and declines. Call once after sign-in. */
  listen() {
    if (this.#listening) return;
    this.#listening = true;
    void nativeCalls().then((native) => (this.native = native));
    if (isAndroid) {
      // The incoming call notification (the app wasn't on screen when it rang).
      void onCallLink((link) => void this.#link(link));
      void pipAllowed(this.prefs.pip).catch(() => {});
      void launchCall().then((link) => link && this.#link(link)).catch(() => {});
      // The ringtone waits while the app is in the background (the notification rings then).
      document.addEventListener("visibilitychange", () => {
        if (document.visibilityState === "visible" && this.incoming && !this.ringSilenced) startRinging();
      });
      void onCallAction((action) => {
        if (action === "hang_up") void this.leave();
        else if (action === "toggle_mic") {
          if (this.roomId) void this.toggleMic();
        } else if (action === "audio_routes") {
          void this.#routesChanged();
        } else if (action === "app_hidden") {
          void this.#cameraHidden(true);
        } else if (action === "app_shown") {
          void this.#cameraHidden(false);
        } else if (action === "stop_location") {
          // Handled by the live location store.
        } else if (action === "pip_on") {
          // Android's picture-in-picture: only the call, full window.
          this.#minimizedBeforePip = this.minimized;
          this.minimized = false;
          this.pip = true;
        } else {
          this.pip = false;
          if (this.roomId) this.minimized = this.#minimizedBeforePip;
        }
      });
      // The ongoing call notification follows the call: shown while in one, its mute button
      // following the mic (none with push to talk, where the mic opens and closes all the time).
      $effect.root(() => {
        $effect(() => {
          const roomId = this.roomId;
          if (!roomId) return;
          const muted = this.prefs.pushToTalk ? null : !this.micOn;
          const texts = {
            title: this.roomName,
            text: tr("Ongoing call · tap to return"),
            hangUp: tr("Hang up"),
            channel: tr("Calls"),
            mute: tr("Mute"),
            unmute: tr("Unmute"),
          };
          callNotification(true, texts, roomId, muted, this.camOn).catch((e) => console.warn("call notification", e));
        });
      });
    }
    // Media keys (end-to-end encryption) for the call we're in; the native engine gets them
    // from the core directly.
    void onCallKey((key) => {
      if (key.room_id === this.roomId) void this.#keys?.add(key).catch((e) => console.warn("call key", e));
    });
    void onCallSignal((signal) => {
      if (signal.kind === "ring") return this.#ring(signal);
      if (signal.room_id !== this.roomId) return;
      if (signal.kind === "hand") {
        const { [signal.user_id]: _, ...rest } = this.hands;
        this.hands = signal.raised ? { ...rest, [signal.user_id]: Date.now() } : rest;
        if (signal.raised) this.#cue("unmute");
        return;
      }
      if (signal.kind === "reaction") return this.#showReaction(signal.user_id, signal.emoji);
      const invite = this.invites[signal.user_id];
      if (signal.kind === "ringing") {
        if (this.calling && !this.ringingDevices.includes(signal.device_id)) this.ringingDevices = [...this.ringingDevices, signal.device_id];
        if (invite && invite.status !== "declined" && !invite.devices.includes(signal.device_id))
          this.#setInvite(signal.user_id, { status: "ringing", devices: [...invite.devices, signal.device_id] });
      } else if (this.calling) {
        void this.leave().then(() => this.#tell(tr("Call declined")));
      } else if (invite) {
        this.#setInvite(signal.user_id, { ...invite, status: "declined" });
      }
    });
  }

  #ring(ring: Ring, alert = true) {
    if (this.active || this.incoming) return;
    this.incoming = ring;
    this.ringSilenced = false;
    if (!isAndroid || document.visibilityState === "visible") startRinging();
    clearTimeout(this.#ringEnd);
    this.#ringEnd = setTimeout(() => this.stopRing(), Math.max(0, ring.expires_at - Date.now()));
    if (alert) this.#alert(ring);
  }

  /** Outside the window when the app isn't in front (hidden in the tray, behind other windows,
   * a background tab): the ringing screen alone would go unseen. */
  #alert(ring: Ring) {
    const room = chatList.rooms.find((r) => r.id === ring.room_id);
    const body =
      room && !room.is_dm
        ? tr("Calls you into {name}", { name: room.name })
        : ring.video
          ? tr("Incoming video call")
          : tr("Incoming call");
    if (!isWeb) return void callAlert(ring, ring.caller_name, body).catch((e) => console.warn("call alert", e));
    if (document.visibilityState === "visible" && document.hasFocus()) return;
    try {
      if (Notification.permission !== "granted") return;
      const n = new Notification(ring.caller_name, { body, tag: `call:${ring.room_id}`, requireInteraction: true });
      n.onclick = () => (window.focus(), n.close());
      setTimeout(() => n.close(), Math.max(0, ring.expires_at - Date.now()));
    } catch {
      /* no notifications in this browser */
    }
  }

  /** From the incoming call notification: Answer joins the call, a tap shows it ringing. */
  async #link(link: CallLink) {
    if (link.answer) {
      const started = Date.now();
      let room = chatList.rooms.find((r) => r.id === link.room_id);
      // Launched by the notification: the chat list may still be loading.
      while (!room && Date.now() - started < 10_000) {
        await new Promise((r) => setTimeout(r, 200));
        room = chatList.rooms.find((r) => r.id === link.room_id);
      }
      if (!room) return this.#tell(tr("Couldn't join the call"));
      this.stopRing();
      await this.join(room, link.video, false);
    } else if (link.expires_at > Date.now()) {
      this.#ring(
        {
          kind: "ring",
          room_id: link.room_id,
          notification_id: link.notification_id,
          caller_name: link.caller_name,
          caller_avatar: chatList.rooms.find((r) => r.id === link.room_id)?.avatar ?? null,
          video: link.video,
          expires_at: link.expires_at,
        },
        false,
      );
    }
  }

  /** Ring someone into this call (voice channel, group call): only their devices ring. */
  async ringPerson(userId: string) {
    if (!this.roomId) return;
    this.#setInvite(userId, { status: "calling", devices: [] });
    clearTimeout(this.#inviteTimers.get(userId));
    this.#inviteTimers.set(
      userId,
      setTimeout(() => {
        const invite = this.invites[userId];
        if (invite && (invite.status === "calling" || invite.status === "ringing")) this.#setInvite(userId, { ...invite, status: "no_answer" });
      }, INVITE_RING_MS),
    );
    try {
      await ringUser(userId);
    } catch (e) {
      console.warn("ring", e);
      this.#setInvite(userId, { status: "failed", devices: [] });
    }
  }

  #setInvite(userId: string, invite: Invite | null) {
    const next = { ...this.invites };
    if (invite) next[userId] = invite;
    else {
      delete next[userId];
      clearTimeout(this.#inviteTimers.get(userId));
      this.#inviteTimers.delete(userId);
    }
    this.invites = next;
  }

  // ── Raised hands and reactions (Element Call's events, so its users see them both ways) ──
  /** Raised hands in our call: user ID → when it went up (for the order). */
  hands = $state<Record<string, number>>({});
  /** Our own hand. */
  handUp = $state(false);
  /** Reactions showing now (3 s): user ID → emoji. */
  reactions = $state<Record<string, { emoji: string; at: number }>>({});

  async toggleHand() {
    const up = !this.handUp;
    const me = chatList.userId;
    const set = (on: boolean) => {
      this.handUp = on;
      if (!me) return;
      const { [me]: _, ...rest } = this.hands;
      this.hands = on ? { ...rest, [me]: Date.now() } : rest;
    };
    set(up);
    try {
      await raiseHand(up);
    } catch (e) {
      set(!up);
      console.warn("raise hand", e);
    }
  }

  react(emoji: string, name: string) {
    if (chatList.userId) this.#showReaction(chatList.userId, emoji);
    callReact(emoji, name).catch((e) => console.warn("call reaction", e));
  }

  #showReaction(userId: string, emoji: string) {
    const at = Date.now();
    if (this.prefs.reactionVolume >= 0.01 && !this.deafened) {
      void import("$lib/soundboard/reactions")
        .then(({ playReactionSound }) => playReactionSound(emoji, loudness(this.prefs.reactionVolume)))
        .catch((e) => console.warn("reaction sound", e));
    }
    this.reactions = { ...this.reactions, [userId]: { emoji, at } };
    setTimeout(() => {
      if (this.reactions[userId]?.at !== at) return;
      const { [userId]: _, ...rest } = this.reactions;
      this.reactions = rest;
    }, 3000);
  }

  /** The incoming call keeps showing, without sound or vibration ("Silence"). */
  ringSilenced = $state(false);
  silenceRing() {
    this.ringSilenced = true;
    stopRinging();
  }

  /** The ring ended without an answer here (timeout, caller hung up, answered elsewhere). */
  stopRing() {
    clearTimeout(this.#ringEnd);
    stopRinging();
    this.incoming = null;
  }

  async accept(room: RoomSummary) {
    const video = this.incoming?.video ?? false;
    this.stopRing();
    // A voice channel joins like a tap on it (in place under the chat list on a computer).
    await this.join(room, video, false, room.is_voice);
  }

  async decline() {
    const ring = this.incoming;
    this.stopRing();
    if (ring) await declineCall(ring.room_id, ring.notification_id).catch(() => {});
  }

  #lk: Room | null = null;
  /** livekit-client's VideoPreset (the engine loads lazily). */
  #VideoPreset: (typeof import("livekit-client"))["VideoPreset"] | null = null;
  /** Who is speaking by our own measure (LiveKit's comes from the server about twice a second,
   * smoothed: the ring lagged ~1 s behind the voice), by LiveKit identity. */
  #voices = new Map<string, { speaking: boolean; lastLoud: number }>();
  #meter: ReturnType<typeof setInterval> | undefined;
  #keys: KeyProvider | null = null;
  // Soundboard: a Web Audio context (also plays our own sounds to us), and in webview calls the
  // published track sounds are played into.
  #sbContext: AudioContext | null = null;
  #sbOut: MediaStreamAudioDestinationNode | null = null;
  #sbPlaying: AudioBufferSourceNode[] = [];
  /** Our own sound as we hear it (the volume slider changes it while it plays). */
  #sbGain: GainNode | null = null;
  /** Our media is end-to-end encrypted (encrypted room). */
  encrypted = $state(false);
  #members = new Map<string, Member>();
  /** Remote audio elements, kept out of the DOM tree of the call view. */
  #audio = new Map<string, HTMLMediaElement>();

  get active() {
    return this.roomId !== null;
  }

  /** `ringing`: we start it (1:1 chats ring the other person; the core sends the ring).
   * `inPlace`: voice channels on a computer connect without opening the call screen. */
  async join(room: RoomSummary, video: boolean, ringing = true, inPlace = false) {
    if (this.roomId === room.id) return void (this.minimized = false);
    if (this.incoming?.room_id === room.id) this.stopRing();
    await this.leave();
    // A first guess for "Calling…" (the core says for sure once it joined: the chat list's
    // copy of who's in the call lags right after a call).
    this.calling = ringing && room.is_dm && room.in_call.length === 0;
    if (this.calling) {
      this.#noAnswer = setTimeout(() => {
        if (this.calling && this.roomId === room.id) void this.leave().then(() => this.#tell(tr("No answer")));
      }, NO_ANSWER_MS);
    }
    // Fetched while the call is being set up with the server, so it adds no wait.
    if (!this.native) void loadEngine().catch(() => {});
    this.roomId = room.id;
    this.roomName = room.name;
    this.isDm = room.is_dm;
    this.status = "connecting";
    this.error = null;
    this.minimized = inPlace && this.docked;
    this.micOn = !this.prefs.pushToTalk;
    this.deafened = false;
    this.talking = false;
    this.camOn = video;
    this.screenOn = false;
    // Like phone apps: a call starts in the headset when one is connected, else on the
    // earpiece (voice channels and groups too), a video call on the loudspeaker.
    this.ringingDevices = [];
    this.people = [];
    if (isAndroid) void this.#autoRoute(video);
    roomMembers(room.id)
      .then((list) => {
        this.#members = new Map(list.map((m) => [m.user_id, m]));
        if (this.roomId === room.id) this.people = list;
        this.#sync();
      })
      .catch(() => {});

    try {
      const { url, jwt, encrypted, rang } = await joinCall(room.id, video);
      if (ringing && rang !== this.calling) {
        this.calling = rang;
        clearTimeout(this.#noAnswer);
        if (rang)
          this.#noAnswer = setTimeout(() => {
            if (this.calling && this.roomId === room.id) void this.leave().then(() => this.#tell(tr("No answer")));
          }, NO_ANSWER_MS);
      }
      if (this.native) {
        // Before awaiting: the first member snapshot comes while joining.
        this.#nativeIn = true;
        this.encrypted = encrypted;
        await nativeCallJoin(room.id, url, jwt, encrypted, this.micOn, this.#nativeSetup(), (event) => {
          if (event.kind === "members") {
            this.#nativeMembers = event.members;
            this.#sync();
          } else if (event.kind === "connected") this.#connectedNow();
          else if (event.kind === "reconnecting") this.status = "reconnecting";
          else if (event.kind === "screen_ended") this.screenOn = false;
          else if (this.#nativeIn) void this.leave();
        }, (key, jpeg) => nativeVideo.push(key, jpeg));
        this.#connectedNow();
        for (const userId of this.prefs.muted) void nativeCallSetMutedForMe(userId, true).catch(() => {});
        for (const [userId, volume] of Object.entries(this.prefs.volumes)) void nativeCallSetVolume(userId, loudness(volume)).catch(() => {});
        void nativeCallSetSoundboardVolume(loudness(this.prefs.soundboardVolume)).catch(() => {});
        if (this.prefs.soundboardVolume < 0.01) void nativeCallSetSoundboardOff(true).catch(() => {});
        if (video) await nativeCallSetCamera(true).catch((e) => this.#fail("camera", e));
        this.#sync();
        return;
      }
      const [{ Room, RoomEvent, ConnectionState, VideoPresets, ScreenSharePresets, VideoPreset }, { MatrixKeyProvider, e2eeWorker }] =
        await loadEngine();
      const keys = new MatrixKeyProvider();
      const worker = await e2eeWorker();
      const lk = new Room({
        adaptiveStream: true,
        dynacast: true,
        audioCaptureDefaults: {
          noiseSuppression: this.prefs.noiseSuppression,
          echoCancellation: this.prefs.echoCancellation,
          autoGainControl: this.prefs.autoGain,
          deviceId: this.prefs.inputDevice ?? undefined,
        },
        videoCaptureDefaults: { deviceId: this.prefs.cameraDevice ?? undefined, resolution: VideoPresets.h720.resolution },
        // Sharper than LiveKit's cautious defaults (same as the native engine).
        publishDefaults: {
          videoEncoding: { maxBitrate: 1_700_000, maxFramerate: 30 },
          screenShareEncoding: ScreenSharePresets.h1080fps15.encoding,
        },
        audioOutput: { deviceId: this.prefs.outputDevice ?? undefined },
        // Always there: whoever encrypts (Element Call, FluffyChat) is decrypted with their key,
        // even where we don't encrypt ourselves.
        e2ee: worker ? { keyProvider: keys, worker } : undefined,
      });
      this.#lk = lk;
      this.#VideoPreset = VideoPreset;
      this.#keys = keys;
      for (const key of await callKeys(room.id)) void keys.add(key).catch((e) => console.warn("call key", e));
      lk.on(RoomEvent.ParticipantConnected, () => this.#sync())
        .on(RoomEvent.ParticipantDisconnected, () => this.#sync())
        .on(RoomEvent.TrackSubscribed, (track, _publication, participant) => {
          this.#attachAudio(track);
          this.#applyVolume(participant);
          this.#sync();
        })
        .on(RoomEvent.TrackUnsubscribed, (track) => (this.#detachAudio(track), this.#sync()))
        .on(RoomEvent.TrackMuted, () => this.#sync())
        .on(RoomEvent.TrackUnmuted, () => this.#sync())
        .on(RoomEvent.LocalTrackPublished, () => this.#sync())
        .on(RoomEvent.LocalTrackUnpublished, () => this.#sync())
        .on(RoomEvent.ActiveSpeakersChanged, () => this.#sync())
        .on(RoomEvent.ConnectionQualityChanged, () => this.#sync())
        .on(RoomEvent.ConnectionStateChanged, (state) => {
          if (state === ConnectionState.Connected) this.#connectedNow();
          else if (state === ConnectionState.Reconnecting) this.status = "reconnecting";
        })
        .on(RoomEvent.Disconnected, () => {
          // Kicked or the connection is gone for good: clean up our membership too.
          if (this.#lk === lk) void this.leave();
        });
      await lk.connect(url, jwt);
      this.#connectedNow();
      // Before the microphone is published, so nothing goes out in clear.
      if (encrypted && worker) {
        await lk.setE2EEEnabled(true);
        this.encrypted = true;
      } else if (encrypted) console.warn("this WebView can't encrypt call media");
      // Push-to-talk starts silent (the mic is opened while talking).
      if (!this.prefs.pushToTalk) {
        await lk.localParticipant.setMicrophoneEnabled(true).catch((e) => this.#fail("microphone", e));
      }
      if (video) {
        this.facing = "user";
        await lk.localParticipant.setCameraEnabled(true).catch((e) => this.#fail("camera", e));
        void this.#countCameras();
      }
      await lk.startAudio().catch(() => {});
      this.#startMeter(lk);
      this.#sync();
    } catch (e) {
      const message = String(e);
      await this.leave();
      this.error = message;
      // The call view is gone by now: say why.
      this.#tell(tr("Couldn't join the call: {value}", { value: message.replace(/^Error: /, "") }));
    }
  }

  /** Play a soundboard sound (48 kHz mono) for everyone in the call, and to us. Its own track,
   * so the microphone's processing doesn't touch it; a new sound replaces the one playing. */
  async playSound(buffer: AudioBuffer) {
    if (!this.#connected || this.deafened) return;
    const ctx = (this.#sbContext ??= new AudioContext({ sampleRate: 48_000 }));
    await ctx.resume().catch(() => {});
    for (const node of this.#sbPlaying) node.stop();
    this.#sbPlaying = [];
    const local = ctx.createBufferSource();
    local.buffer = buffer;
    const volume = ctx.createGain();
    volume.gain.value = 0.7 * loudness(this.prefs.soundboardVolume);
    this.#sbGain = volume;
    local.connect(volume).connect(ctx.destination);
    local.start();
    this.#sbPlaying.push(local);
    if (this.#nativeIn) {
      const { pcm16 } = await import("./soundboard.svelte");
      const bytes = pcm16(buffer);
      let binary = "";
      for (let k = 0; k < bytes.length; k += 0x8000) binary += String.fromCharCode(...bytes.subarray(k, k + 0x8000));
      await nativeCallPlaySound(btoa(binary)).catch((e) => console.warn("soundboard", e));
      return;
    }
    const lk = this.#lk;
    if (!lk) return;
    if (!this.#sbOut) {
      this.#sbOut = ctx.createMediaStreamDestination();
      await lk.localParticipant
        .publishTrack(this.#sbOut.stream.getAudioTracks()[0], { name: "soundboard", source: Track.Source.Unknown, dtx: false, red: false })
        .catch((e) => console.warn("soundboard", e));
    }
    const out = ctx.createBufferSource();
    out.buffer = buffer;
    out.connect(this.#sbOut);
    out.start();
    this.#sbPlaying.push(out);
  }

  #cue(cue: CallCue) {
    playCue(cue, this.prefs.callSoundLevel);
  }

  /** Connected (first time, or back after reconnecting): our join sound the first time. */
  #connectedNow() {
    const first = this.status !== "connected" && this.#present === null;
    this.status = "connected";
    if (first) {
      // Whoever is already there is the starting point (no sound for them).
      this.#sync();
      const others = this.tiles.filter((t) => !t.isMe);
      this.#present = new Set(others.map((t) => t.id));
      this.#sharing = new Set(others.filter((t) => t.sharing).map((t) => t.id));
      this.#cue("join");
    }
  }

  async leave() {
    clearTimeout(this.#noAnswer);
    if (this.#present !== null) this.#cue("leave");
    this.#present = null;
    this.#sharing.clear();
    clearInterval(this.#meter);
    this.#meter = undefined;
    this.#voices.clear();
    for (const node of this.#sbPlaying) node.stop();
    this.#sbPlaying = [];
    this.#sbGain = null;
    this.#sbOut = null;
    void this.#sbContext?.close().catch(() => {});
    this.#sbContext = null;
    this.calling = false;
    clearInterval(this.#clockTimer);
    this.startedAt = null;
    this.#intent = null;
    this.showMyScreen = false;
    this.screenPending = false;
    this.screenStarting = false;
    this.screenSound = null;
    this.screenNote = null;
    this.screenPicker?.pick(null);
    void this.#win?.stop();
    this.#win = null;
    this.unwatched = new Set();
    this.screenSoundOff = new Set();
    this.screenVolumes = {};
    this.#cameraPaused = false;
    this.#endHiddenCam();
    this.ringingDevices = [];
    this.hands = {};
    this.handUp = false;
    this.reactions = {};
    for (const timer of this.#inviteTimers.values()) clearTimeout(timer);
    this.#inviteTimers.clear();
    this.invites = {};
    this.people = [];
    const lk = this.#lk;
    this.#lk = null;
    this.#keys = null;
    this.encrypted = false;
    const wasNative = this.#nativeIn;
    this.#nativeIn = false;
    this.#nativeMembers = [];
    nativeVideo.clear();
    const wasIn = this.roomId !== null;
    if (wasIn && isAndroid) {
      callNotification(false).catch(() => {});
      // Leaving the call audio mode re-routes the sound: let the leave sound finish first.
      setTimeout(() => {
        if (this.roomId === null) callAudio(null).catch(() => {});
      }, 1200);
    }
    this.audioRoutes = [];
    this.audioCurrent = null;
    this.#released = null;
    this.roomId = null;
    this.chatOpen = false;
    this.status = null;
    this.tiles = [];
    this.deafened = false;
    this.talking = false;
    for (const el of this.#audio.values()) el.remove();
    this.#audio.clear();
    this.#boosted.clear();
    void this.#boostContext?.close().catch(() => {});
    this.#boostContext = null;
    await lk?.disconnect().catch(() => {});
    if (wasNative || (wasIn && this.native)) await nativeCallLeave().catch(() => {});
    if (wasIn) await leaveCall().catch(() => {});
  }

  async toggleMic() {
    if (!this.#connected) return;
    // Unmuting while deafened undeafens too (as Discord does).
    if (this.deafened && !this.micOn) this.#setDeafened(false);
    this.#cue(this.micOn ? "mute" : "unmute");
    await this.#setMic(!this.micOn);
  }

  toggleSpeaker() {
    void this.#route(this.speaker ? (this.hasHeadset ? "headset" : "earpiece") : "speaker");
  }

  /** Send call audio to one of `audioRoutes`. */
  chooseAudio(route: AudioRoute) {
    void this.#route("device", route.id);
  }

  /** The route just asked for, shown at once: Android reports the switch ~0.4 s later (the
   *  speaker button lagged). Cleared when Android confirms it, or after 2 s. */
  #wanted = $state<number | null>(null);
  #wantedTimer: ReturnType<typeof setTimeout> | undefined;
  #want(id: number | null) {
    clearTimeout(this.#wantedTimer);
    this.#wanted = id;
    if (id !== null) this.#wantedTimer = setTimeout(() => (this.#wanted = null), 2000);
  }

  #applyRoutes(routes: AudioRoutes) {
    this.audioRoutes = routes.devices;
    this.audioCurrent = routes.current ?? null;
    if (this.#wanted !== null && this.audioCurrent === this.#wanted) this.#want(null);
    this.#knownRoutes = new Set(routes.devices.map((r) => r.id));
  }

  /** The built-in place we chose (or the user picked): when the call's audio starts, the
   * WebView moves it to the loudspeaker by itself (Chromium's audio thread), so a voice call
   * ended up on the speaker; that move is undone. */
  #intent: "speaker" | "earpiece" | null = null;
  async #route(route: "speaker" | "earpiece" | "headset" | "device", device?: number) {
    if (!isAndroid) return;
    this.#released = null;
    const picked = route === "device" ? this.audioRoutes.find((r) => r.id === device)?.kind : route;
    this.#intent = picked === "speaker" || picked === "earpiece" ? picked : null;
    if (route !== "device") this.#speakerOn = route === "speaker";
    const kinds = route === "headset" ? HEADSETS : [route === "device" ? "" : route];
    const match = route === "device" ? this.audioRoutes.find((r) => r.id === device) : kinds.map((k) => this.audioRoutes.find((r) => r.kind === k)).find(Boolean);
    this.#want(match?.id ?? null);
    try {
      this.#applyRoutes(await callAudio(route, device));
    } catch (e) {
      this.#want(null);
      console.warn("call audio", e);
    }
  }

  /** The default place: a connected headset, else the loudspeaker for video, the earpiece for
   * voice. */
  async #autoRoute(video: boolean) {
    const routes = await audioRoutes().catch(() => ({ devices: [] as AudioRoute[] }));
    const headset = routes.devices.some((r) => HEADSETS.includes(r.kind));
    await this.#route(headset ? "headset" : video ? "speaker" : "earpiece");
  }

  /** Android says headsets came or went, or it moved the audio itself. */
  async #routesChanged() {
    if (!isAndroid || !this.roomId) return;
    const before = this.#knownRoutes;
    const routes = await audioRoutes().catch(() => null);
    if (!routes || !this.roomId) return;
    if (this.#released !== null) {
      // Listening mode: nothing is the call device on purpose. If the headset went away,
      // forget it: unmuting then picks the default place.
      if (!routes.devices.some((r) => r.id === this.#released)) this.#released = null;
      this.audioRoutes = routes.devices;
      this.#knownRoutes = new Set(routes.devices.map((r) => r.id));
      return;
    }
    const added = routes.devices.find((r) => HEADSETS.includes(r.kind) && !before.has(r.id));
    const current = routes.devices.find((r) => r.id === routes.current);
    this.#applyRoutes(routes);
    // Earbuds just connected: the call goes into them (like phone calls).
    if (added) return void this.#route("device", added.id);
    // The headset in use went away: back to the default place.
    if (!current) return void this.#autoRoute(this.camOn);
    // Moved between the phone's own earpiece and speaker without us: put it back.
    if (this.#intent && current.kind !== this.#intent && (current.kind === "speaker" || current.kind === "earpiece")) void this.#route(this.#intent);
  }

  async toggleDeafen() {
    if (!this.#connected) return;
    const deaf = !this.deafened;
    this.#cue(deaf ? "deafen" : "undeafen");
    this.#setDeafened(deaf);
    if (deaf) {
      this.#micBeforeDeafen = this.micOn;
      if (this.micOn) await this.#setMic(false);
    } else if (this.#micBeforeDeafen && !this.prefs.pushToTalk) {
      await this.#setMic(true);
    }
  }

  #setDeafened(deaf: boolean) {
    this.deafened = deaf;
    for (const el of this.#audio.values()) el.muted = deaf;
    // Boosted voices play through their gain node: silence / restore those too.
    for (const p of this.#lk?.remoteParticipants.values() ?? []) this.#applyVolume(p);
    if (this.#nativeIn) void nativeCallSetDeafened(deaf).catch(() => {});
  }

  async #setMic(on: boolean) {
    if (!this.#connected) return;
    this.micOn = on;
    // Listening mode: the Bluetooth headset gets its call mode back before the mic opens.
    if (on && this.#released !== null) {
      const id = this.#released;
      this.#released = null;
      await this.#route("device", id);
    }
    const listen = !on && isAndroid && this.prefs.btListenMode && this.audioNow?.kind === "bluetooth";
    if (this.#nativeIn) await nativeCallSetMic(on).catch((e) => this.#fail("microphone", e));
    else {
      // Stopping the mic (not just muting it) is what lets the headset leave its call mode.
      const mic = this.#lk?.localParticipant.getTrackPublication(Track.Source.Microphone)?.track as LocalAudioTrack | undefined;
      if (mic) mic.stopOnMute = listen;
      await this.#lk?.localParticipant.setMicrophoneEnabled(on).catch((e) => this.#fail("microphone", e));
    }
    if (listen && !this.micOn && this.audioNow) {
      this.#released = this.audioNow.id;
      try {
        this.#applyRoutes(await callAudio("release"));
      } catch (e) {
        console.warn("call audio", e);
      }
    }
    this.#sync();
  }
  /** Listening mode: the Bluetooth headset we let go of while muted (its route id). */
  #released: number | null = null;

  setBtListenMode(on: boolean) {
    this.prefs = { ...this.prefs, btListenMode: on };
    this.#savePrefs();
  }
  setPip(on: boolean) {
    this.prefs = { ...this.prefs, pip: on };
    this.#savePrefs();
    void pipAllowed(on).catch(() => {});
  }
  setBtNames(on: boolean) {
    this.prefs = { ...this.prefs, btNames: on };
    this.#savePrefs();
  }

  /** Push-to-talk: hold to speak. */
  async talk(on: boolean) {
    if (!this.#connected || !this.prefs.pushToTalk || this.deafened || this.talking === on) return;
    this.talking = on;
    await this.#setMic(on);
  }

  #savePrefs() {
    localStorage.setItem(PREFS_KEY, JSON.stringify(this.prefs));
  }

  async setPushToTalk(on: boolean) {
    this.prefs = { ...this.prefs, pushToTalk: on };
    this.#savePrefs();
    this.talking = false;
    if (this.#connected && !this.deafened) await this.#setMic(!on);
  }

  async setNoiseSuppression(on: boolean) {
    await this.setPrefs({ noiseSuppression: on });
  }

  #nativeSetup(): NativeAudioSetup {
    const p = this.prefs;
    return {
      input: p.inputDevice,
      output: p.outputDevice,
      camera: p.cameraDevice,
      noise_suppression: p.noiseSuppression,
      echo_cancellation: p.echoCancellation,
      auto_gain: p.autoGain,
    };
  }

  /** Change voice & video preferences (Settings, call options), applied to a call in progress. */
  async setPrefs(change: Partial<CallPrefs>) {
    const before = this.prefs;
    this.prefs = { ...before, ...change };
    this.#savePrefs();
    if (this.#nativeIn) return void (await nativeCallSetSetup(this.#nativeSetup()).catch(() => {}));
    const lk = this.#lk;
    if (!lk) return;
    const p = this.prefs;
    if (p.inputDevice !== before.inputDevice) await lk.switchActiveDevice("audioinput", p.inputDevice ?? "default").catch(() => {});
    if (p.outputDevice !== before.outputDevice) await lk.switchActiveDevice("audiooutput", p.outputDevice ?? "default").catch(() => {});
    if (p.cameraDevice !== before.cameraDevice && this.camOn) await lk.switchActiveDevice("videoinput", p.cameraDevice ?? "default").catch(() => {});
    // Applied to the live microphone too (browsers that can't change it keep the old one).
    const mic = lk.localParticipant.getTrackPublication(Track.Source.Microphone)?.track;
    await mic?.mediaStreamTrack
      .applyConstraints({ noiseSuppression: p.noiseSuppression, echoCancellation: p.echoCancellation, autoGainControl: p.autoGain })
      .catch(() => {});
  }

  volumeOf(userId: string) {
    return this.prefs.volumes[userId] ?? 1;
  }

  setVolume(userId: string, volume: number) {
    const volumes = { ...this.prefs.volumes };
    if (Math.abs(volume - 1) < 0.005) delete volumes[userId];
    else volumes[userId] = volume;
    this.prefs = { ...this.prefs, volumes };
    this.#savePrefs();
    this.#applyVolumeOf(userId);
  }

  isMutedForMe(userId: string) {
    return this.prefs.muted.includes(userId);
  }

  toggleMuteForMe(userId: string) {
    const muted = this.isMutedForMe(userId)
      ? this.prefs.muted.filter((u) => u !== userId)
      : [...this.prefs.muted, userId];
    this.prefs = { ...this.prefs, muted };
    this.#savePrefs();
    this.#applyVolumeOf(userId);
  }

  #applyVolumeOf(userId: string) {
    if (this.#nativeIn) {
      const silent = this.isMutedForMe(userId) || this.volumeOf(userId) < 0.01;
      void nativeCallSetVolume(userId, loudness(this.volumeOf(userId))).catch(() => {});
      return void nativeCallSetMutedForMe(userId, silent).catch(() => {});
    }
    for (const p of this.#lk?.remoteParticipants.values() ?? []) {
      if (userOf(p.identity) === userId) this.#applyVolume(p);
    }
  }

  #applyVolume(participant: RemoteParticipant) {
    const userId = userOf(participant.identity);
    // Sliders are positions, not power: the same hearing curve as call sounds.
    const volume = this.isMutedForMe(userId) ? 0 : this.volumeOf(userId);
    participant.setVolume(loudness(Math.min(volume, 1)));
    const mic = participant.getTrackPublication(Track.Source.Microphone)?.track;
    if (mic?.sid) this.#boostTrack(mic.sid, mic.mediaStreamTrack, volume > 1 ? loudness(volume) : null);
    // Their soundboard: its own track (setVolume above only touches the microphone).
    for (const pub of participant.audioTrackPublications.values()) {
      if (pub.trackName === "soundboard" && pub.audioTrack) {
        (pub.audioTrack as RemoteAudioTrack).setVolume(this.isMutedForMe(userId) ? 0 : loudness(this.prefs.soundboardVolume));
      }
    }
  }

  /** How loud reaction sounds are for us (0 = none). */
  setReactionVolume(volume: number) {
    this.prefs = { ...this.prefs, reactionVolume: volume };
    this.#savePrefs();
  }
  #reactionBefore = 1;
  /** The speaker icon of the reactions panel: mute, or back to the volume it had. */
  toggleReactionMute() {
    if (this.prefs.reactionVolume >= 0.01) {
      this.#reactionBefore = this.prefs.reactionVolume;
      this.setReactionVolume(0);
    } else this.setReactionVolume(this.#reactionBefore >= 0.01 ? this.#reactionBefore : 1);
  }

  /** The soundboard volume before the speaker icon muted it (a second tap brings it back). */
  #soundboardBefore = 1;
  /** The speaker icon of the soundboard: mute, or back to the volume it had. */
  toggleSoundboardMute() {
    if (this.prefs.soundboardVolume >= 0.01) {
      this.#soundboardBefore = this.prefs.soundboardVolume;
      this.setSoundboardVolume(0);
    } else this.setSoundboardVolume(this.#soundboardBefore >= 0.01 ? this.#soundboardBefore : 1);
  }

  /** How loud soundboard sounds are for us (ours and others'), 0–1. Natively, others' can only
   * be on or off. */
  setSoundboardVolume(volume: number) {
    this.prefs = { ...this.prefs, soundboardVolume: volume };
    this.#savePrefs();
    if (this.#sbGain) this.#sbGain.gain.value = 0.7 * loudness(volume);
    if (this.#nativeIn) {
      void nativeCallSetSoundboardVolume(loudness(volume)).catch(() => {});
      return void nativeCallSetSoundboardOff(volume < 0.01).catch(() => {});
    }
    for (const p of this.#lk?.remoteParticipants.values() ?? []) this.#applyVolume(p);
  }

  /** Open the person menu: at the pointer with a mouse, as a bottom sheet on touch. */
  openMenu(person: CallPerson, event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    const touch = (event as PointerEvent).pointerType
      ? (event as PointerEvent).pointerType !== "mouse"
      : matchMedia("(pointer: coarse)").matches;
    this.menu = { person, x: event.clientX, y: event.clientY, sheet: touch };
  }

  async toggleCamera() {
    if (this.#nativeIn) {
      this.camOn = !this.camOn;
      return void (await nativeCallSetCamera(this.camOn).catch((e) => this.#fail("camera", e)));
    }
    if (!this.#lk) return;
    this.camOn = !this.camOn;
    // A camera turned back on starts with the front one, like the first time.
    if (this.camOn) this.facing = "user";
    await this.#lk.localParticipant.setCameraEnabled(this.camOn).catch((e) => this.#fail("camera", e));
    this.#sync();
    if (this.camOn) void this.#countCameras();
  }

  /** The camera was on when the app went out of sight (no picture-in-picture): Android freezes
   * it there, so it's stopped (the others see our photo, like WhatsApp's "video paused") and
   * started again when we're back. The camera button keeps showing it on. */
  #cameraPaused = false;
  async #pauseCamera(pause: boolean) {
    const lk = this.#lk;
    if (!lk) return;
    if (pause) {
      if (!this.camOn || this.#cameraPaused) return;
      this.#cameraPaused = true;
      await lk.localParticipant.setCameraEnabled(false).catch(() => {});
    } else {
      if (!this.#cameraPaused) return;
      this.#cameraPaused = false;
      if (!this.camOn) return;
      await lk.localParticipant
        .setCameraEnabled(true, { facingMode: this.facing })
        .catch((e) => this.#fail("camera", e));
    }
    this.#sync();
  }

  /** Out of sight (no picture-in-picture), the WebView stops handing camera frames to the
   * page. The app then opens the camera itself (BackgroundCamera.kt) and sends each frame as
   * I420 bytes over a message port; they're written into a generated track that replaces the
   * camera's in the published track until we're back, when the WebView's camera returns.
   * Where that isn't possible, the camera is paused instead (the others see our photo). */
  #hiddenCam: {
    port: MessagePort;
    writer: WritableStreamDefaultWriter<VideoFrame>;
    gen: MediaStreamTrack;
    sender: RTCRtpSender;
  } | null = null;
  /** The app is out of sight (the latest app_hidden / app_shown). */
  #appHidden = false;
  async #cameraHidden(hidden: boolean) {
    this.#appHidden = hidden;
    const lk = this.#lk;
    if (!lk) return;
    const track = lk.localParticipant.getTrackPublication(Track.Source.Camera)?.videoTrack as LocalVideoTrack | undefined;
    if (hidden) {
      const sender = track?.sender;
      if (!this.camOn || !track || !sender || this.#hiddenCam || this.#cameraPaused) return;
      const Generator = (globalThis as { MediaStreamTrackGenerator?: new (o: { kind: string }) => MediaStreamTrack & { writable: WritableStream<VideoFrame> } }).MediaStreamTrackGenerator;
      if (!Generator || typeof VideoFrame === "undefined") return this.#pauseCamera(true);
      const portReady = new Promise<MessagePort | null>((resolve) => {
        const timer = setTimeout(() => (window.removeEventListener("message", got), resolve(null)), 3000);
        function got(e: MessageEvent) {
          if (e.data !== "alwychat-camera" || !e.ports[0]) return;
          clearTimeout(timer);
          window.removeEventListener("message", got);
          resolve(e.ports[0]);
        }
        window.addEventListener("message", got);
      });
      const ok = await backgroundCamera(true, this.facing === "user").catch(() => false);
      const port = ok ? await portReady : null;
      // Back on screen meanwhile (a quick trip to another app): the WebView's camera is still
      // on the sender, so only the native camera has to go.
      if (!this.#appHidden) {
        port?.close();
        if (ok) void backgroundCamera(false, true).catch(() => {});
        return;
      }
      if (!port) {
        void backgroundCamera(false, true).catch(() => {});
        return this.#pauseCamera(true);
      }
      const gen = new Generator({ kind: "video" });
      const writer = gen.writable.getWriter();
      port.onmessage = (e) => {
        const data = e.data;
        if (!(data instanceof ArrayBuffer) || data.byteLength < 8) return;
        // A frame still being written: drop this one rather than queue up delay.
        if ((writer.desiredSize ?? 1) <= 0) return;
        const head = new DataView(data, 0, 8);
        const frame = new VideoFrame(new Uint8Array(data, 8), {
          format: "I420",
          codedWidth: head.getInt32(0, true),
          codedHeight: head.getInt32(4, true),
          timestamp: Math.round(performance.now() * 1000),
        });
        writer.write(frame).catch(() => frame.close());
      };
      this.#hiddenCam = { port, writer, gen, sender };
      // Swapped on the WebRTC sender itself: LiveKit keeps its camera track (and its encryption
      // and layer settings, which its own replaceTrack redoes: the others then got black frames).
      try {
        await sender.replaceTrack(gen);
      } catch (e) {
        console.warn("background camera", e);
        this.#endHiddenCam();
        return this.#pauseCamera(true);
      }
    } else {
      if (this.#cameraPaused) return this.#pauseCamera(false);
      const cam = this.#hiddenCam;
      if (!cam) return;
      void backgroundCamera(false, true).catch(() => {});
      // The camera track again: Chromium resumes it once we're on screen. If it ended meanwhile,
      // LiveKit opens the camera again (and puts it on the sender itself).
      try {
        if (track && track.mediaStreamTrack.readyState === "live") await cam.sender.replaceTrack(track.mediaStreamTrack);
        else if (track) await track.restartTrack({ facingMode: this.facing, resolution: { width: 1280, height: 720 } });
      } catch (e) {
        console.warn("camera back", e);
        await lk.localParticipant.setCameraEnabled(false).catch(() => {});
        if (this.camOn) await lk.localParticipant.setCameraEnabled(true, { facingMode: this.facing }).catch((err) => this.#fail("camera", err));
      }
      this.#endHiddenCam();
      this.#sync();
    }
  }

  #endHiddenCam() {
    const cam = this.#hiddenCam;
    if (!cam) return;
    this.#hiddenCam = null;
    cam.port.onmessage = null;
    cam.port.close();
    void cam.writer.close().catch(() => {});
    cam.gen.stop();
    void backgroundCamera(false, true).catch(() => {});
  }

  async #countCameras() {
    const devices = await navigator.mediaDevices?.enumerateDevices().catch(() => []);
    this.cameraCount = (devices ?? []).filter((d) => d.kind === "videoinput").length;
  }

  /** Front ↔ back camera on a phone; the next camera on a computer that has several. */
  async flipCamera() {
    const lk = this.#lk;
    if (!lk || !this.canFlip) return;
    const track = lk.localParticipant.getTrackPublication(Track.Source.Camera)?.videoTrack as LocalVideoTrack | undefined;
    if (!track) return;
    try {
      if (PHONE) {
        const next = this.facing === "user" ? "environment" : "user";
        await track.restartTrack({ facingMode: next, resolution: { width: 1280, height: 720, frameRate: 30 } });
        this.facing = next;
      } else {
        const cams = (await navigator.mediaDevices.enumerateDevices()).filter((d) => d.kind === "videoinput");
        const now = track.mediaStreamTrack.getSettings().deviceId;
        const next = cams[(cams.findIndex((c) => c.deviceId === now) + 1) % cams.length];
        if (next) await lk.switchActiveDevice("videoinput", next.deviceId);
      }
    } catch (e) {
      this.#fail("camera", e);
    }
    this.#sync();
  }

  /** Our own shared screen shown in the call (off by default: seeing it twice is useless). */
  showMyScreen = $state(false);
  /** Sharing asked, the desktop's picker still open (nothing is sent yet). */
  screenPending = $state(false);
  /** Picked (Windows app capture), waiting for the first picture and the server. */
  screenStarting = $state(false);
  /** Linux: the sound going with our shared screen, and the apps playing (null = none). */
  screenSound = $state<NativeScreenSound | null>(null);
  async refreshScreenSound() {
    if (this.#win) return this.#showWinSound();
    if (!this.#nativeIn || !this.screenOn) return void (this.screenSound = null);
    this.screenSound = await nativeCallScreenSound().catch(() => null);
    // A window share starts silent (GNOME doesn't say which app it is): in its first seconds,
    // and until a choice is made by hand, take the app picked last time if it's playing, else
    // the only app playing.
    const sound = this.screenSound;
    if (sound && sound.choice.kind === "none" && Date.now() < this.#soundAutoUntil) {
      const app = sound.apps.find((a) => a.id === this.prefs.screenApp) ?? (sound.apps.length === 1 ? sound.apps[0] : null);
      if (app) {
        this.#soundAutoUntil = 0;
        await this.setScreenSound({ kind: "app", app: app.id }, false);
      }
    }
  }
  /** Until when a window share may still pick its app's sound by itself (0 = not any more). */
  #soundAutoUntil = 0;
  async setScreenSound(choice: SoundChoice, byHand = true) {
    if (byHand) {
      this.#soundAutoUntil = 0;
      if (choice.kind === "app" && !this.#win) {
        this.prefs = { ...this.prefs, screenApp: choice.app };
        this.#savePrefs();
      }
    }
    if (this.#win) {
      this.#winSound = choice.kind;
      this.#showWinSound();
      return void (await winScreenSound(choice.kind).catch((e) => console.warn("screen sound", e)));
    }
    if (this.screenSound) this.screenSound = { ...this.screenSound, choice };
    await nativeCallSetScreenSound(choice).catch((e) => console.warn("screen sound", e));
  }

  /** Others' shares we don't watch (not received at all) / don't hear, by LiveKit identity;
   * forgotten when that share ends. */
  unwatched = $state(new Set<string>());
  screenSoundOff = $state(new Set<string>());
  /** Not watching a share silences it too; watching again brings its sound back. */
  watchScreen(identity: string, watch: boolean) {
    const next = new Set(this.unwatched);
    const sound = new Set(this.screenSoundOff);
    if (watch) next.delete(identity), sound.delete(identity);
    else next.add(identity), sound.add(identity);
    this.unwatched = next;
    this.screenSoundOff = sound;
    this.#applyScreen(identity);
  }
  /** How loud someone's screen sound is for us (slider position 0–1; forgotten with the share). */
  screenVolumes = $state<Record<string, number>>({});
  screenVolumeOf(identity: string) {
    return this.screenVolumes[identity] ?? 1;
  }
  setScreenVolume(identity: string, volume: number) {
    this.screenVolumes = { ...this.screenVolumes, [identity]: volume };
    // Moving the slider up un-mutes; down to 0 mutes.
    if (volume < 0.01 !== this.screenSoundOff.has(identity)) return this.setScreenSoundOff(identity, volume < 0.01);
    this.#applyScreen(identity);
  }
  setScreenSoundOff(identity: string, off: boolean) {
    if (!off && this.screenVolumeOf(identity) < 0.01) this.screenVolumes = { ...this.screenVolumes, [identity]: 1 };
    const next = new Set(this.screenSoundOff);
    if (off) next.add(identity);
    else next.delete(identity);
    this.screenSoundOff = next;
    this.#applyScreen(identity);
  }
  #applyScreen(identity: string) {
    const watch = !this.unwatched.has(identity);
    const sound = !this.screenSoundOff.has(identity);
    const volume = loudness(this.screenVolumeOf(identity));
    if (this.#nativeIn) return void nativeCallWatchScreen(identity, watch, sound, volume).catch((e) => console.warn("watch screen", e));
    const p = this.#lk?.remoteParticipants.get(identity);
    if (!p) return;
    p.setVolume(volume, Track.Source.ScreenShareAudio);
    p.getTrackPublication(Track.Source.ScreenShare)?.setSubscribed(watch);
    p.getTrackPublication(Track.Source.ScreenShareAudio)?.setSubscribed(sound);
    this.#sync();
  }
  setShowMyScreen(on: boolean) {
    this.showMyScreen = on;
    if (this.#nativeIn) void nativeCallScreenPreview(on).catch(() => {});
  }

  /** What a new share starts with (Settings → Voice & video). */
  setScreenDefault(choice: ScreenQuality | "last") {
    this.prefs = { ...this.prefs, screenDefault: choice };
    this.#savePrefs();
  }

  /** The codec setting as this engine can use it: no AV1 on Linux's engine (no backup copy). */
  #codec(): ScreenCodec {
    return this.native && this.prefs.screenCodec === "av1" ? "auto" : this.prefs.screenCodec;
  }

  /** A quality's settings with the bitrate this device's data and codec choices give. */
  #screenQ(quality: ScreenQuality) {
    const q = SCREEN_QUALITY[quality] ?? SCREEN_QUALITY.balanced;
    const factor = (DATA_FACTOR[this.prefs.screenData] ?? 1) * CODEC_FACTOR(this.#codec());
    return { ...q, maxBitrate: Math.round(q.maxBitrate * factor) };
  }

  /** The codec the webview publishes a screen with (AV1 keeps VP8 as backup for viewers that
   * can't decode it); Auto = H.264 in the Windows app (the graphics card encodes it there). */
  #screenCodecOptions(): { videoCodec: "av1" | "h264" | "vp8"; backupCodec?: { codec: "vp8" } | false } {
    const c = this.prefs.screenCodec;
    if (c === "av1") return { videoCodec: "av1", backupCodec: { codec: "vp8" } };
    if (c === "h264" || (c === "auto" && isWindows)) return { videoCodec: "h264", backupCodec: false };
    return { videoCodec: "vp8", backupCodec: false };
  }

  setScreenEncoding(change: Partial<Pick<CallPrefs, "screenCodec" | "screenData">>) {
    this.prefs = { ...this.prefs, ...change };
    this.#savePrefs();
  }

  /** What our share sends right now (shown on its card while sharing). */
  screenStats = $state<ScreenStats | null>(null);
  #statsPolling = false;
  async #pollScreenStats() {
    if (this.#statsPolling || !this.screenOn) return;
    this.#statsPolling = true;
    while (this.screenOn) {
      await new Promise((r) => setTimeout(r, 2000));
      if (!this.screenOn) break;
      this.screenStats = await this.#readScreenStats().catch(() => null);
    }
    this.screenStats = null;
    this.#statsPolling = false;
  }
  async #readScreenStats(): Promise<ScreenStats | null> {
    if (this.#nativeIn) return nativeCallScreenStats();
    const pub = this.#lk?.localParticipant.getTrackPublication(Track.Source.ScreenShare);
    const sender = (pub?.videoTrack as LocalVideoTrack | undefined)?.sender;
    if (!sender) return null;
    let best: ScreenStats | null = null;
    let codecs = new Map<string, string>();
    const report = await sender.getStats();
    report.forEach((r) => r.type === "codec" && codecs.set(r.id, String(r.mimeType ?? "").replace("video/", "")));
    report.forEach((r) => {
      if (r.type !== "outbound-rtp" || (best && (r.frameWidth ?? 0) < best.width)) return;
      const codec = codecs.get(r.codecId) ?? "";
      best = {
        encoder: [codec, r.encoderImplementation, r.powerEfficientEncoder ? "GPU" : ""].filter(Boolean).join(" · "),
        fps: r.framesPerSecond ?? 0,
        width: r.frameWidth ?? 0,
        height: r.frameHeight ?? 0,
        limit: r.qualityLimitationReason ?? "none",
        mbps: (r.targetBitrate ?? 0) / 1e6,
      };
    });
    codecs = new Map();
    return best;
  }

  /** The quality of the share going on (remembered as the last used): changed live. */
  async setScreenQuality(quality: ScreenQuality) {
    this.prefs = { ...this.prefs, screenQuality: quality };
    this.#savePrefs();
    if (!this.screenOn) return;
    const q = this.#screenQ(quality);
    if (this.#win) void winScreenQuality(q).catch((e) => console.warn("screen quality", e));
    if (this.#nativeIn) return void (await nativeCallScreenQuality(q).catch((e) => this.#fail("screen sharing", e)));
    const track = this.#lk?.localParticipant.getTrackPublication(Track.Source.ScreenShare)?.videoTrack as LocalVideoTrack | undefined;
    if (!track) return;
    const fluid = q.fluid;
    const media = track.mediaStreamTrack;
    media.contentHint = fluid ? "motion" : "detail";
    await media.applyConstraints({ width: { max: q.width }, height: { max: q.height }, frameRate: { max: q.fps } }).catch((e) => console.warn("screen constraints", e));
    const sender = track.sender;
    if (!sender) return;
    const params = sender.getParameters();
    for (const enc of params.encodings) {
      // The backup layer (shrunk) gets a third of the bitrate, like when it was published.
      const backup = (enc.scaleResolutionDownBy ?? 1) > 1;
      enc.maxFramerate = q.fps;
      enc.maxBitrate = backup ? Math.round(Math.max(300_000, q.maxBitrate / 3)) : q.maxBitrate;
    }
    params.degradationPreference = fluid ? "maintain-framerate" : "maintain-resolution";
    await sender.setParameters(params).catch((e) => console.warn("screen encoding", e));
  }

  /** Start sharing (the default quality from the settings) or stop. */
  async toggleScreen() {
    this.error = null;
    if (!this.screenOn && this.prefs.screenDefault !== "last") this.prefs = { ...this.prefs, screenQuality: this.prefs.screenDefault };
    const q = this.#screenQ(this.prefs.screenQuality);
    if (this.#nativeIn) {
      // The desktop asks which screen or window (Wayland: the portal dialog); nothing is sent
      // before something is chosen. Closing the picker = "cancelled", not an error.
      this.screenOn = !this.screenOn;
      this.screenSound = null;
      if (!this.screenOn) {
        this.screenPending = false;
        return void (await nativeCallSetScreen(false).catch((e) => this.#fail("screen sharing", e)));
      }
      void nativeCallScreenPreview(this.showMyScreen).catch(() => {});
      this.screenPending = true;
      try {
        // Published once with room for the most demanding choice (60 fps 1080p); the capture's
        // rate and size follow the chosen quality, live.
        await nativeCallSetScreen(true, { ...q, maxBitrate: this.#screenQ("speed").maxBitrate }, this.#codec());
        this.#soundAutoUntil = Date.now() + 15_000;
        void this.#pollScreenStats();
        void this.refreshScreenSound();
      } catch (e) {
        if (String(e).includes("cancelled")) this.screenOn = false;
        else this.#fail("screen sharing", e);
      } finally {
        this.screenPending = false;
      }
      return;
    }
    if (!this.#lk) return;
    // Windows app: our own picker and capture, which follows an app across its windows.
    if (isWindows && winShareSupported()) return this.screenOn ? this.#stopWinShare() : this.#startWinShare(q);
    this.screenOn = !this.screenOn;
    const fluid = SCREEN_QUALITY[this.prefs.screenQuality]?.fluid ?? true;
    await this.#lk.localParticipant
      .setScreenShareEnabled(
        this.screenOn,
        // "motion": the browser keeps the frame rate and lowers detail when it must (games,
        // videos); "detail" keeps text sharp and drops frames instead.
        {
          // The shared sound never includes this app's own playback (the others' voices):
          // `restrictOwnAudio` (Chromium); browsers that don't know it ignore it.
          audio: { restrictOwnAudio: true, echoCancellation: false, noiseSuppression: false, autoGainControl: false } as AudioCaptureOptions,
          resolution: { width: q.width, height: q.height, frameRate: q.fps },
          contentHint: fluid ? "motion" : "detail",
        },
        {
          screenShareEncoding: { maxBitrate: q.maxBitrate, maxFramerate: q.fps },
          // The backup layer at the same frame rate (LiveKit's default runs it at 3 fps).
          screenShareSimulcastLayers: this.#VideoPreset
            ? [new this.#VideoPreset(q.width / 2, q.height / 2, Math.round(Math.max(300_000, q.maxBitrate / 3)), q.fps)]
            : undefined,
          degradationPreference: fluid ? "maintain-framerate" : "maintain-resolution",
          ...this.#screenCodecOptions(),
        },
      )
      .catch((e) => this.#fail("screen sharing", e));
    this.#sync();
  }

  // ── Windows: app-side screen capture (stores/winShare.ts) ──
  /** Our screen/window picker while open. */
  screenPicker = $state<{ sources: WinSource[]; pick: (s: WinSource | null) => void } | null>(null);
  /** What the share is doing ("Sharing League of Legends", "Waiting for … to come back"). */
  screenNote = $state<string | null>(null);
  #win: WinShare | null = null;
  #winTracks: LocalTrack[] = [];
  #winSound: WinSound = "all";
  #winApp = "";

  async refreshScreenSources() {
    if (!this.screenPicker) return;
    const sources = await winScreenSources().catch(() => null);
    if (sources && this.screenPicker) this.screenPicker = { ...this.screenPicker, sources };
  }

  /** The card's sound menu: everything but AlwyChat, nothing, or the shared app. */
  #showWinSound() {
    const choice: SoundChoice = this.#winSound === "app" ? { kind: "app", app: "shared" } : { kind: this.#winSound };
    this.screenSound = { choice, apps: this.#winApp ? [{ id: "shared", name: this.#winApp }] : [] };
  }

  async #startWinShare(q: (typeof SCREEN_QUALITY)[ScreenQuality]) {
    const sources = await winScreenSources().catch((e) => (this.#fail("screen sharing", e), null));
    if (!sources) return;
    const source = await new Promise<WinSource | null>((pick) => (this.screenPicker = { sources, pick }));
    this.screenPicker = null;
    const lk = this.#lk;
    if (!source || !lk) return;
    // A window carries its own app's sound; a whole screen every app's but ours.
    this.#winSound = source.kind === "window" ? "app" : "all";
    this.#winApp = source.app;
    const share = new WinShare();
    this.#win = share;
    this.screenOn = true;
    this.screenStarting = true;
    this.#showWinSound();
    try {
      await share.start(source.id, q, this.#winSound, (e) => this.#winEvent(share, e));
      let timer: ReturnType<typeof setTimeout> | undefined;
      await Promise.race([share.firstFrame, new Promise((_, fail) => (timer = setTimeout(() => fail(new Error("no picture came")), 10_000)))]).finally(() =>
        clearTimeout(timer),
      );
      if (this.#win !== share) return;
      const fluid = SCREEN_QUALITY[this.prefs.screenQuality]?.fluid ?? true;
      share.video.contentHint = fluid ? "motion" : "detail";
      const encoding = { maxBitrate: q.maxBitrate, maxFramerate: q.fps };
      const video = await lk.localParticipant.publishTrack(share.video, {
        source: Track.Source.ScreenShare,
        name: "screen",
        videoEncoding: encoding,
        screenShareEncoding: encoding,
        screenShareSimulcastLayers: this.#VideoPreset ? [new this.#VideoPreset(q.width / 2, q.height / 2, Math.round(Math.max(300_000, q.maxBitrate / 3)), q.fps)] : undefined,
        degradationPreference: fluid ? "maintain-framerate" : "maintain-resolution",
        ...this.#screenCodecOptions(),
      });
      const audio = await lk.localParticipant.publishTrack(share.audio, {
        source: Track.Source.ScreenShareAudio,
        name: "screen-audio",
        dtx: false,
        red: false,
        audioPreset: { maxBitrate: 128_000 },
      });
      this.#winTracks = [video.track, audio.track].filter((t): t is LocalTrack => !!t);
    } catch (e) {
      await this.#stopWinShare();
      this.#fail("screen sharing", e);
    } finally {
      this.screenStarting = false;
      this.#sync();
    }
  }

  #winEvent(share: WinShare, e: WinShareEvent) {
    if (this.#win !== share) return;
    if (e.kind === "source") {
      this.#winApp = e.app;
      this.screenNote = e.app ? tr("Sharing {app}", { app: e.app }) : null;
      this.#showWinSound();
    } else if (e.kind === "waiting") {
      this.screenNote = tr("Waiting for {app} to come back…", { app: this.#winApp });
    } else if (e.kind === "ended") {
      void this.#stopWinShare();
      this.#showError(tr("Screen sharing stopped: {reason}", { reason: e.reason }));
    }
  }

  async #stopWinShare() {
    const share = this.#win;
    this.#win = null;
    this.screenOn = false;
    this.screenPending = false;
    this.screenStarting = false;
    this.screenSound = null;
    this.screenNote = null;
    const lk = this.#lk;
    for (const t of this.#winTracks) await lk?.localParticipant.unpublishTrack(t, true).catch(() => {});
    this.#winTracks = [];
    await share?.stop();
    this.#sync();
  }

  #fail(what: string, e: unknown) {
    if (what === "microphone") this.micOn = false;
    if (what === "camera") this.camOn = false;
    if (what === "screen sharing") this.screenOn = false;
    const name = (e as { name?: string } | null)?.name;
    // Closing the screen picker without choosing anything isn't an error.
    if (what === "screen sharing" && (name === "NotAllowedError" || name === "AbortError")) return;
    this.#showError(
      name === "NotAllowedError"
        ? tr("AlwyChat isn't allowed to use the {what}", { what: what })
        : tr("Couldn't start the {what}: {e}", { what: what, e: e }),
    );
  }

  /** A problem with the call's devices: shown for a moment, it doesn't stay for the whole call. */
  #errorTimer: ReturnType<typeof setTimeout> | undefined;
  #showError(message: string) {
    this.error = message;
    clearTimeout(this.#errorTimer);
    this.#errorTimer = setTimeout(() => (this.error = null), 8000);
  }

  /** Measure everyone's voice 10 times a second from WebRTC's audio levels (our microphone's
   * source, each remote microphone's inbound stream), like the Linux engine does. */
  #startMeter(lk: Room) {
    const LEVEL = 0.03; // WebRTC audioLevel (0–1) that counts as speaking
    const HOLD = 400; // ms the ring stays after the voice drops, so it doesn't flicker
    let busy = false;
    const levelOf = async (track: LkTrack | undefined, type: "media-source" | "inbound-rtp") => {
      const report = await track?.getRTCStatsReport().catch(() => undefined);
      let level = 0;
      report?.forEach((s: { type?: string; kind?: string; audioLevel?: number }) => {
        if (s.type === type && s.kind === "audio" && typeof s.audioLevel === "number") level = Math.max(level, s.audioLevel);
      });
      return level;
    };
    clearInterval(this.#meter); // a reconnect starts it again
    this.#meter = setInterval(async () => {
      if (busy || this.#lk !== lk) return;
      busy = true;
      try {
        const levels: [string, number][] = [];
        const mine = lk.localParticipant.getTrackPublication(Track.Source.Microphone);
        levels.push([lk.localParticipant.identity, mine && !mine.isMuted ? await levelOf(mine.track, "media-source") : 0]);
        for (const p of lk.remoteParticipants.values()) {
          const mic = p.getTrackPublication(Track.Source.Microphone);
          levels.push([p.identity, mic && !mic.isMuted ? await levelOf(mic.track, "inbound-rtp") : 0]);
        }
        const now = performance.now();
        let changed = false;
        for (const [id, level] of levels) {
          const voice = this.#voices.get(id) ?? { speaking: false, lastLoud: -Infinity };
          if (level >= LEVEL) voice.lastLoud = now;
          const speaking = now - voice.lastLoud < HOLD;
          if (speaking !== voice.speaking) changed = true;
          voice.speaking = speaking;
          this.#voices.set(id, voice);
        }
        if (changed && this.#lk === lk) this.#sync();
      } finally {
        busy = false;
      }
    }, 100);
  }

  /** Above 100 %: an audio element can't go louder than what was sent, so that person's voice
   * plays through a gain node instead (their element stays attached but silent). `gain` null =
   * back to the element. */
  #boostTrack(sid: string, media: MediaStreamTrack, gain: number | null) {
    const el = this.#audio.get(sid);
    const now = this.#boosted.get(sid);
    if (gain === null) {
      if (now) {
        now.source.disconnect();
        now.gain.disconnect();
        this.#boosted.delete(sid);
      }
      if (el) el.muted = this.deafened;
      return;
    }
    if (!el) return;
    try {
      const ctx = (this.#boostContext ??= new AudioContext());
      void ctx.resume().catch(() => {});
      let node = now;
      if (!node) {
        const source = ctx.createMediaStreamSource(new MediaStream([media]));
        const out = ctx.createGain();
        source.connect(out).connect(ctx.destination);
        node = { source, gain: out };
        this.#boosted.set(sid, node);
      }
      node.gain.gain.value = this.deafened ? 0 : gain;
      el.muted = true;
    } catch (e) {
      console.warn("volume boost", e);
    }
  }

  #attachAudio(track: RemoteTrack) {
    if (track.kind !== Track.Kind.Audio || !track.sid) return;
    const el = track.attach();
    el.style.display = "none";
    el.muted = this.deafened;
    document.body.append(el);
    this.#audio.set(track.sid, el);
  }

  #detachAudio(track: RemoteTrack) {
    if (!track.sid) return;
    this.#boostTrack(track.sid, track.mediaStreamTrack, null);
    track.detach().forEach((el) => el.remove());
    this.#audio.get(track.sid)?.remove();
    this.#audio.delete(track.sid);
  }

  #tile(p: Participant, isMe: boolean): Tile {
    const userId = userOf(p.identity);
    const member = this.#members.get(userId);
    const camera = p.getTrackPublication(Track.Source.Camera);
    const screen = p.getTrackPublication(Track.Source.ScreenShare);
    return {
      id: p.identity,
      userId,
      name: member?.name ?? (p.name || userId.slice(1).split(":")[0]),
      avatar: member?.avatar ?? null,
      isMe,
      speaking: this.#voices.get(p.identity)?.speaking ?? p.isSpeaking,
      micOn: p.isMicrophoneEnabled,
      camera: camera?.track && !camera.isMuted ? camera.track : null,
      screen: screen?.track && !screen.isMuted ? screen.track : null,
      cameraFrames: null,
      screenFrames: null,
      sharing: !!screen && !screen.isMuted,
      lost: !isMe && (p.connectionQuality as string) === "lost",
    };
  }

  #nativeTile(m: NativeCallMember): Tile {
    const userId = userOf(m.identity);
    const member = this.#members.get(userId);
    return {
      id: m.identity,
      userId,
      name: member?.name ?? userId.slice(1).split(":")[0],
      avatar: member?.avatar ?? null,
      isMe: m.is_local,
      speaking: m.speaking,
      micOn: m.is_local ? this.micOn : m.mic_on,
      camera: null,
      screen: null,
      cameraFrames: m.camera,
      screenFrames: m.screen,
      sharing: m.sharing ?? !!m.screen,
      lost: !m.is_local && m.lost,
    };
  }

  #sync() {
    this.#syncMembers();
    if (this.screenOn) void this.#pollScreenStats();
    if (this.status === "connected" && !this.calling) this.#startClock();
  }

  #syncMembers() {
    if (this.#nativeIn) {
      if (this.calling && this.#nativeMembers.length > 1) {
        this.calling = false;
        clearTimeout(this.#noAnswer);
      }
      this.tiles = this.#nativeMembers.map((m) => this.#nativeTile(m));
      this.#heard();
      return;
    }
    const lk = this.#lk;
    if (!lk) return;
    // Someone joined: we're no longer just calling.
    if (this.calling && lk.remoteParticipants.size > 0) {
      this.calling = false;
      clearTimeout(this.#noAnswer);
    }
    this.tiles = [
      this.#tile(lk.localParticipant, true),
      ...[...lk.remoteParticipants.values()].map((p) => this.#tile(p, false)),
    ];
    this.#heard();
  }

  /** Sounds for people coming and going, and for a screen share starting. */
  #heard() {
    for (const t of this.tiles) if (!t.isMe && this.invites[t.userId]) this.#setInvite(t.userId, null);
    if (this.#present === null) return;
    const others = this.tiles.filter((t) => !t.isMe);
    const now = new Set(others.map((t) => t.id));
    const sharing = new Set(others.filter((t) => t.sharing).map((t) => t.id));
    // A share that ended takes our "not watching" / "no sound" choice with it.
    const gone = (set: Set<string>) => [...set].filter((id) => !sharing.has(id));
    if (gone(this.unwatched).length) this.unwatched = new Set([...this.unwatched].filter((id) => sharing.has(id)));
    if (gone(this.screenSoundOff).length) this.screenSoundOff = new Set([...this.screenSoundOff].filter((id) => sharing.has(id)));
    if (Object.keys(this.screenVolumes).some((id) => !sharing.has(id)))
      this.screenVolumes = Object.fromEntries(Object.entries(this.screenVolumes).filter(([id]) => sharing.has(id)));
    const joined = [...now].some((id) => !this.#present!.has(id));
    const left = [...this.#present].some((id) => !now.has(id));
    const shared = [...sharing].some((id) => !this.#sharing.has(id));
    this.#present = now;
    this.#sharing = sharing;
    if (joined) this.#cue("peerJoin");
    else if (left) this.#cue("peerLeave");
    // Someone who left takes their raised hand with them.
    if (left) {
      const present = new Set(this.tiles.map((t) => t.userId));
      const kept = Object.fromEntries(Object.entries(this.hands).filter(([user]) => present.has(user)));
      if (Object.keys(kept).length !== Object.keys(this.hands).length) this.hands = kept;
    }
    if (shared) this.#cue("screen");
    // A 1:1 call ends when the other person hangs up, at once (like phone apps): LiveKit only
    // says someone left when they did (a lost connection waits for a timeout first). While we
    // are reconnecting ourselves, a short grace period instead.
    if (left && now.size === 0 && this.isDm) {
      const roomId = this.roomId;
      const end = () => {
        if (this.roomId === roomId && this.#present?.size === 0) void this.leave().then(() => this.#tell(tr("Call ended")));
      };
      if (this.status === "connected") end();
      else setTimeout(end, 4000);
    }
  }
}

export const call = new CallStore();

// Dev builds only: lets the desktop test harness (WebDriver) drive the call view.
if (import.meta.env.DEV) (globalThis as Record<string, unknown>).__call = call;
