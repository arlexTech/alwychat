// The call we're in: MatrixRTC signalling in the core, audio/video with livekit-client here,
// or in Rust where the webview has no WebRTC (Linux desktop: `native`; pictures arrive as frames,
// see nativeVideo.svelte.ts).
import {
  ConnectionState,
  Room,
  RoomEvent,
  ScreenSharePresets,
  Track,
  VideoPresets,
  type Participant,
  type RemoteAudioTrack,
  type RemoteParticipant,
  type RemoteTrack,
} from "livekit-client";
import {
  isAndroid,
  launchCall,
  onCallLink,
  type CallLink,
  callAudio,
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
  type CallSignal,
  type Member,
  type RoomSummary,
} from "$lib/backend";
import { MediaQuery } from "svelte/reactivity";
import { startRinging, stopRinging } from "$lib/ringtone";
import { nav } from "./nav.svelte";
import { chatList } from "./chatList.svelte";
import { nativeVideo } from "./nativeVideo.svelte";
import { tr } from "$lib/i18n/index.svelte";
import { MatrixKeyProvider, e2eeWorker } from "$lib/callKeys";

type Ring = Extract<CallSignal, { kind: "ring" }>;
/** A 1:1 call nobody answers ends after this. */
const NO_ANSWER_MS = 45_000;

export interface Tile {
  /** LiveKit identity: "@user:server:DEVICE". */
  id: string;
  userId: string;
  name: string;
  avatar: string | null;
  isMe: boolean;
  speaking: boolean;
  micOn: boolean;
  camera: Track | null;
  screen: Track | null;
  /** Native calls: frame keys of the camera / screen (drawn by NativeVideo.svelte). */
  cameraFrames: string | null;
  screenFrames: string | null;
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
}

