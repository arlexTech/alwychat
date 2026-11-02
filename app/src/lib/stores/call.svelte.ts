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
  type RemoteParticipant,
  type RemoteTrack,
} from "livekit-client";
import {
  declineCall,
  joinCall,
  leaveCall,
  nativeCallJoin,
  nativeCallLeave,
  nativeCalls,
  nativeCallSetDeafened,
  nativeCallSetMic,
  nativeCallSetMutedForMe,
  nativeCallSetSetup,
  nativeCallSetCamera,
  nativeCallSetScreen,
  onCallSignal,
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
import { nativeVideo } from "./nativeVideo.svelte";
import { tr } from "$lib/i18n/index.svelte";

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
    startRinging();
    clearTimeout(this.#ringEnd);
    this.#ringEnd = setTimeout(() => this.stopRing(), Math.max(0, ring.expires_at - Date.now()));
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
    roomMembers(room.id)
      .then((list) => {
        this.#members = new Map(list.map((m) => [m.user_id, m]));
        this.#sync();
      })
      .catch(() => {});

    try {
      const { url, jwt } = await joinCall(room.id, video);
      if (this.native) {
        // Before awaiting: the first member snapshot comes while joining.
        this.#nativeIn = true;
        await nativeCallJoin(url, jwt, this.micOn, this.#nativeSetup(), (event) => {
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
        if (video) await nativeCallSetCamera(true).catch((e) => this.#fail("camera", e));
        this.#sync();
        return;
      }
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
      });
      this.#lk = lk;
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
      // Push-to-talk starts silent (the mic is opened while talking).
      if (!this.prefs.pushToTalk) {
        await lk.localParticipant.setMicrophoneEnabled(true).catch((e) => this.#fail("microphone", e));
      }
      if (video) await lk.localParticipant.setCameraEnabled(true).catch((e) => this.#fail("camera", e));
      await lk.startAudio().catch(() => {});
      this.#sync();
    } catch (e) {
      const message = String(e);
      await this.leave();
      this.error = message;
      // The call view is gone by now: say why.
      this.#tell(tr("Couldn't join the call: {value}", { value: message.replace(/^Error: /, "") }));
    }
  }

  async leave() {
    clearTimeout(this.#noAnswer);
    this.calling = false;
    const lk = this.#lk;
    this.#lk = null;
    const wasNative = this.#nativeIn;
    this.#nativeIn = false;
    this.#nativeMembers = [];
    nativeVideo.clear();
    const wasIn = this.roomId !== null;
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
      speaking: p.isSpeaking,
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
