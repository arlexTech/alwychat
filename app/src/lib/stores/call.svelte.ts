// The call we're in: MatrixRTC signalling in the core, audio/video with livekit-client here.
import {
  ConnectionState,
  Room,
  RoomEvent,
  Track,
  type Participant,
  type RemoteTrack,
} from "livekit-client";
import { joinCall, leaveCall, roomMembers, type Member, type RoomSummary } from "$lib/backend";

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
}

type Status = "connecting" | "connected" | "reconnecting";

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
  /** Shrunk to a pill so the chats stay usable during the call. */
  minimized = $state(false);
  tiles = $state<Tile[]>([]);

  #lk: Room | null = null;
  #members = new Map<string, Member>();
  /** Remote audio elements, kept out of the DOM tree of the call view. */
  #audio = new Map<string, HTMLMediaElement>();

  get active() {
    return this.roomId !== null;
  }

  async join(room: RoomSummary, video: boolean) {
    if (this.roomId === room.id) return void (this.minimized = false);
    await this.leave();
    this.roomId = room.id;
    this.roomName = room.name;
    this.status = "connecting";
    this.error = null;
    this.minimized = false;
    this.micOn = true;
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
      const lk = new Room({ adaptiveStream: true, dynacast: true });
      this.#lk = lk;
      lk.on(RoomEvent.ParticipantConnected, () => this.#sync())
        .on(RoomEvent.ParticipantDisconnected, () => this.#sync())
        .on(RoomEvent.TrackSubscribed, (track) => (this.#attachAudio(track), this.#sync()))
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
      await lk.localParticipant.setMicrophoneEnabled(true).catch((e) => this.#fail("microphone", e));
      if (video) await lk.localParticipant.setCameraEnabled(true).catch((e) => this.#fail("camera", e));
      await lk.startAudio().catch(() => {});
      this.#sync();
    } catch (e) {
      const message = String(e);
      await this.leave();
      this.error = message;
    }
  }

  async leave() {
    const lk = this.#lk;
    this.#lk = null;
    const wasIn = this.roomId !== null;
    this.roomId = null;
    this.status = null;
    this.tiles = [];
    for (const el of this.#audio.values()) el.remove();
    this.#audio.clear();
    await lk?.disconnect().catch(() => {});
    if (wasIn) await leaveCall().catch(() => {});
  }

  async toggleMic() {
    if (!this.#lk) return;
    this.micOn = !this.micOn;
    await this.#lk.localParticipant.setMicrophoneEnabled(this.micOn).catch((e) => this.#fail("microphone", e));
    this.#sync();
  }

  async toggleCamera() {
    if (!this.#lk) return;
    this.camOn = !this.camOn;
    await this.#lk.localParticipant.setCameraEnabled(this.camOn).catch((e) => this.#fail("camera", e));
    this.#sync();
  }

  async toggleScreen() {
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
      ? `AlwyChat isn't allowed to use the ${what}`
      : `Couldn't start the ${what}: ${e}`;
  }

  #attachAudio(track: RemoteTrack) {
    if (track.kind !== Track.Kind.Audio || !track.sid) return;
    const el = track.attach();
    el.style.display = "none";
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
    };
  }

  #sync() {
    const lk = this.#lk;
    if (!lk) return;
    this.tiles = [
      this.#tile(lk.localParticipant, true),
      ...[...lk.remoteParticipants.values()].map((p) => this.#tile(p, false)),
    ];
  }
}

export const call = new CallStore();
