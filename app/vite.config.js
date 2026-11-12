import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;
// Web version in development (scripts/dev-web.sh): plain http on the LAN, behind a reverse
// proxy that gives it an https name (sign-in and the media service worker need https).
const webHost = process.env.ALWY_DEV_HOST;
const webPort = Number(process.env.ALWY_DEV_PORT || 8090);

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [sveltekit()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // VITE_ (the default) and the Tauri CLI's build target (TAURI_ENV_PLATFORM…).
  envPrefix: ["VITE_", "TAURI_ENV_"],
  // 2. tauri expects a fixed port, fail if that port is not available
  server: webHost
    ? {
        host: "0.0.0.0",
        port: webPort,
        strictPort: true,
        allowedHosts: [webHost],
        watch: { ignored: ["**/src-tauri/**"] },
      }
    : {
        port: 1420,
        strictPort: true,
        host: host || "127.0.0.1",
        hmr: host
          ? {
              protocol: "ws",
              host,
              port: 1421,
            }
          : undefined,
        watch: {
          // 3. tell Vite to ignore watching `src-tauri`
          ignored: ["**/src-tauri/**"],
        },
      },
}));
