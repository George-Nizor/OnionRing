import { defineConfig, loadEnv } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";
// @ts-expect-error type error without @types/node package
import { createReadStream, statSync } from "node:fs";
// @ts-expect-error type error without @types/node package
import { resolve, relative, isAbsolute, extname } from "node:path";
const host = process.env.TAURI_DEV_HOST;

/** @type {Record<string, string>} */
const TYPES = { ".json": "application/json", ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".webp": "image/webp" };

/**
 * Browser preview only: serves `<data root>` read-only at /__onionring/ (see src/lib/mock.ts).
 * @param {string | undefined} root
 * @returns {import("vite").Plugin}
 */
function dataFolder(root) {
  return {
    name: "onionring-data",
    configureServer(server) {
      if (!root) return;
      server.middlewares.use("/__onionring", (req, res, next) => {
        const url = /** @type {{ url?: string }} */ (/** @type {unknown} */ (req)).url ?? "/";
        const rel = decodeURIComponent(url.split("?")[0]).replace(/^\/+/, "");
        const file = resolve(root, rel);
        const inside = relative(root, file);
        if (!inside || inside.startsWith("..") || isAbsolute(inside)) return next();
        try {
          if (!statSync(file).isFile()) return next();
        } catch {
          return next();
        }
        res.setHeader("Content-Type", TYPES[extname(file).toLowerCase()] ?? "application/octet-stream");
        // The snapshot changes whenever it's regenerated; art doesn't.
        res.setHeader("Cache-Control", extname(file).toLowerCase() === ".json" ? "no-store" : "max-age=3600");
        createReadStream(file).pipe(res);
      });
    },
  };
}

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const data = loadEnv(mode, process.cwd(), "VITE_").VITE_ONIONRING_DATA;
  return {
    plugins: [dataFolder(data), sveltekit()],
    // Keep Rust errors visible in `tauri dev`.
    clearScreen: false,
    server: {
      // Tauri expects a fixed port.
      port: 1420,
      strictPort: true,
      host: host || "127.0.0.1",
      hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
      watch: {
        ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"],
      },
    },
  };
});