/** Someone in a call or a voice channel, as the person menu shows them. */
export interface CallPerson {
  userId: string;
  name: string;
  avatar: string | null;
  isMe: boolean;
  /** We're in that call with them: volume and mute apply. */
  live: boolean;
}
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
  };
  try {
    return { ...defaults, ...JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}") };
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
  status = $state<Status | null>(null);
  error = $state<string | null>(null);
  micOn = $state(true);
  camOn = $state(false);
  screenOn = $state(false);
  /** Nobody heard (and our mic off), Discord style. */
  deafened = $state(false);
  /** Android picture-in-picture: the app is the small call window (UI = call tiles only). */
  pip = $state(false);
  #minimizedBeforePip = false;
  /** Phone: call audio on the loudspeaker (else earpiece, or headphones when connected). */
  speaker = $state(false);
  /** Mic state to restore when undeafening. */
  #micBeforeDeafen = true;
  prefs = $state<CallPrefs>(loadPrefs());
  /** Push-to-talk: the talk button / key is held. */
  talking = $state(false);
  /** Shrunk to a pill so the chats stay usable during the call. */
  minimized = $state(false);
  /** The call's chat is open beside it (computer). */
  chatOpen = $state(false);
  #roomy = new MediaQuery("min-width: 900px");
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
      void launchCall().then((link) => link && this.#link(link)).catch(() => {});
      // The ringtone waits while the app is in the background (the notification rings then).
      document.addEventListener("visibilitychange", () => {
        if (document.visibilityState === "visible" && this.incoming) startRinging();
      });
      void onCallAction((action) => {
        if (action === "hang_up") void this.leave();
        else if (action === "toggle_mic") {
          if (this.roomId) void this.toggleMic();
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
      if (signal.kind === "ring") this.#ring(signal);
      else if (signal.room_id === this.roomId && this.calling) {
        void this.leave().then(() => this.#tell(tr("Call declined")));
      }
    });
  }

  #ring(ring: Ring) {
    if (this.active || this.incoming) return;
    this.incoming = ring;
    if (!isAndroid || document.visibilityState === "visible") startRinging();
    clearTimeout(this.#ringEnd);
    this.#ringEnd = setTimeout(() => this.stopRing(), Math.max(0, ring.expires_at - Date.now()));
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
      this.#ring({
        kind: "ring",
        room_id: link.room_id,
        notification_id: link.notification_id,
        caller_name: link.caller_name,
        caller_avatar: chatList.rooms.find((r) => r.id === link.room_id)?.avatar ?? null,
        video: link.video,
        expires_at: link.expires_at,
      });
    }
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
    await this.join(room, video, false);
  }

  async decline() {
    const ring = this.incoming;
    this.stopRing();
    if (ring) await declineCall(ring.room_id, ring.notification_id).catch(() => {});
  }

  #lk: Room | null = null;
  /** Who is speaking by our own measure (LiveKit's comes from the server about twice a second,
   * smoothed: the ring lagged ~1 s behind the voice), by LiveKit identity. */
  #voices = new Map<string, { speaking: boolean; lastLoud: number }>();
  #meter: ReturnType<typeof setInterval> | undefined;
  #keys: MatrixKeyProvider | null = null;
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

  /** `ringing`: we start it (1:1 chats ring the other person; the core sends the ring). */
  async join(room: RoomSummary, video: boolean, ringing = true) {
    if (this.roomId === room.id) return void (this.minimized = false);
    if (this.incoming?.room_id === room.id) this.stopRing();
    await this.leave();
    this.calling = ringing && room.is_dm && room.in_call.length === 0;
    if (this.calling) {
      this.#noAnswer = setTimeout(() => {
        if (this.calling && this.roomId === room.id) void this.leave().then(() => this.#tell(tr("No answer")));
      }, NO_ANSWER_MS);
    }
    this.roomId = room.id;
    this.roomName = room.name;
    this.status = "connecting";
    this.error = null;
    this.minimized = false;
    this.micOn = !this.prefs.pushToTalk;
    this.deafened = false;
    this.talking = false;
    this.camOn = video;
    this.screenOn = false;
    // Like phone apps: a call starts on the earpiece (voice channels and groups too), a video
    // call on the loudspeaker.
    this.speaker = video;
    if (isAndroid) callAudio(this.speaker).catch((e) => console.warn("call audio", e));
    roomMembers(room.id)
      .then((list) => {
        this.#members = new Map(list.map((m) => [m.user_id, m]));
        this.#sync();
      })
      .catch(() => {});

    try {
      const { url, jwt, encrypted } = await joinCall(room.id, video);
      if (this.native) {
        // Before awaiting: the first member snapshot comes while joining.
        this.#nativeIn = true;
        this.encrypted = encrypted;
        await nativeCallJoin(room.id, url, jwt, encrypted, this.micOn, this.#nativeSetup(), (event) => {
          if (event.kind === "members") {
            this.#nativeMembers = event.members;
            this.#sync();
          } else if (event.kind === "connected") this.status = "connected";
          else if (event.kind === "reconnecting") this.status = "reconnecting";
          else if (event.kind === "screen_ended") this.screenOn = false;
          else if (this.#nativeIn) void this.leave();
        }, (key, jpeg) => nativeVideo.push(key, jpeg));
        this.status = "connected";
        for (const userId of this.prefs.muted) void nativeCallSetMutedForMe(userId, true).catch(() => {});
        if (this.prefs.soundboardVolume < 0.01) void nativeCallSetSoundboardOff(true).catch(() => {});
        if (video) await nativeCallSetCamera(true).catch((e) => this.#fail("camera", e));
        this.#sync();
        return;
      }
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
        .on(RoomEvent.ConnectionStateChanged, (state) => {
          if (state === ConnectionState.Connected) this.status = "connected";
          else if (state === ConnectionState.Reconnecting) this.status = "reconnecting";
        })
        .on(RoomEvent.Disconnected, () => {
          // Kicked or the connection is gone for good: clean up our membership too.
          if (this.#lk === lk) void this.leave();
        });
      await lk.connect(url, jwt);
      this.status = "connected";
      // Before the microphone is published, so nothing goes out in clear.
      if (encrypted && worker) {
        await lk.setE2EEEnabled(true);
        this.encrypted = true;
      } else if (encrypted) console.warn("this WebView can't encrypt call media");
      // Push-to-talk starts silent (the mic is opened while talking).
      if (!this.prefs.pushToTalk) {
        await lk.localParticipant.setMicrophoneEnabled(true).catch((e) => this.#fail("microphone", e));
      }
      if (video) await lk.localParticipant.setCameraEnabled(true).catch((e) => this.#fail("camera", e));
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
    volume.gain.value = 0.7 * this.prefs.soundboardVolume;
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

  async leave() {
    clearTimeout(this.#noAnswer);
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
      callAudio(null).catch(() => {});
    }
    this.roomId = null;
    this.chatOpen = false;
    this.status = null;
    this.tiles = [];
    this.deafened = false;
    this.talking = false;
    for (const el of this.#audio.values()) el.remove();
    this.#audio.clear();
    await lk?.disconnect().catch(() => {});
    if (wasNative || (wasIn && this.native)) await nativeCallLeave().catch(() => {});
    if (wasIn) await leaveCall().catch(() => {});
  }

  async toggleMic() {
    if (!this.#connected) return;
    // Unmuting while deafened undeafens too (as Discord does).
    if (this.deafened && !this.micOn) this.#setDeafened(false);
    await this.#setMic(!this.micOn);
  }

  toggleSpeaker() {
    this.speaker = !this.speaker;
    callAudio(this.speaker).catch((e) => console.warn("call audio", e));
  }

  async toggleDeafen() {
    if (!this.#connected) return;
    const deaf = !this.deafened;
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
    if (this.#nativeIn) void nativeCallSetDeafened(deaf).catch(() => {});
  }

  async #setMic(on: boolean) {
    if (!this.#connected) return;
    this.micOn = on;
    if (this.#nativeIn) await nativeCallSetMic(on).catch((e) => this.#fail("microphone", e));
    else await this.#lk?.localParticipant.setMicrophoneEnabled(on).catch((e) => this.#fail("microphone", e));
    this.#sync();
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
    if (volume >= 0.999) delete volumes[userId];
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
    // Natively only on/off for now (the platform audio module mixes remote tracks itself).
    if (this.#nativeIn) {
      const silent = this.isMutedForMe(userId) || this.volumeOf(userId) < 0.01;
      return void nativeCallSetMutedForMe(userId, silent).catch(() => {});
    }
    for (const p of this.#lk?.remoteParticipants.values() ?? []) {
      if (userOf(p.identity) === userId) this.#applyVolume(p);
    }
  }

  #applyVolume(participant: RemoteParticipant) {
    const userId = userOf(participant.identity);
    participant.setVolume(this.isMutedForMe(userId) ? 0 : this.volumeOf(userId));
    // Their soundboard: its own track (setVolume above only touches the microphone).
    for (const pub of participant.audioTrackPublications.values()) {
      if (pub.trackName === "soundboard" && pub.audioTrack) {
        (pub.audioTrack as RemoteAudioTrack).setVolume(this.isMutedForMe(userId) ? 0 : this.prefs.soundboardVolume);
      }
    }
  }

  /** How loud soundboard sounds are for us (ours and others'), 0–1. Natively, others' can only
   * be on or off. */
  setSoundboardVolume(volume: number) {
    this.prefs = { ...this.prefs, soundboardVolume: volume };
    this.#savePrefs();
    if (this.#sbGain) this.#sbGain.gain.value = 0.7 * volume;
    if (this.#nativeIn) return void nativeCallSetSoundboardOff(volume < 0.01).catch(() => {});
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
    await this.#lk.localParticipant.setCameraEnabled(this.camOn).catch((e) => this.#fail("camera", e));
    this.#sync();
  }

  async toggleScreen() {
    if (this.#nativeIn) {
      // The desktop asks which screen or window (Wayland: the portal dialog).
      this.screenOn = !this.screenOn;
      return void (await nativeCallSetScreen(this.screenOn).catch((e) => this.#fail("screen sharing", e)));
    }
    if (!this.#lk) return;
    this.screenOn = !this.screenOn;
    await this.#lk.localParticipant
      .setScreenShareEnabled(this.screenOn, { audio: true })
      .catch((e) => this.#fail("screen sharing", e));
    this.#sync();
  }

  #fail(what: string, e: unknown) {
    if (what === "microphone") this.micOn = false;
    if (what === "camera") this.camOn = false;
    if (what === "screen sharing") this.screenOn = false;
    this.error = e instanceof DOMException && e.name === "NotAllowedError"
      ? tr("AlwyChat isn't allowed to use the {what}", { what: what })
      : tr("Couldn't start the {what}: {e}", { what: what, e: e });
  }

  /** Measure everyone's voice 10 times a second from WebRTC's audio levels (our microphone's
   * source, each remote microphone's inbound stream), like the Linux engine does. */
  #startMeter(lk: Room) {
    const LEVEL = 0.03; // WebRTC audioLevel (0–1) that counts as speaking
    const HOLD = 400; // ms the ring stays after the voice drops, so it doesn't flicker
    let busy = false;
    const levelOf = async (track: Track | undefined, type: "media-source" | "inbound-rtp") => {
      const report = await track?.getRTCStatsReport().catch(() => undefined);
      let level = 0;
      report?.forEach((s: { type?: string; kind?: string; audioLevel?: number }) => {
        if (s.type === type && s.kind === "audio" && typeof s.audioLevel === "number") level = Math.max(level, s.audioLevel);
      });
      return level;
    };
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
    };
  }

  #sync() {
    if (this.#nativeIn) {
      if (this.calling && this.#nativeMembers.length > 1) {
        this.calling = false;
        clearTimeout(this.#noAnswer);
      }
      this.tiles = this.#nativeMembers.map((m) => this.#nativeTile(m));
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
  }
}

export const call = new CallStore();

// Dev builds only: lets the desktop test harness (WebDriver) drive the call view.
if (import.meta.env.DEV) (globalThis as Record<string, unknown>).__call = call;
