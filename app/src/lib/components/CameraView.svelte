<script lang="ts">
  // The in-app camera (Messenger / Snapchat style): tap the shutter for a photo, hold it to film
  // (up to a minute, sound included), flip between cameras, torch where the phone has one.
  // The shot is shown for a second look, then waits in the composer like a picked photo.
  import { tr } from "$lib/i18n/index.svelte";
  import { onBack } from "$lib/back.svelte";
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import X from "@lucide/svelte/icons/x";
  import SwitchCamera from "@lucide/svelte/icons/switch-camera";
  import Zap from "@lucide/svelte/icons/zap";
  import ZapOff from "@lucide/svelte/icons/zap-off";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Check from "@lucide/svelte/icons/check";

  let {
    onShot,
    onClose,
    onFallback,
  }: {
    /** The photo (JPEG) or video taken, with a file name. */
    onShot: (blob: Blob, name: string) => void;
    onClose: () => void;
    /** The system camera app instead (shown when this one can't start). */
    onFallback?: () => void;
  } = $props();

  /** Holding the shutter longer than this films. */
  const HOLD_MS = 280;
  const MAX_VIDEO_MS = 60_000;
  /** Filming shorter than this was meant as a tap: a photo instead. */
  const MIN_VIDEO_MS = 700;

  let video = $state<HTMLVideoElement>();
  let facing = $state<"user" | "environment">("environment");
  let stream: MediaStream | null = null;
  let error = $state<string | null>(null);
  let torchAvailable = $state(false);
  let torch = $state(false);
  let flash = $state(false);
  let recording = $state(false);
  /** 0–1 of the longest video. */
  let progress = $state(0);
  let shot = $state<{ blob: Blob; url: string; name: string; isVideo: boolean } | null>(null);

  let recorder: MediaRecorder | null = null;
  let mic: MediaStream | null = null;
  let holdTimer: ReturnType<typeof setTimeout> | undefined;
  let pressed = false;
  /** Filming is starting (asking for the microphone). */
  let starting = false;
  let recordStart = 0;
  /** The recording being stopped is thrown away (too short). */
  let discard = false;

  $effect(() => onBack(() => (shot ? retake() : onClose())));

  function stopStream() {
    for (const track of stream?.getTracks() ?? []) track.stop();
    stream = null;
  }

  async function start() {
    stopStream();
    error = null;
    torch = false;
    try {
      stream = await navigator.mediaDevices.getUserMedia({
        video: { facingMode: facing, width: { ideal: 1920 }, height: { ideal: 1080 } },
        audio: false,
      });
      if (video) video.srcObject = stream;
      const track = stream.getVideoTracks()[0];
      const caps = (track?.getCapabilities?.() ?? {}) as MediaTrackCapabilities & { torch?: boolean };
      torchAvailable = !!caps.torch;
    } catch (e) {
      error = (e as DOMException)?.name === "NotAllowedError" ? tr("The camera isn't allowed for AlwyChat") : tr("The camera couldn't start");
      console.warn("camera", e);
    }
  }

  onMount(() => {
    void start();
    return () => {
      clearTimeout(holdTimer);
      if (recorder?.state === "recording") recorder.stop();
      for (const track of mic?.getTracks() ?? []) track.stop();
      stopStream();
      if (shot) URL.revokeObjectURL(shot.url);
    };
  });

  async function flip() {
    facing = facing === "user" ? "environment" : "user";
    await start();
  }

  async function toggleTorch() {
    const track = stream?.getVideoTracks()[0];
    if (!track) return;
    try {
      await track.applyConstraints({ advanced: [{ torch: !torch } as MediaTrackConstraintSet] });
      torch = !torch;
    } catch (e) {
      console.warn("torch", e);
    }
  }

  /** Local date and time for file names, as phone cameras do: 20261103_234405. */
  function stamp() {
    const d = new Date();
    const two = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}${two(d.getMonth() + 1)}${two(d.getDate())}_${two(d.getHours())}${two(d.getMinutes())}${two(d.getSeconds())}`;
  }

  function takePhoto() {
    if (!video?.videoWidth) return;
    const canvas = document.createElement("canvas");
    canvas.width = video.videoWidth;
    canvas.height = video.videoHeight;
    canvas.getContext("2d")!.drawImage(video, 0, 0);
    flash = true;
    setTimeout(() => (flash = false), 120);
    canvas.toBlob(
      (blob) => {
        if (blob) show(blob, `IMG_${stamp()}.jpg`, false);
      },
      "image/jpeg",
      0.92,
    );
  }

  /** MP4 first: iPhones (and FluffyChat there) can't play WebM. */
  function videoType() {
    for (const type of ["video/mp4;codecs=avc1,mp4a", "video/mp4", "video/webm;codecs=vp9,opus", "video/webm"]) {
      if (MediaRecorder.isTypeSupported(type)) return type;
    }
    return "";
  }

  async function startRecording() {
    if (!stream) return;
    starting = true;
    // The microphone only while filming (no mic light while just looking).
    mic = await navigator.mediaDevices
      .getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true } })
      .catch(() => null);
    starting = false;
    // Let go while the mic was being asked for: a photo after all.
    if (!pressed || !stream) {
      for (const track of mic?.getTracks() ?? []) track.stop();
      mic = null;
      if (stream) takePhoto();
      return;
    }
    const type = videoType();
    const tracks = [...stream.getVideoTracks(), ...(mic?.getAudioTracks() ?? [])];
    const chunks: Blob[] = [];
    recorder = new MediaRecorder(new MediaStream(tracks), { mimeType: type || undefined, videoBitsPerSecond: 2_500_000 });
    recorder.ondataavailable = (e) => e.data.size && chunks.push(e.data);
    recorder.onstop = () => {
      recording = false;
      for (const track of mic?.getTracks() ?? []) track.stop();
      mic = null;
      if (discard) return;
      const mime = (recorder?.mimeType || type || "video/webm").split(";")[0];
      const blob = new Blob(chunks, { type: mime });
      if (blob.size) show(blob, `VID_${stamp()}.${mime === "video/mp4" ? "mp4" : "webm"}`, true);
    };
    discard = false;
    recorder.start(1000);
    recording = true;
    const started = performance.now();
    recordStart = started;
    const tick = () => {
      if (!recording) return;
      progress = Math.min(1, (performance.now() - started) / MAX_VIDEO_MS);
      if (progress >= 1) stopRecording();
      else requestAnimationFrame(tick);
    };
    progress = 0;
    requestAnimationFrame(tick);
  }

  function stopRecording() {
    if (recorder?.state === "recording") recorder.stop();
  }

  function press(event: PointerEvent) {
    if (shot || error) return;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    pressed = true;
    holdTimer = setTimeout(() => void startRecording(), HOLD_MS);
  }

  function release() {
    if (!pressed) return;
    pressed = false;
    clearTimeout(holdTimer);
    if (recording && performance.now() - recordStart < MIN_VIDEO_MS) {
      discard = true;
      stopRecording();
      takePhoto();
    } else if (recording) stopRecording();
    else if (!starting) takePhoto();
  }

  function show(blob: Blob, name: string, isVideo: boolean) {
    if (shot) URL.revokeObjectURL(shot.url);
    shot = { blob, url: URL.createObjectURL(blob), name, isVideo };
  }

  function retake() {
    if (shot) URL.revokeObjectURL(shot.url);
    shot = null;
  }

  function use() {
    if (!shot) return;
    onShot(shot.blob, shot.name);
    onClose();
  }
</script>

<div class="camera" transition:fade={{ duration: 120 }}>
  <!-- svelte-ignore a11y_media_has_caption -->
  <video bind:this={video} class:mirror={facing === "user"} autoplay playsinline muted hidden={!!shot}></video>
  {#if shot}
    {#if shot.isVideo}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video class="review" src={shot.url} autoplay loop playsinline></video>
    {:else}
      <img class="review" src={shot.url} alt={tr("The photo you took")} />
    {/if}
  {/if}
  {#if flash}<div class="flash"></div>{/if}

  <header>
    <button class="round" onclick={() => (shot ? retake() : onClose())} aria-label={shot ? tr("Retake") : tr("Close")}><X size={24} /></button>
    {#if recording}<span class="rec">● {Math.round(progress * 60)} s</span>{/if}
    {#if torchAvailable && !shot}
      <button class="round" onclick={toggleTorch} aria-label={torch ? tr("Torch off") : tr("Torch on")} aria-pressed={torch}>
        {#if torch}<Zap size={22} />{:else}<ZapOff size={22} />{/if}
      </button>
    {/if}
  </header>

  {#if error}
    <div class="error">
      <p>{error}</p>
      {#if onFallback}<button class="text" onclick={() => (onClose(), onFallback?.())}>{tr("Use the camera app")}</button>{/if}
    </div>
  {/if}

  <footer>
    {#if shot}
      <button class="round big" onclick={retake} aria-label={tr("Retake")}><RotateCcw size={26} /></button>
      <button class="round big accent" onclick={use} aria-label={tr("Use")}><Check size={30} /></button>
    {:else}
      <span class="spacer"></span>
      <button
        class="shutter"
        class:recording
        style:--progress={progress}
        onpointerdown={press}
        onpointerup={release}
        onpointercancel={release}
        oncontextmenu={(e) => e.preventDefault()}
        aria-label={tr("Tap for a photo, hold to film")}
        disabled={!!error}
      ></button>
      <button class="round" onclick={flip} aria-label={tr("Switch camera")} disabled={recording}><SwitchCamera size={24} /></button>
    {/if}
  </footer>
  {#if !shot && !error && !recording}
    <p class="hint">{tr("Tap for a photo, hold to film")}</p>
  {/if}
</div>

<style>
  .camera {
    position: fixed;
    inset: 0;
    z-index: 80;
    background: #000;
    color: #fff;
    touch-action: none;
    user-select: none;
  }
  video,
  .review {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .review {
    object-fit: contain;
  }
  .mirror {
    transform: scaleX(-1);
  }
  .flash {
    position: absolute;
    inset: 0;
    background: #fff;
    opacity: 0.8;
  }
  header {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: max(12px, env(safe-area-inset-top)) calc(12px + var(--window-controls)) 12px 12px;
  }
  .rec {
    padding: 4px 10px;
    border-radius: 999px;
    background: #e5484d;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  footer {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    justify-content: space-around;
    padding: 24px 24px max(32px, env(safe-area-inset-bottom));
  }
  .spacer {
    width: 48px;
  }
  .round {
    width: 48px;
    height: 48px;
    border: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.45);
    color: #fff;
    cursor: pointer;
  }
  .round.big {
    width: 64px;
    height: 64px;
  }
  .round.accent {
    background: var(--accent);
  }
  .round:disabled {
    opacity: 0.4;
  }
  /* The ring fills while filming. */
  .shutter {
    --progress: 0;
    width: 78px;
    height: 78px;
    border-radius: 50%;
    border: none;
    padding: 0;
    cursor: pointer;
    background:
      radial-gradient(circle, #fff 0 58%, transparent 59%),
      conic-gradient(#e5484d calc(var(--progress) * 360deg), rgb(255 255 255 / 0.55) 0);
    transition: transform 0.15s ease;
  }
  .shutter.recording {
    transform: scale(1.18);
    background:
      radial-gradient(circle, #e5484d 0 40%, transparent 41%),
      conic-gradient(#e5484d calc(var(--progress) * 360deg), rgb(255 255 255 / 0.55) 0);
  }
  .shutter:disabled {
    opacity: 0.4;
  }
  .hint {
    position: absolute;
    left: 0;
    right: 0;
    bottom: calc(max(32px, env(safe-area-inset-bottom)) + 100px);
    margin: 0;
    text-align: center;
    font-size: 0.85rem;
    opacity: 0.8;
    text-shadow: 0 1px 3px #000;
  }
  .error {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 12px;
    padding: 24px;
    text-align: center;
  }
  .text {
    border: none;
    border-radius: 999px;
    padding: 10px 18px;
    background: var(--accent);
    color: #fff;
    font: inherit;
    cursor: pointer;
  }
</style>
