// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  // Inject component CSS from JS instead of separate virtual CSS modules: with Vite 8 and
  // vite-plugin-svelte 7.3, those modules sometimes resolve to the raw .svelte source,
  // breaking layout and scoping ("failed to load virtual css module").
  vitePlugin: { emitCss: false },
  kit: {
    adapter: adapter({
      fallback: "index.html",
    }),
  },
};

export default config;
