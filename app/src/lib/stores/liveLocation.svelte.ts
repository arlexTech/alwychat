// Our live location shares: follow the position while AlwyChat runs and send it to each chat
// (Element-compatible beacons), until the chosen time is up or "Stop".
import { startLiveLocation, stopLiveLocation, updateLiveLocation } from "$lib/backend";
import { tr } from "$lib/i18n/index.svelte";

/** Don't send more often than this, unless we moved a lot. */
const MIN_INTERVAL_MS = 15_000;
const MIN_MOVE_M = 25;
const KEY = "alwychat.liveLocation";

/** "14:30", "tomorrow 14:30" or a date: when a share ends. */
export function untilTime(at: number) {
  const time = new Date(at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const day = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((day(new Date(at)) - day(new Date())) / 86_400_000);
  return days === 0 ? time : days === 1 ? tr("tomorrow {time}", { time }) : `${new Date(at).toLocaleDateString()} ${time}`;
}

type Share = { roomId: string; shareId: string; endsAt: number };

export const geoUri = (lat: number, lon: number, accuracy?: number | null) =>
  `geo:${lat.toFixed(6)},${lon.toFixed(6)}${accuracy ? `;u=${Math.round(accuracy)}` : ""}`;

function distance(a: GeolocationCoordinates, b: GeolocationCoordinates) {
  const r = 6_371_000;
  const dLat = ((b.latitude - a.latitude) * Math.PI) / 180;
  const dLon = ((b.longitude - a.longitude) * Math.PI) / 180;
  const h =
    Math.sin(dLat / 2) ** 2 +
    Math.cos((a.latitude * Math.PI) / 180) * Math.cos((b.latitude * Math.PI) / 180) * Math.sin(dLon / 2) ** 2;
  return 2 * r * Math.asin(Math.sqrt(h));
}

class LiveLocation {
  shares = $state<Share[]>(JSON.parse(localStorage.getItem(KEY) ?? "[]").filter((s: Share) => s.shareId && s.endsAt > Date.now()));
  error = $state<string | null>(null);
  #watch: number | null = null;
  #last: { at: number; coords: GeolocationCoordinates } | null = null;
  #endTimer: ReturnType<typeof setInterval> | undefined;

  /** Resume shares still running from before a restart. */
  start() {
    if (this.shares.length) this.#follow();
  }

  sharing(roomId: string) {
    return this.shares.find((s) => s.roomId === roomId) ?? null;
  }

  async share(roomId: string, durationMs: number, first: GeolocationCoordinates) {
    const shareId = await startLiveLocation(roomId, durationMs, geoUri(first.latitude, first.longitude, first.accuracy));
    this.shares = [...this.shares.filter((s) => s.roomId !== roomId), { roomId, shareId, endsAt: Date.now() + durationMs }];
    this.#last = { at: Date.now(), coords: first };
    this.#save();
    this.#follow();
  }

  async stop(roomId: string) {
    this.shares = this.shares.filter((s) => s.roomId !== roomId);
    this.#save();
    if (!this.shares.length) this.#unfollow();
    await stopLiveLocation(roomId).catch((e) => (this.error = String(e)));
  }

  #save() {
    localStorage.setItem(KEY, JSON.stringify(this.shares));
  }

  #follow() {
    if (this.#watch !== null || !navigator.geolocation) return;
    this.#watch = navigator.geolocation.watchPosition(
      (pos) => void this.#moved(pos.coords),
      (e) => (this.error = e.code === e.PERMISSION_DENIED ? tr("Location access was turned off") : e.message),
      { enableHighAccuracy: true, maximumAge: 10_000 },
    );
    clearInterval(this.#endTimer);
    this.#endTimer = setInterval(() => {
      const now = Date.now();
      for (const s of this.shares.filter((s) => s.endsAt <= now)) void this.stop(s.roomId);
    }, 10_000);
  }

  #unfollow() {
    if (this.#watch !== null) navigator.geolocation.clearWatch(this.#watch);
    this.#watch = null;
    clearInterval(this.#endTimer);
  }

  async #moved(coords: GeolocationCoordinates) {
    const now = Date.now();
    const last = this.#last;
    if (last && now - last.at < MIN_INTERVAL_MS && distance(last.coords, coords) < MIN_MOVE_M) return;
    this.#last = { at: now, coords };
    const uri = geoUri(coords.latitude, coords.longitude, coords.accuracy);
    for (const s of this.shares) {
      if (s.endsAt > now) await updateLiveLocation(s.roomId, s.shareId, uri).catch((e) => (this.error = String(e)));
    }
  }
}

export const liveLocation = new LiveLocation();
