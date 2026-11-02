<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  // Settings → Voice & video (after Discord's): devices, a mic test, audio processing,
  // push-to-talk key, camera with preview. Saved on this device (call preferences).
  import Mic from "@lucide/svelte/icons/mic";
  import Video from "@lucide/svelte/icons/video";
  import { onDestroy } from "svelte";
  import { isAndroid, nativeDevices, type NativeDevice } from "$lib/backend";
  import { keyName } from "$lib/keys";
  import { call, type CallPrefs } from "$lib/stores/call.svelte";

  let inputs = $state<NativeDevice[]>([]);
  let outputs = $state<NativeDevice[]>([]);
  let cameras = $state<NativeDevice[]>([]);
  let error = $state<string | null>(null);
  // Browsers that can't route audio to another device (WebKit, Android) get no speaker list.
  const canPickOutput = $derived(call.native || (!isAndroid && "setSinkId" in HTMLMediaElement.prototype));

  async function load() {
    try {
      if (call.native) {
        const d = await nativeDevices();
        inputs = d.inputs;
        outputs = d.outputs;
        cameras = d.cameras;
        return;
      }
      // Device names are only given once the page may use the microphone.
      await navigator.mediaDevices
        ?.getUserMedia({ audio: true })
        .then((s) => s.getTracks().forEach((t) => t.stop()))
        .catch(() => {});
      const list = (await navigator.mediaDevices?.enumerateDevices()) ?? [];
      const of = (kind: MediaDeviceKind, fallback: string) =>
        list
          .filter((d) => d.kind === kind && d.deviceId && d.deviceId !== "default")
          .map((d, i) => ({ id: d.deviceId, name: d.label || `${fallback} ${i + 1}` }));
      inputs = of("audioinput", tr("Microphone"));
      outputs = of("audiooutput", tr("Speaker"));
      cameras = of("videoinput", tr("Camera"));
    } catch (e) {
      error = String(e);
    }
  }
  void load();

  function set(change: Partial<CallPrefs>) {
    void call.setPrefs(change);
  }

  // ── Mic test: a live level meter ──
  let testing = $state(false);
  let level = $state(0);
  let stopTest: (() => void) | null = null;
  async function toggleTest() {
    if (testing) return endTest();
    error = null;
    try {
      const p = call.prefs;
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          // The native engine's device IDs aren't the browser's: test the default there.
          deviceId: !call.native && p.inputDevice ? { exact: p.inputDevice } : undefined,
          echoCancellation: p.echoCancellation,
          noiseSuppression: p.noiseSuppression,
          autoGainControl: p.autoGain,
        },
      });
      const ctx = new AudioContext();
      const analyser = ctx.createAnalyser();
      analyser.fftSize = 1024;
      ctx.createMediaStreamSource(stream).connect(analyser);
      const data = new Float32Array(analyser.fftSize);
      let frame = 0;
      const tick = () => {
        analyser.getFloatTimeDomainData(data);
        let sum = 0;
        for (const v of data) sum += v * v;
        // RMS in dB, -60…0 → 0…1.
        const db = 20 * Math.log10(Math.sqrt(sum / data.length) || 1e-6);
        level = Math.max(0, Math.min(1, (db + 60) / 60));
        frame = requestAnimationFrame(tick);
      };
      tick();
      testing = true;
      stopTest = () => {
        cancelAnimationFrame(frame);
        stream.getTracks().forEach((t) => t.stop());
        void ctx.close();
        level = 0;
      };
    } catch (e) {
      error = e instanceof DOMException && e.name === "NotAllowedError" ? tr("AlwyChat isn't allowed to use the microphone") : String(e);
    }
  }
  function endTest() {
    stopTest?.();
    stopTest = null;
    testing = false;
  }

  // ── Camera preview ──
  let preview = $state<MediaStream | null>(null);
  async function togglePreview() {
    if (preview) return endPreview();
    error = null;
    try {
      const id = call.prefs.cameraDevice;
      preview = await navigator.mediaDevices.getUserMedia({
        video: !call.native && id ? { deviceId: { exact: id } } : true,
      });
    } catch (e) {
      error = e instanceof DOMException && e.name === "NotAllowedError" ? tr("AlwyChat isn't allowed to use the camera") : `Camera: ${e}`;
    }
  }
  function endPreview() {
    preview?.getTracks().forEach((t) => t.stop());
    preview = null;
  }
  function stream(el: HTMLVideoElement, s: MediaStream) {
    el.srcObject = s;
  }

  // ── Push-to-talk key ──
  let listening = $state(false);
  function captureKey(e: KeyboardEvent) {
    if (!listening) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.code !== "Escape") set({ pttKey: e.code });
    listening = false;
  }

  onDestroy(() => {
    endTest();
    endPreview();
  });
</script>

<svelte:window onkeydown={captureKey} />

