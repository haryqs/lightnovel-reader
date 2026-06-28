import { defineConfig } from "vite";
import { VitePWA } from "vite-plugin-pwa";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  clearScreen: false,
  server: {
    port: 3000,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  plugins: [
    VitePWA({
      registerType: "autoUpdate",
      workbox: {
        globPatterns: ["**/*.{js,css,html,wasm,png,svg,ico}"],
        maximumFileSizeToCacheInBytes: 3 * 1024 * 1024, // 3MB for WASM
      },
      manifest: {
        name: "LightNovel Reader",
        short_name: "LNR",
        description: "本地优先的开源轻小说阅读器",
        theme_color: "#3f66f2",
        background_color: "#fafcff",
        display: "standalone",
        icons: [
          { src: "/icons/32x32.png", sizes: "32x32", type: "image/png" },
          { src: "/icons/128x128.png", sizes: "128x128", type: "image/png" },
          { src: "/icons/128x128@2x.png", sizes: "256x256", type: "image/png" },
        ],
      },
    }),
  ],
}));
