// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import process from "node:process";

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
      // The web version (scripts/build-web.sh) goes to its own folder.
      pages: process.env.ALWY_WEB ? "build-web" : "build",
      assets: process.env.ALWY_WEB ? "build-web" : "build",
    }),
    // Web version hosted under a sub-path (e.g. ALWY_WEB_BASE=/chat for example.org/chat/).
    paths: { base: process.env.ALWY_WEB_BASE ?? "" },
  },
};

export default config;
