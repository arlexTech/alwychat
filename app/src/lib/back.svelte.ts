// Android back gesture: closes the most recently opened layer (viewer, sheet, menu, chat…).
// With nothing open we let go of it, so Android's default runs: on Android 12+ the app moves
// to the background (still alive) instead of being closed.
import { hasBackend, interceptBack, isAndroid, isBubble } from "$lib/backend";

const layers: (() => void)[] = [];
let release: Promise<() => Promise<void>> | null = null;

function sync() {
  // A chat bubble's activity asks the page itself (BubbleActivity: no layer open = fold it).
  if (!hasBackend || !isAndroid || isBubble) return;
  if (layers.length && !release) {
    release = interceptBack(() => layers.at(-1)?.());
  } else if (!layers.length && release) {
    const done = release;
    release = null;
    void done.then((unregister) => unregister());
  }
}

/** Register an open layer; `close` runs on back. Returns the cleanup (use as an $effect return). */
export function onBack(close: () => void): () => void {
  layers.push(close);
  sync();
  return () => {
    const i = layers.lastIndexOf(close);
    if (i >= 0) layers.splice(i, 1);
    sync();
  };
}

if (isBubble)
  (globalThis as { __alwyBack?: () => boolean }).__alwyBack = () => {
    const close = layers.at(-1);
    close?.();
    return !!close;
  };
