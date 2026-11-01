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
</script>

<div class="scene" class:mini aria-hidden="true">
  <svg viewBox="0 0 400 860" preserveAspectRatio="xMidYMax slice">
    {#if scene === "ocean"}
      <defs>
        <linearGradient id="oc-bg" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#1366d6" />
          <stop offset="0.45" stop-color="#0a3c94" />
          <stop offset="1" stop-color="#020a22" />
        </linearGradient>
        <linearGradient id="oc-ray" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#9fe8ff" stop-opacity="0.35" />
          <stop offset="1" stop-color="#9fe8ff" stop-opacity="0" />
        </linearGradient>
      </defs>
      <rect width="400" height="860" fill="url(#oc-bg)" />
      <g class="rays">
        <polygon points="120,0 170,0 260,620 150,620" fill="url(#oc-ray)" />
        <polygon points="230,0 260,0 330,520 250,520" fill="url(#oc-ray)" opacity="0.7" />
        <polygon points="40,0 70,0 120,480 40,480" fill="url(#oc-ray)" opacity="0.5" />
      </g>
      <!-- turtle -->
      <g class="swim" fill="#0b2f6e" opacity="0.75">
        <ellipse cx="190" cy="330" rx="62" ry="30" />
        <ellipse cx="262" cy="322" rx="20" ry="13" />
        <path d="M150 342 q-40 20 -60 50 q40 -6 70 -36z" />
        <path d="M215 352 q20 30 10 60 q-22 -20 -28 -54z" />
        <path d="M160 312 q-30 -26 -60 -26 q20 22 52 34z" />
        <path d="M128 336 q-18 2 -26 10 q16 2 28 -4z" />
      </g>
      <!-- jellyfish -->
      <g class="bob" stroke="#7fb8ff" fill="none" opacity="0.35" stroke-width="2">
        <path d="M300 170 q30 -40 60 0 z" fill="#7fb8ff" fill-opacity="0.25" />
        <path d="M308 172 q-6 30 6 60 M322 172 q6 34 -4 70 M336 172 q-4 30 8 62 M350 172 q6 28 -2 54" />
      </g>
      <!-- angler fish -->
      <g fill="#03122e" opacity="0.9">
        <path d="M200 690 q50 -40 100 -10 q-20 40 -100 30 l-30 20 l6 -32 l-24 -18 z" />
        <circle cx="290" cy="640" r="5" fill="#bdf6ff" class="glow" />
        <path d="M268 668 q10 -30 22 -28" stroke="#03122e" stroke-width="2" fill="none" />
      </g>
      <!-- seaweed and coral -->
      <g fill="#041a44">
        <path d="M0 860 V560 q30 -40 20 -90 q30 30 22 80 q24 -40 50 -30 q-30 40 -22 100 q10 60 -10 300z" />
        <path d="M400 860 V600 q-26 -50 -10 -110 q-36 30 -28 90 q-30 -30 -54 -10 q34 30 30 90 q-6 50 4 200z" />
        <path d="M120 860 q-10 -120 20 -200 q10 80 -4 200z M150 860 q0 -90 30 -150 q0 70 -14 150z" />
        <path d="M260 860 q10 -100 -10 -170 q30 60 34 170z" />
      </g>
      {#each bubbles as b (b.i)}
        <circle class="bubble" cx={b.x} cy={860 - b.y / 3} r={2 + b.s * 6} fill="none" stroke="#bfefff" stroke-opacity="0.4" style:--d="{8 + b.d * 10}s" style:--delay="{-b.s * 12}s" />
      {/each}
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
        <ellipse class="fall" cx={p.x} cy={-20} rx="5" ry="3" fill="#ffc8da" opacity="0.85" style:--d="{9 + p.d * 8}s" style:--delay="{-p.s * 16}s" style:--drift="{(p.s - 0.5) * 120}px" />
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
        <path class="rise" transform="translate({h.x} 900) scale({0.6 + h.s})" d="M0 6 C0 -2 -12 -2 -12 6 C-12 14 0 20 0 24 C0 20 12 14 12 6 C12 -2 0 -2 0 6Z" fill="#ff5c8a" opacity={0.25 + h.d * 0.35} style:--d="{10 + h.d * 10}s" style:--delay="{-h.s * 20}s" />
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
  </svg>
</div>

<style>
  .scene {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
  }
  svg {
    width: 100%;
    height: 100%;
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
  .bob {
    animation: bob 6s ease-in-out infinite alternate;
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
  .glow {
    animation: twinkle 1.6s ease-in-out infinite alternate;
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
  @keyframes bob {
    from {
      transform: translateY(0);
    }
    to {
      transform: translateY(-24px);
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
