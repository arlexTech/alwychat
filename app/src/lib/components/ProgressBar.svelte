<script lang="ts">
  // A thin progress bar: `value` 0–1, or null when the total isn't known (moving stripe).
  let { value, label }: { value: number | null; label?: string } = $props();
</script>

<div
  class="bar"
  class:unknown={value === null}
  role="progressbar"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={value === null ? undefined : Math.round(value * 100)}
>
  <span style:width={value === null ? undefined : `${Math.max(0, Math.min(1, value)) * 100}%`}></span>
</div>

<style>
  .bar {
    position: relative;
    height: 6px;
    border-radius: 3px;
    background: var(--surface-hover);
    overflow: hidden;
  }
  .bar span {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: 3px;
    background: var(--accent);
    transition: width 0.2s linear;
  }
  .bar.unknown span {
    width: 35%;
    animation: slide 1.1s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(290%);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .bar.unknown span {
      animation: none;
      width: 100%;
      opacity: 0.5;
    }
  }
</style>