<h3 class="m-section">{tr("Voice")}</h3>
<div class="pair">
  <label class="field">
    <span>{tr("Microphone")}</span>
    <select class="m-input" value={call.prefs.inputDevice ?? ""} onchange={(e) => set({ inputDevice: e.currentTarget.value || null })}>
      <option value="">{tr("System default")}</option>
      {#each inputs as d (d.id)}<option value={d.id}>{d.name}</option>{/each}
    </select>
  </label>
  {#if canPickOutput}
    <label class="field">
      <span>{tr("Speaker")}</span>
      <select class="m-input" value={call.prefs.outputDevice ?? ""} onchange={(e) => set({ outputDevice: e.currentTarget.value || null })}>
        <option value="">{tr("System default")}</option>
        {#each outputs as d (d.id)}<option value={d.id}>{d.name}</option>{/each}
      </select>
    </label>
  {/if}
</div>

<div class="test">
  <button class="m-btn" class:on={testing} onclick={toggleTest}><Mic size={16} />{testing ? tr("Stop test") : tr("Test microphone")}</button>
  <div class="meter" aria-label={tr("Microphone level")} role="meter" aria-valuenow={Math.round(level * 100)} aria-valuemin={0} aria-valuemax={100}>
    {#each Array(40) as _, i (i)}<span class:lit={testing && level * 40 > i} class:loud={i >= 32}></span>{/each}
  </div>
</div>
<p class="m-hint">{tr("Speak: the bars should move. Changes apply to a call in progress too.")}</p>

<h3 class="m-section">{tr("Audio processing")}</h3>
<label class="m-row toggle">
  <span class="m-text"><strong>{tr("Echo cancellation")}</strong><small>{tr("Stops others hearing themselves through your speakers")}</small></span>
  <input type="checkbox" role="switch" checked={call.prefs.echoCancellation} onchange={(e) => set({ echoCancellation: e.currentTarget.checked })} />
</label>
<label class="m-row toggle">
  <span class="m-text"><strong>{tr("Noise suppression")}</strong><small>{tr("Filters background noise (keyboard, fan, street)")}</small></span>
  <input type="checkbox" role="switch" checked={call.prefs.noiseSuppression} onchange={(e) => set({ noiseSuppression: e.currentTarget.checked })} />
</label>
<label class="m-row toggle">
  <span class="m-text"><strong>{tr("Automatic gain")}</strong><small>{tr("Keeps your voice at an even volume")}</small></span>
  <input type="checkbox" role="switch" checked={call.prefs.autoGain} onchange={(e) => set({ autoGain: e.currentTarget.checked })} />
</label>

<h3 class="m-section">{tr("Push to talk")}</h3>
<label class="m-row toggle">
  <span class="m-text"><strong>{tr("Push to talk")}</strong><small>{tr("Your mic is open only while you hold the talk button{value}", { value: isAndroid ? "" : " or the key below" })}</small></span>
  <input type="checkbox" role="switch" checked={call.prefs.pushToTalk} onchange={(e) => call.setPushToTalk(e.currentTarget.checked)} />
</label>
{#if !isAndroid}
  <div class="m-row">
    <span class="m-text"><strong>{tr("Key")}</strong><small>{tr("Works while AlwyChat is the active window")}</small></span>
    <button class="m-btn key" class:on={listening} onclick={() => (listening = !listening)}>
      {listening ? tr("Press a key… (Esc to cancel)") : keyName(call.prefs.pttKey)}
    </button>
  </div>
{/if}

<h3 class="m-section">{tr("Video")}</h3>
<label class="field">
  <span>{tr("Camera")}</span>
  <select class="m-input" value={call.prefs.cameraDevice ?? ""} onchange={(e) => (endPreview(), set({ cameraDevice: e.currentTarget.value || null }))}>
    <option value="">{call.native ? tr("First camera") : tr("System default")}</option>
    {#each cameras as d (d.id)}<option value={d.id}>{d.name}</option>{/each}
  </select>
</label>
<div class="preview">
  {#if preview}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video use:stream={preview} autoplay playsinline muted></video>
  {:else}
    <button class="m-btn" onclick={togglePreview}><Video size={16} />{tr("Preview camera")}</button>
  {/if}
</div>
{#if preview}<button class="m-btn" onclick={endPreview}>{tr("Stop preview")}</button>{/if}

{#if error}<p class="m-error">{error}</p>{/if}

<style>
  .pair {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 12px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .field > span {
    font-size: 0.85rem;
    font-weight: 600;
  }
  select {
    width: 100%;
  }
  .test {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-top: 16px;
  }
  .m-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    white-space: nowrap;
  }
  .m-btn.on {
    background: var(--accent);
    color: #fff;
  }
  .meter {
    flex: 1;
    display: flex;
    gap: 3px;
    height: 22px;
    align-items: stretch;
  }
  .meter span {
    flex: 1;
    border-radius: 2px;
    background: var(--surface-hover);
    transition: background 0.06s;
  }
  .meter span.lit {
    background: #3ba55d;
  }
  .meter span.lit.loud {
    background: #faa61a;
  }
  .toggle {
    cursor: pointer;
  }
  .key {
    min-width: 120px;
    justify-content: center;
    font-variant-numeric: tabular-nums;
  }
  .preview {
    display: grid;
    place-items: center;
    aspect-ratio: 16 / 9;
    max-width: 480px;
    margin: 12px 0 8px;
    border-radius: 14px;
    overflow: hidden;
    background: var(--surface-2);
  }
  .preview video {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transform: scaleX(-1);
  }
</style>
