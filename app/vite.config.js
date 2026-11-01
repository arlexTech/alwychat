import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";
// @ts-expect-error type error without @types/node package
import fs from "node:fs";
const host = process.env.TAURI_DEV_HOST;
// Web version in development (scripts/dev-web.sh): https on its own name, sign-in needs it.
const webHost = process.env.ALWY_DEV_HOST;
const webCert = process.env.ALWY_DEV_CERT;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [sveltekit()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: webHost
    ? {
        host: "127.0.0.1",
        port: 8443,
        strictPort: true,
        allowedHosts: [webHost],
        https: {
          key: fs.readFileSync(`${webCert}/${webHost}.key`),
          cert: fs.readFileSync(`${webCert}/${webHost}.crt`),
        },
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
