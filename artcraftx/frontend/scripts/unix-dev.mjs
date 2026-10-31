// Invoked by script/unix_dev.sh. Keep this under frontend so Node resolves the
// workspace's installed Vite, without requiring a global Nx or another install.
import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";

const rootDir = fileURLToPath(new URL("../../", import.meta.url));
const frontendDir = path.join(rootDir, "frontend");
const appDir = path.join(frontendDir, "apps/artcraft");
const rustDir = path.join(rootDir, "crates/artcraftx");
const host = "127.0.0.1";
const startingPort = Number(process.env.ARTCRAFTX_DEV_PORT ?? "5183");

let vite;
let rust;
let stopping = false;

// cargo-tauri, cargo, rustc, and the app inherit this dedicated Unix process
// group. Never find or kill processes by port/name: ArtCraft may be running too.
function signalRustGroup(signal) {
  if (!rust?.pid) return false;
  try {
    process.kill(-rust.pid, signal);
    return true;
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
    return false;
  }
}

async function stop(exitCode) {
  if (stopping) return;
  stopping = true;
  console.log("\n[artcraftx] Stopping frontend and Rust/Tauri...");

  // Bound shutdown even when a compiler, webview, or Vite plugin gets stuck.
  const deadline = setTimeout(() => {
    signalRustGroup("SIGKILL");
    process.exit(exitCode);
  }, 5000);
  signalRustGroup("SIGTERM");
  await Promise.allSettled([
    vite?.close(),
    (async () => {
      while (signalRustGroup(0)) await delay(50);
    })(),
  ]);
  clearTimeout(deadline);
  process.exit(exitCode);
}

function fail(error) {
  console.error(`[artcraftx] ${error.stack ?? error}`);
  void stop(1);
}

process.on("SIGINT", () => void stop(130));
process.on("SIGTERM", () => void stop(143));
process.on("SIGHUP", () => void stop(129));
process.on("uncaughtException", fail);
process.on("unhandledRejection", fail);

async function main() {
  if (!Number.isInteger(startingPort) || startingPort < 1024 || startingPort > 65535) {
    throw new Error("ARTCRAFTX_DEV_PORT must be an integer between 1024 and 65535.");
  }

  process.env.VITE_ENVIRONMENT_TYPE ??= "production";
  process.chdir(appDir);
  vite = await createServer({
    configFile: path.join(appDir, "vite.config.ts"),
    clearScreen: false,
    server: {
      host,
      port: startingPort,
      // Vite retries EADDRINUSE on successive ports while actually binding the
      // server. There is no probe/release/rebind window for a competing app.
      strictPort: false,
      open: false,
      hmr: true,
    },
  });
  if (stopping) return;
  await vite.listen();
  const { port } = vite.httpServer.address();
  const devUrl = `http://${host}:${port}`;

  // On Vite config edits, restart on this same port or fail visibly. Silently
  // moving again would leave Tauri pointing at the old (possibly foreign) app.
  for (const serverOptions of [vite.config.server, vite.config.inlineConfig.server]) {
    serverOptions.port = port;
    serverOptions.strictPort = true;
  }

  const configPath = path.join(rustDir, "tauri-dev-hot-reload.conf.json");
  const config = JSON.parse(await readFile(configPath, "utf8"));
  if (stopping) return;
  const csp = config.app.security.csp;
  const overrides = {
    build: { devUrl, beforeDevCommand: "" },
    app: {
      security: {
        devCsp: {
          ...csp,
          "connect-src": [...csp["connect-src"], devUrl, `ws://${host}:${port}`],
        },
      },
    },
  };

  console.log(`\n[artcraftx] Frontend + HMR: ${devUrl}`);
  console.log("[artcraftx] Rust/Tauri: native IPC; automatic rebuild and app restart enabled.");
  console.log("[artcraftx] Press Ctrl-C to stop both.\n");

  rust = spawn("cargo", [
    "tauri", "dev",
    "--no-dev-server",
    "--config", configPath,
    "--config", JSON.stringify(overrides),
  ], {
    cwd: rootDir,
    // Start a separate process group, but keep all output in this terminal.
    detached: true,
    stdio: ["ignore", "inherit", "inherit"],
    env: {
      ...process.env,
      TAURI_FRONTEND_PATH: frontendDir,
      TAURI_APP_PATH: rustDir,
      SQLX_OFFLINE: process.env.SQLX_OFFLINE ?? "true",
      RUSTFLAGS: process.env.RUSTFLAGS ?? "-Awarnings",
      WEBKIT_DISABLE_DMABUF_RENDERER: process.env.WEBKIT_DISABLE_DMABUF_RENDERER ?? "1",
      WEBKIT_DISABLE_COMPOSITING_MODE: process.env.WEBKIT_DISABLE_COMPOSITING_MODE ?? "1",
    },
  });
  rust.on("error", fail);
  rust.on("exit", (code, signal) => {
    if (!stopping) {
      console.log(`[artcraftx] Rust/Tauri exited (${signal ?? code}).`);
      void stop(code ?? 1);
    }
  });
}

main().catch(fail);
