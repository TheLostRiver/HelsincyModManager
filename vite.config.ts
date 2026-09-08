import react from "@vitejs/plugin-react";
import { relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const nonFrontendDirectories = new Set([
  "src-tauri",
  "target",
  ".worktrees",
  ".claude",
  ".codex",
  ".agents",
  ".planning",
  ".tmp",
  "tmp",
  ".vite",
  "armor-data",
]);

export function createDevWatchIgnored(rootDirectory: string) {
  return (watchedPath: string) => {
    // Prune known repository-root trees, not same-named frontend subdirectories.
    const [topLevelDirectory] = relative(rootDirectory, watchedPath).split(sep);
    return nonFrontendDirectories.has(topLevelDirectory);
  };
}

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    // 必须显式绑 IPv4：不设 host 时 vite 取默认值 `localhost`，Node 17+ 会把它
    // 解析成 IPv6 `::1` 并只监听该地址；而 Tauri 的 WebView2 解析 localhost 优先
    // 走 IPv4 `127.0.0.1`，连不上就是一片白屏，且控制台无任何报错。
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    watch: {
      ignored: createDevWatchIgnored(fileURLToPath(new URL(".", import.meta.url))),
    },
  },
});
