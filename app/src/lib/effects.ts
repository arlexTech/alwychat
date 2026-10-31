// Full-screen message effects on a throwaway canvas (Element-compatible names, see
// crates/alwy-core/src/effects.rs). One at a time; nothing runs between effects.

import type { Effect } from "$lib/backend";

type Particle = {
  x: number;
  y: number;
  vx: number;
  vy: number;
  size: number;
  color: string;
  spin: number;
  angle: number;
  /** Seconds left; fades out over the last half second. */
  life: number;
  glyph?: string;
};

type Spec = {
  duration: number;
  gravity: number;
  /** Particles added at the start and, for steady effects, per second while it runs. */
  start: (w: number, h: number) => Particle[];
  perSecond?: number;
  spawn?: (w: number, h: number) => Particle[];
  draw: (ctx: CanvasRenderingContext2D, p: Particle) => void;
};

const CONFETTI = ["#ff5c8a", "#7b5cff", "#ffd23f", "#3bceac", "#22b2d8", "#ff7a59"];
const rand = (a: number, b: number) => a + Math.random() * (b - a);
const pick = <T>(list: T[]) => list[Math.floor(Math.random() * list.length)];
const isLight = () => document.documentElement.dataset.theme === "light";

function heart(ctx: CanvasRenderingContext2D, s: number) {
  ctx.beginPath();
  ctx.moveTo(0, s * 0.3);
  ctx.bezierCurveTo(-s, -s * 0.4, -s * 0.4, -s, 0, -s * 0.45);
  ctx.bezierCurveTo(s * 0.4, -s, s, -s * 0.4, 0, s * 0.3);
  ctx.fill();
}

function burst(x: number, y: number): Particle[] {
  const color = pick(CONFETTI);
  return Array.from({ length: 46 }, (_, i) => {
    const a = (i / 46) * Math.PI * 2;
    const speed = rand(140, 260);
    return { x, y, vx: Math.cos(a) * speed, vy: Math.sin(a) * speed, size: 2.4, color, spin: 0, angle: 0, life: rand(1.1, 1.6) };
  });
}

