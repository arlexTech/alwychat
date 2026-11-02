<script lang="ts">
  import type { Scene } from "$lib/themes";

  /**
   * A theme's illustrated chat background: hand-drawn SVG, slowly animated (bubbles, stars,
   * petals…), still when the system asks for reduced motion. `mini`: a card preview (no motion).
   */
  let { scene, mini = false }: { scene: Scene; mini?: boolean } = $props();

  /** Same "random" layout every time (a scene shouldn't reshuffle on each render). */
  function rand(seed: number) {
    let s = seed;
    return () => ((s = (s * 16807) % 2147483647) - 1) / 2147483646;
  }
  const dots = (seed: number, n: number) => {
    const r = rand(seed);
    return Array.from({ length: n }, (_, i) => ({ i, x: r() * 400, y: r() * 860, s: r(), d: r() }));
  };
  const stars = dots(7, 70);
  const bubbles = dots(11, 18);
  const petals = dots(23, 16);
  const flies = dots(31, 14);
  const hearts = dots(43, 14);

  let width = $state(0);
  let height = $state(0);
  const wide = $derived(!mini && width > height * 0.75);
</script>

<!-- Drawn for a phone (portrait). On a wider-than-tall chat, covering would zoom it far in, so
     the whole scene is fitted to the height in the middle and the sides get a blurred, still
     copy of it (like a letterboxed video). -->