const SPECS: Record<Effect, Spec> = {
  confetti: {
    duration: 3.5,
    gravity: 260,
    start: (w) =>
      Array.from({ length: 160 }, () => ({
        x: rand(0, w),
        y: rand(-40, -10),
        vx: rand(-60, 60),
        vy: rand(-80, 120),
        size: rand(6, 11),
        color: pick(CONFETTI),
        spin: rand(-8, 8),
        angle: rand(0, Math.PI),
        life: rand(2.6, 3.5),
      })),
    draw: (ctx, p) => {
      ctx.fillStyle = p.color;
      ctx.fillRect(-p.size / 2, -p.size / 4, p.size, p.size / 2);
    },
  },
  fireworks: {
    duration: 3.4,
    gravity: 120,
    start: (w, h) => burst(w / 2, h * 0.35),
    perSecond: 2.2,
    spawn: (w, h) => burst(rand(w * 0.15, w * 0.85), rand(h * 0.15, h * 0.5)),
    draw: (ctx, p) => {
      ctx.fillStyle = p.color;
      ctx.beginPath();
      ctx.arc(0, 0, p.size, 0, Math.PI * 2);
      ctx.fill();
    },
  },
  hearts: {
    duration: 4,
    gravity: -30,
    start: (w, h) =>
      Array.from({ length: 34 }, () => ({
        x: rand(0, w),
        y: h + rand(10, 200),
        vx: rand(-20, 20),
        vy: rand(-170, -90),
        size: rand(14, 30),
        color: pick(["#ff416c", "#ff5c8a", "#e8459c", "#ff7a9c"]),
        spin: rand(-0.6, 0.6),
        angle: rand(-0.3, 0.3),
        life: rand(3.2, 4),
      })),
    draw: (ctx, p) => {
      ctx.fillStyle = p.color;
      heart(ctx, p.size);
    },
  },
  snowfall: {
    duration: 7,
    gravity: 0,
    start: () => [],
    perSecond: 28,
    spawn: (w) => [{
      x: rand(0, w),
      y: -10,
      vx: rand(-15, 15),
      vy: rand(40, 90),
      size: rand(2, 5),
      color: isLight() ? "#9cc3ec" : "#ffffff",
      spin: rand(1, 3),
      angle: rand(0, 6),
      life: 6,
    }],
    draw: (ctx, p) => {
      ctx.fillStyle = p.color;
      ctx.beginPath();
      // The canvas is rotated by `angle`: a small sideways sway.
      ctx.arc(Math.sin(p.angle) * 6, 0, p.size, 0, Math.PI * 2);
      ctx.fill();
    },
  },
  rainfall: {
    duration: 5,
    gravity: 0,
    start: () => [],
    perSecond: 70,
    spawn: (w) => [{
      x: rand(0, w + 80),
      y: -30,
      vx: -60,
      vy: rand(650, 850),
      size: rand(12, 22),
      color: isLight() ? "rgb(40 110 200 / 0.55)" : "rgb(150 190 255 / 0.55)",
      spin: 0,
      angle: 0,
      life: 1.6,
    }],
    draw: (ctx, p) => {
      ctx.strokeStyle = p.color;
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(0, 0);
      ctx.lineTo(-p.size * 0.08, p.size);
      ctx.stroke();
    },
  },
  space_invaders: {
    duration: 5,
    gravity: 0,
    start: (w) =>
      Array.from({ length: 22 }, (_, i) => ({
        x: ((i % 11) + 0.5) * (w / 11),
        y: -40 - Math.floor(i / 11) * 60,
        vx: 40,
        vy: 70,
        size: 28,
        color: "",
        spin: 0,
        angle: 0,
        life: 5,
        glyph: "👾",
      })),
    draw: (ctx, p) => {
      ctx.font = `${p.size}px system-ui`;
      ctx.textAlign = "center";
      ctx.fillText(p.glyph!, 0, 0);
    },
  },
};

let running = false;

export function playEffect(effect: Effect) {
  const spec = SPECS[effect];
  if (!spec || running || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  running = true;

  const canvas = document.createElement("canvas");
  canvas.style.cssText = "position:fixed;inset:0;width:100%;height:100%;pointer-events:none;z-index:90";
  document.body.append(canvas);
  const ctx = canvas.getContext("2d")!;
  const dpr = Math.min(devicePixelRatio || 1, 2);
  const w = innerWidth;
  const h = innerHeight;
  canvas.width = w * dpr;
  canvas.height = h * dpr;

  const particles = spec.start(w, h);
  let elapsed = 0;
  let spawnDebt = 0;
  let last = performance.now();

  function frame(now: number) {
    const dt = Math.min((now - last) / 1000, 0.05);
    last = now;
    elapsed += dt;
    if (spec.spawn && elapsed < spec.duration - 1.5) {
      spawnDebt += (spec.perSecond ?? 0) * dt;
      for (; spawnDebt >= 1; spawnDebt--) particles.push(...spec.spawn(w, h));
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    for (let i = particles.length - 1; i >= 0; i--) {
      const p = particles[i];
      p.life -= dt;
      if (p.life <= 0 || p.y > h + 60 || p.y < -400) {
        particles.splice(i, 1);
        continue;
      }
      p.vy += spec.gravity * dt;
      p.x += p.vx * dt;
      p.y += p.vy * dt;
      p.angle += p.spin * dt;
      ctx.save();
      ctx.globalAlpha = Math.min(1, p.life * 2);
      ctx.translate(p.x, p.y);
      if (effect !== "snowfall") ctx.rotate(p.angle);
      spec.draw(ctx, p);
      ctx.restore();
    }
    if (elapsed < spec.duration + 1 && (particles.length || elapsed < spec.duration)) {
      requestAnimationFrame(frame);
    } else {
      canvas.remove();
      running = false;
    }
  }
  requestAnimationFrame(frame);
}