<div class="scene" class:mini aria-hidden="true" bind:clientWidth={width} bind:clientHeight={height}>
  {#if wide}
    <svg class="backdrop" viewBox="0 0 400 860" preserveAspectRatio="xMidYMid slice">{@render art()}</svg>
  {/if}
  <svg class:fitted={wide} viewBox="0 0 400 860" preserveAspectRatio={wide ? "xMidYMax meet" : "xMidYMax slice"}>{@render art()}</svg>
</div>

{#snippet art()}
    {#if scene === "ocean"}
      <!-- Still on purpose: depth comes from haze (far shapes blurred and faded into the water). -->
      <defs>
        <linearGradient id="oc-bg" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#0c6ad6" />
          <stop offset="0.3" stop-color="#0a52b8" />
          <stop offset="0.55" stop-color="#073a8c" />
          <stop offset="0.78" stop-color="#03173f" />
          <stop offset="1" stop-color="#01050e" />
        </linearGradient>
        <radialGradient id="oc-sky" cx="0.5" cy="0" r="0.7">
          <stop offset="0" stop-color="#5fb4ff" stop-opacity="0.35" />
          <stop offset="1" stop-color="#5fb4ff" stop-opacity="0" />
        </radialGradient>
        <linearGradient id="oc-ray" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#bfe4ff" stop-opacity="0.18" />
          <stop offset="1" stop-color="#bfe4ff" stop-opacity="0" />
        </linearGradient>
        <linearGradient id="oc-fade" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#01050e" stop-opacity="0" />
          <stop offset="1" stop-color="#01050e" stop-opacity="0.9" />
        </linearGradient>
        <radialGradient id="oc-lure" r="0.5">
          <stop offset="0" stop-color="#d8fff6" />
          <stop offset="0.25" stop-color="#7dffe0" stop-opacity="0.8" />
          <stop offset="1" stop-color="#7dffe0" stop-opacity="0" />
        </radialGradient>
        <filter id="oc-far" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="4" /></filter>
        <filter id="oc-mid" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="1.2" /></filter>
        <filter id="oc-soft" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="10" /></filter>
      </defs>
      <rect width="400" height="860" fill="url(#oc-bg)" />
      <rect width="400" height="500" fill="url(#oc-sky)" />
      <g filter="url(#oc-soft)">
        <polygon points="150,-10 205,-10 290,560 170,560" fill="url(#oc-ray)" />
        <polygon points="250,-10 280,-10 345,430 268,430" fill="url(#oc-ray)" />
        <polygon points="60,-10 95,-10 130,380 55,380" fill="url(#oc-ray)" />
      </g>

      <!-- far: hazy kelp and rocks deep below -->
      <g filter="url(#oc-far)" fill="#06347e" opacity="0.6">
        <path d="M150 860 C140 760 160 680 150 600 C170 680 178 770 172 860Z" />
        <path d="M188 860 C196 770 186 700 200 620 C206 700 210 790 206 860Z" />
        <path d="M232 860 C226 790 240 720 232 660 C250 720 254 800 248 860Z" />
        <path d="M262 860 C270 800 262 750 276 700 C282 760 284 820 280 860Z" />
        <ellipse cx="330" cy="560" rx="40" ry="16" />
        <path d="M300 520 c10 -30 40 -30 50 0 c-14 -6 -36 -6 -50 0z" />
      </g>

      <!-- jellyfish, far right -->
      <g filter="url(#oc-mid)" opacity="0.45">
        <path d="M296 196 C298 160 352 150 360 192 C346 186 310 186 296 196Z" fill="#3f86df" />
        <g stroke="#3f86df" stroke-width="2" fill="none" stroke-linecap="round">
          <path d="M304 194 C298 230 314 250 304 290 C298 320 310 340 302 370" />
          <path d="M316 194 C322 236 308 262 318 300 C324 330 314 350 320 384" />
          <path d="M330 194 C326 230 340 258 330 296 C324 326 336 352 328 380" />
          <path d="M344 192 C350 226 340 250 350 286 C356 310 348 330 352 356" />
        </g>
      </g>

      <!-- left: tube sponges of different heights, openings on top -->
      <g filter="url(#oc-mid)">
        {#each [[-6, 360, 44], [30, 420, 38], [62, 500, 34], [-10, 540, 40], [86, 600, 30]] as [x, top, w], i (i)}
          <path d="M{x} 860 V{top + 10} C{x} {top - 4} {x + w} {top - 4} {x + w} {top + 10} V860Z" fill={i % 2 ? "#0a4192" : "#083a88"} opacity="0.92" />
          <ellipse cx={x + w / 2} cy={top + 8} rx={w / 2 - 5} ry="5" fill="#04245a" />
        {/each}
        <g fill="#0a4598" opacity="0.9">
          {#each [[8, 440], [16, 480], [42, 500], [48, 540], [12, 600], [72, 560], [24, 650], [92, 660]] as [x, y], i (i)}
            <circle cx={x} cy={y} r="3.5" fill="#062e6e" />
          {/each}
        </g>
      </g>
      <g fill="#072f73" opacity="0.9">
        <path d="M100 860 C104 800 120 760 140 740 C132 770 128 800 134 860Z" />
        <path d="M118 772 C130 752 150 748 160 756 C146 760 136 768 128 780Z" />
      </g>

      <!-- right: kelp with leaves, swaying the same way -->
      <g filter="url(#oc-mid)" opacity="0.9">
        {#each [[372, 440, "#083a88"], [344, 520, "#07337c"], [392, 600, "#0a4192"]] as [x, top, color], i (i)}
          <path d="M{x} 860 C{Number(x) - 14} 760 {Number(x) + 10} 680 {x} {top}" stroke={String(color)} stroke-width="5" fill="none" />
          {#each Array(7) as _, j (j)}
            {@const y = Number(top) + 30 + j * 52}
            <path d="M{Number(x) - 2} {y} C{Number(x) - 34} {y - 20} {Number(x) - 40} {y - 46} {Number(x) - 26} {y - 54} C{Number(x) - 18} {y - 30} {Number(x) - 8} {y - 16} {Number(x) - 2} {y}Z" fill={String(color)} />
            <path d="M{Number(x) + 2} {y + 24} C{Number(x) + 30} {y + 4} {Number(x) + 36} {y - 20} {Number(x) + 24} {y - 28} C{Number(x) + 16} {y - 6} {Number(x) + 8} {y + 8} {Number(x) + 2} {y + 24}Z" fill={String(color)} />
          {/each}
        {/each}
      </g>

      <!-- turtle, mid-water, swimming right -->
      <g opacity="0.85" fill="#0b4192">
        <!-- far front flipper -->
        <path d="M226 322 C246 300 276 292 296 296 C278 306 252 316 232 330Z" fill="#093a85" />
        <!-- rear flippers -->
        <path d="M130 344 C116 350 104 362 100 372 C116 368 130 360 140 350Z" />
        <path d="M138 318 C124 308 112 306 104 310 C116 318 128 322 140 326Z" fill="#093a85" />
        <!-- shell -->
        <path d="M128 334 C132 300 176 286 210 292 C238 298 250 316 246 334 C240 356 200 366 168 362 C144 358 126 350 128 334Z" fill="#0c4597" />
        <g stroke="#1758b4" stroke-width="1.6" fill="none" stroke-linejoin="round" opacity="0.9">
          <path d="M166 302 L196 300 L214 316 L204 338 L174 342 L158 324Z" />
          <path d="M166 302 L150 312 M196 300 L206 294 M214 316 L236 318 M204 338 L220 352 M174 342 L168 358 M158 324 L134 330" />
        </g>
        <!-- head -->
        <path d="M244 322 C252 312 270 310 282 316 C290 322 286 334 274 336 C262 338 252 334 244 330Z" fill="#0c4597" />
        <circle cx="274" cy="321" r="2.2" fill="#03173f" />
        <!-- near front flipper, sweeping back like a wing -->
        <path d="M222 340 C232 360 226 392 206 414 C204 392 200 370 196 352Z" fill="#0d4a9e" />
      </g>

      <!-- small round fish, right -->
      <g fill="#06336f" opacity="0.8" filter="url(#oc-mid)">
        <path d="M300 470 C300 446 330 440 344 458 L362 446 L358 472 L364 494 L344 484 C330 500 300 494 300 470Z" />
      </g>
      <!-- squid, lower middle -->
      <g fill="#05285e" opacity="0.7" filter="url(#oc-mid)">
        <path d="M246 560 C262 534 290 520 312 526 C300 540 280 556 262 568Z" />
        <path d="M250 566 C236 574 224 590 214 606 M256 570 C246 584 240 600 236 620 M244 562 C228 566 214 576 200 590" stroke="#05285e" stroke-width="3" fill="none" stroke-linecap="round" />
      </g>

      <!-- bubbles -->
      <g fill="none" stroke="#a9d8ff" stroke-opacity="0.35" stroke-width="1.5">
        <circle cx="160" cy="470" r="9" /><circle cx="172" cy="446" r="5" /><circle cx="168" cy="430" r="3" />
        <circle cx="236" cy="268" r="6" /><circle cx="244" cy="252" r="3.5" />
        <circle cx="200" cy="700" r="10" /><circle cx="210" cy="676" r="6" /><circle cx="206" cy="660" r="3" />
        <circle cx="120" cy="240" r="4" /><circle cx="328" cy="640" r="4" />
      </g>
      <g fill="#cfeaff" fill-opacity="0.35">
        <circle cx="157" cy="466" r="2" /><circle cx="197" cy="696" r="2.4" /><circle cx="234" cy="265" r="1.4" />
      </g>

      <!-- depth fade, then the anglerfish in the dark -->
      <rect y="560" width="400" height="300" fill="url(#oc-fade)" />
      <circle cx="262" cy="690" r="40" fill="url(#oc-lure)" opacity="0.5" />
      <g fill="#020c22">
        <!-- body: big head with an open jaw, tapering to the tail -->
        <path d="M282 744 C280 716 256 700 228 702 C196 704 176 722 168 744 L140 728 L146 756 L138 784 L168 768 C182 790 214 800 244 794 C262 790 276 780 282 770 L264 762 L282 744Z" />
        <path d="M230 704 C236 690 248 682 262 690 C266 700 264 708 262 712" stroke="#020c22" stroke-width="2.5" fill="none" />
        <path d="M206 704 C204 690 196 684 186 690 C194 694 198 700 200 708Z" />
      </g>
      <path d="M266 748 L270 756 L274 748 L278 758 M266 768 L270 762 L274 770 L278 762" stroke="#a8c6ff" stroke-opacity="0.55" stroke-width="1.4" fill="none" />
      <circle cx="262" cy="690" r="3.5" fill="#e6fff8" />
      <circle cx="252" cy="728" r="3" fill="#0a2a5c" />

      <!-- seabed -->
      <path d="M0 860 V800 C60 780 120 806 180 792 C240 778 300 800 400 784 V860Z" fill="#01040b" />
    {:else if scene === "lagoon"}
      <defs>
        <linearGradient id="lg-bg" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#18c2c9" />
          <stop offset="0.5" stop-color="#0a7c94" />
          <stop offset="1" stop-color="#033442" />
        </linearGradient>
      </defs>
      <rect width="400" height="860" fill="url(#lg-bg)" />
      <g class="rays" fill="#e6ffff" opacity="0.12">
        <polygon points="80,0 120,0 220,700 120,700" />
        <polygon points="260,0 280,0 340,600 270,600" />
      </g>
      <g fill="#ffb347" opacity="0.85" class="swim">
        <path d="M120 300 q30 -20 60 0 q-30 20 -60 0z M180 300 l18 -12 v24z" />
      </g>
      <g fill="#ff6b8b" opacity="0.8" class="swim2">
        <path d="M250 420 q24 -16 48 0 q-24 16 -48 0z M298 420 l14 -10 v20z" />
      </g>
      <g fill="#02303d">
        <path d="M0 860 V700 q60 -30 120 -10 q60 -40 120 0 q80 -30 160 10 V860z" />
        <circle cx="70" cy="690" r="26" /><circle cx="330" cy="700" r="30" />
        <path d="M180 690 q-6 -60 10 -100 q6 50 2 100z M200 690 q4 -50 20 -80 q-4 50 -10 80z" />
      </g>
      {#each bubbles.slice(0, 10) as b (b.i)}
        <circle class="bubble" cx={b.x} cy={860 - b.y / 4} r={2 + b.s * 4} fill="#e6ffff" fill-opacity="0.25" style:--d="{9 + b.d * 8}s" style:--delay="{-b.s * 12}s" />
      {/each}
    {:else if scene === "space"}
      <defs>
        <linearGradient id="sp-bg" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#0d0624" />
          <stop offset="1" stop-color="#1c0b3e" />
        </linearGradient>
        <radialGradient id="sp-neb" cx="0.3" cy="0.35" r="0.5">
          <stop offset="0" stop-color="#a24cff" stop-opacity="0.45" />
          <stop offset="1" stop-color="#a24cff" stop-opacity="0" />
        </radialGradient>
        <radialGradient id="sp-neb2" cx="0.75" cy="0.7" r="0.45">
          <stop offset="0" stop-color="#ff4c9a" stop-opacity="0.3" />
          <stop offset="1" stop-color="#ff4c9a" stop-opacity="0" />
        </radialGradient>
        <linearGradient id="sp-planet" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#ffb36b" />
          <stop offset="1" stop-color="#b03a8a" />
        </linearGradient>
      </defs>
      <rect width="400" height="860" fill="url(#sp-bg)" />
      <rect width="400" height="860" fill="url(#sp-neb)" />
      <rect width="400" height="860" fill="url(#sp-neb2)" />
      {#each stars as s (s.i)}
        <circle class="twinkle" cx={s.x} cy={s.y} r={0.6 + s.s * 1.6} fill="#fff" style:--d="{2 + s.d * 4}s" style:--delay="{-s.s * 4}s" />
      {/each}
      <g transform="translate(300 230)">
        <circle r="46" fill="url(#sp-planet)" />
        <ellipse rx="80" ry="16" fill="none" stroke="#ffd6a8" stroke-opacity="0.6" stroke-width="4" transform="rotate(-18)" />
      </g>
      <circle cx="80" cy="640" r="18" fill="#6d6fb0" opacity="0.7" />
      <path class="comet" d="M0 0 l60 18" stroke="#fff" stroke-opacity="0.7" stroke-width="2" stroke-linecap="round" />
    {:else if scene === "aurora"}
      <defs>
        <linearGradient id="au-bg" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#03111f" />
          <stop offset="1" stop-color="#0b2a3a" />
        </linearGradient>
        <linearGradient id="au-band" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#4dffb8" stop-opacity="0" />
          <stop offset="0.5" stop-color="#4dffb8" stop-opacity="0.55" />
          <stop offset="1" stop-color="#4dffb8" stop-opacity="0" />
        </linearGradient>
        <linearGradient id="au-band2" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#b05cff" stop-opacity="0" />
          <stop offset="0.5" stop-color="#b05cff" stop-opacity="0.4" />
          <stop offset="1" stop-color="#b05cff" stop-opacity="0" />
        </linearGradient>
      </defs>
      <rect width="400" height="860" fill="url(#au-bg)" />
      {#each stars.slice(0, 40) as s (s.i)}
        <circle class="twinkle" cx={s.x} cy={s.y * 0.6} r={0.5 + s.s} fill="#fff" style:--d="{3 + s.d * 3}s" style:--delay="{-s.s * 4}s" />
      {/each}
      <path class="sway" d="M-40 260 C60 160 140 360 240 230 S380 200 460 260 L460 420 C360 360 260 460 160 380 S20 380 -40 440Z" fill="url(#au-band)" />
      <path class="sway2" d="M-40 340 C80 260 160 420 260 330 S400 300 460 360 L460 480 C340 420 240 520 140 450 S20 460 -40 500Z" fill="url(#au-band2)" />
      <g fill="#020b14">
        <path d="M0 860 V640 l60 -70 l40 40 l70 -110 l60 90 l50 -50 l80 110 l40 -30 V860z" />
      </g>
      <g fill="#04131f">
        <path d="M0 860 V720 l50 -30 l60 40 l60 -50 l80 60 l70 -40 l80 40 V860z" />
      </g>
    {:else if scene === "sakura"}
      <defs>
        <linearGradient id="sk-bg" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#3a1d4a" />
          <stop offset="0.55" stop-color="#7a3a6a" />
          <stop offset="1" stop-color="#f08aa8" />
        </linearGradient>
      </defs>
      <rect width="400" height="860" fill="url(#sk-bg)" />
      <circle cx="90" cy="180" r="44" fill="#ffe3ec" opacity="0.35" />
      <g fill="#2a1424">
        <path d="M400 60 C320 80 260 140 220 130 C250 150 300 120 330 140 C300 170 260 220 230 230 C280 220 330 170 400 160Z" />
        <path d="M0 860 V760 q100 -40 200 -10 q100 -30 200 10 V860z" opacity="0.8" />
      </g>
      <g fill="#ffb7cf">
        {#each [[230, 128], [262, 118], [300, 132], [330, 120], [350, 150], [380, 110], [280, 160], [240, 220], [260, 205], [310, 175]] as [x, y], i (i)}
          <circle cx={x} cy={y} r="11" opacity="0.85" />
        {/each}
      </g>
      {#each petals as p (p.i)}
        <ellipse class="fall" cx={p.x} cy={p.y - 100} rx="5" ry="3" fill="#ffc8da" opacity="0.85" style:--d="{9 + p.d * 8}s" style:--delay="{-p.s * 16}s" style:--drift="{(p.s - 0.5) * 120}px" />
      {/each}
    {:else if scene === "forest"}
      <defs>
        <linearGradient id="fo-bg" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#0b2a2a" />
          <stop offset="0.6" stop-color="#14473a" />
          <stop offset="1" stop-color="#06180f" />
        </linearGradient>
      </defs>
      <rect width="400" height="860" fill="url(#fo-bg)" />
      <circle cx="300" cy="150" r="34" fill="#f4f1c8" opacity="0.5" />
      {#each [[0.9, "#0d3a2c", 520], [1, "#0a2c20", 600], [1.15, "#061a12", 690]] as [scale, color, base], layer (layer)}
        <g fill={String(color)}>
          {#each Array(9) as _, i (i)}
            {@const x = i * 52 - 20 + layer * 18}
            {@const h = 150 + ((i * 37 + layer * 23) % 70)}
            <path d="M{x} {Number(base) + 200} L{x + 26 * Number(scale)} {Number(base) - h * Number(scale) + 200} L{x + 52 * Number(scale)} {Number(base) + 200}Z" />
          {/each}
          <rect x="0" y={Number(base) + 190} width="400" height="400" />
        </g>
      {/each}
      {#each flies as f (f.i)}
        <circle class="firefly" cx={f.x} cy={420 + f.y / 3} r={1.5 + f.s * 1.5} fill="#e9ff8a" style:--d="{3 + f.d * 4}s" style:--delay="{-f.s * 5}s" />
      {/each}
    {:else if scene === "sunset"}
      <defs>
        <linearGradient id="su-bg" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#2b1250" />
          <stop offset="0.45" stop-color="#c03a6a" />
          <stop offset="0.7" stop-color="#ff8a4c" />
          <stop offset="1" stop-color="#ffc06b" />
        </linearGradient>
        <linearGradient id="su-sea" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#4a1d5e" />
          <stop offset="1" stop-color="#160a2a" />
        </linearGradient>
      </defs>
      <rect width="400" height="860" fill="url(#su-bg)" />
      <circle cx="200" cy="560" r="80" fill="#ffe08a" opacity="0.9" />
      <rect y="580" width="400" height="280" fill="url(#su-sea)" />
      {#each [600, 630, 665, 705, 750] as y, i (i)}
        <rect class="shimmer" x={150 - i * 10} y={y} width={100 + i * 20} height="3" rx="1.5" fill="#ffe08a" opacity={0.5 - i * 0.08} style:--delay="{-i * 0.6}s" />
      {/each}
      <g fill="#1d0c2e" opacity="0.8">
        <path d="M60 300 q10 -8 20 0 q10 -8 20 0" stroke="#1d0c2e" stroke-width="3" fill="none" />
        <path d="M110 260 q8 -6 16 0 q8 -6 16 0" stroke="#1d0c2e" stroke-width="3" fill="none" />
      </g>
    {:else if scene === "love"}
      <defs>
        <radialGradient id="lv-bg" cx="0.5" cy="0.4" r="0.8">
          <stop offset="0" stop-color="#8a1240" />
          <stop offset="1" stop-color="#2a0616" />
        </radialGradient>
      </defs>
      <rect width="400" height="860" fill="url(#lv-bg)" />
      {#each hearts as h (h.i)}
        <path class="rise" transform="translate({h.x} {h.y + 100}) scale({0.6 + h.s})" d="M0 6 C0 -2 -12 -2 -12 6 C-12 14 0 20 0 24 C0 20 12 14 12 6 C12 -2 0 -2 0 6Z" fill="#ff5c8a" opacity={0.25 + h.d * 0.35} style:--d="{10 + h.d * 10}s" style:--delay="{-h.s * 20}s" />
      {/each}
    {:else if scene === "night"}
      <defs>
        <linearGradient id="ni-bg" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#070b24" />
          <stop offset="1" stop-color="#1e2350" />
        </linearGradient>
      </defs>
      <rect width="400" height="860" fill="url(#ni-bg)" />
      {#each stars.slice(0, 50) as s (s.i)}
        <circle class="twinkle" cx={s.x} cy={s.y * 0.7} r={0.5 + s.s * 1.2} fill="#fff" style:--d="{2 + s.d * 4}s" style:--delay="{-s.s * 4}s" />
      {/each}
      <circle cx="300" cy="170" r="40" fill="#fff6d6" />
      <circle cx="318" cy="158" r="40" fill="#0b1030" />
      <g fill="#0a0d26">
        {#each Array(12) as _, i (i)}
          {@const h = 120 + ((i * 53) % 140)}
          <rect x={i * 34 - 4} y={860 - h - 60} width="30" height={h + 60} />
        {/each}
      </g>
      <g fill="#ffd27a">
        {#each dots(59, 40) as w (w.i)}
          {#if w.s > 0.45}<rect class="window" x={Math.floor(w.x / 34) * 34 + 4 + (w.d > 0.5 ? 12 : 0)} y={700 + w.y / 8} width="6" height="8" opacity="0.8" style:--delay="{-w.s * 9}s" />{/if}
        {/each}
      </g>
    {/if}
{/snippet}

<style>
  .scene {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
  }
  svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  .backdrop {
    filter: blur(32px) saturate(1.1);
    transform: scale(1.15);
  }
  .backdrop :global(*) {
    animation: none !important;
  }
  /* Soft edges where the scene meets its blurred sides. */
  .fitted {
    right: auto;
    left: 50%;
    width: auto;
    aspect-ratio: 400 / 860;
    translate: -50% 0;
    mask-image: linear-gradient(90deg, transparent, black 30%, black 70%, transparent);
  }
  .bubble {
    animation: rise var(--d) linear infinite;
    animation-delay: var(--delay);
  }
  .rise {
    animation: rise-far var(--d) linear infinite;
    animation-delay: var(--delay);
  }
  .twinkle {
    animation: twinkle var(--d) ease-in-out infinite alternate;
    animation-delay: var(--delay);
  }
  .fall {
    animation: fall var(--d) linear infinite;
    animation-delay: var(--delay);
  }
  .firefly {
    animation: firefly var(--d) ease-in-out infinite alternate;
    animation-delay: var(--delay);
  }
  .swim {
    animation: swim 14s ease-in-out infinite alternate;
  }
  .swim2 {
    animation: swim 11s ease-in-out infinite alternate-reverse;
  }
  .rays {
    animation: rays 9s ease-in-out infinite alternate;
  }
  .sway {
    animation: sway 12s ease-in-out infinite alternate;
  }
  .sway2 {
    animation: sway 15s ease-in-out infinite alternate-reverse;
  }
  .shimmer {
    animation: twinkle 2.4s ease-in-out infinite alternate;
    animation-delay: var(--delay);
  }
  .comet {
    animation: comet 9s ease-in infinite;
  }
  @keyframes rise {
    from {
      transform: translateY(0);
    }
    to {
      transform: translateY(-700px);
      opacity: 0;
    }
  }
  @keyframes rise-far {
    from {
      translate: 0 0;
      opacity: 0;
    }
    10% {
      opacity: 1;
    }
    to {
      translate: 0 -1000px;
    }
  }
  @keyframes twinkle {
    from {
      opacity: 0.25;
    }
    to {
      opacity: 1;
    }
  }
  @keyframes fall {
    from {
      transform: translate(0, 0) rotate(0deg);
      opacity: 0;
    }
    10% {
      opacity: 0.85;
    }
    to {
      transform: translate(var(--drift), 920px) rotate(360deg);
    }
  }
  @keyframes firefly {
    from {
      opacity: 0.1;
      transform: translate(0, 0);
    }
    to {
      opacity: 0.9;
      transform: translate(10px, -14px);
    }
  }
  @keyframes swim {
    from {
      transform: translateX(-20px);
    }
    to {
      transform: translateX(30px);
    }
  }
  @keyframes rays {
    from {
      opacity: 0.6;
    }
    to {
      opacity: 1;
    }
  }
  @keyframes sway {
    from {
      transform: translateX(-20px) scaleY(0.95);
    }
    to {
      transform: translateX(20px) scaleY(1.05);
    }
  }
  @keyframes comet {
    0%,
    80% {
      transform: translate(-80px, 120px);
      opacity: 0;
    }
    85% {
      opacity: 1;
    }
    100% {
      transform: translate(420px, 250px);
      opacity: 0;
    }
  }
  .mini *,
  .mini {
    animation: none !important;
  }
  @media (prefers-reduced-motion: reduce) {
    .scene * {
      animation: none !important;
    }
  }
</style>
