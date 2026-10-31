// Integration tests: real Vite sockets + a fake cargo process tree, no GUI/build.
// Run: node --test frontend/scripts/unix-dev.test.mjs
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import os from "node:os";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const rootDir = fileURLToPath(new URL("../../", import.meta.url));
const launcher = path.join(rootDir, "script/unix_dev.sh");

async function until(check) {
  for (let attempt = 0; attempt < 200; attempt++) {
    const result = await check();
    if (result) return result;
    await delay(50);
  }
  throw new Error("Timed out waiting for dev launcher");
}

async function fixture(t, env = {}) {
  const dir = await mkdtemp(path.join(os.tmpdir(), "artcraftx-dev-test-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  await writeFile(path.join(dir, "cargo"), `#!/usr/bin/env node
const fs = require("node:fs");
const { spawn } = require("node:child_process");
if (process.argv.includes("--version")) process.exit(0);
const child = spawn(process.execPath, ["-e", "setInterval(() => {}, 1000)"], { stdio: "ignore" });
fs.writeFileSync(process.env.TEST_RECORD, JSON.stringify({
  args: process.argv.slice(2), env: process.env, cwd: process.cwd(),
  pid: process.pid, childPid: child.pid,
}));
if (process.env.TEST_EXIT_CODE) setTimeout(() => process.exit(Number(process.env.TEST_EXIT_CODE)), 200);
setInterval(() => {}, 1000);
`, { mode: 0o755 });

  const recordFile = path.join(dir, "record.json");
  const proc = spawn(launcher, [], {
    // Prove the launcher is independent of the invoking working directory.
    cwd: dir,
    env: { ...process.env, ...env, PATH: `${dir}:${process.env.PATH}`, TEST_RECORD: recordFile },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  proc.stdout.on("data", (chunk) => { output += chunk; });
  proc.stderr.on("data", (chunk) => { output += chunk; });
  const exited = once(proc, "exit");
  t.after(async () => {
    if (proc.exitCode === null && proc.signalCode === null) proc.kill("SIGTERM");
    await exited;
  });
  return {
    proc, exited, output: () => output,
    async record() {
      return until(async () => {
        if (proc.exitCode !== null) throw new Error(output);
        try { return JSON.parse(await readFile(recordFile, "utf8")); }
        catch (error) { if (error.code !== "ENOENT") throw error; }
      });
    },
  };
}

async function occupyPort(t) {
  const server = createServer();
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  t.after(() => new Promise((resolve) => server.close(resolve)));
  return server;
}

function alive(pid) {
  try { process.kill(pid, 0); return true; }
  catch (error) { if (error.code === "ESRCH") return false; throw error; }
}

function overrides(record) {
  return JSON.parse(record.args.at(-1));
}

test("skips occupied ports, pairs Tauri with real Vite/HMR, and cleans up on Ctrl-C", { timeout: 20000 }, async (t) => {
  const occupied = await occupyPort(t);
  const startingPort = occupied.address().port;
  const run = await fixture(t, { ARTCRAFTX_DEV_PORT: String(startingPort) });
  const record = await run.record();
  const config = overrides(record);
  const url = config.build.devUrl;
  assert.ok(Number(new URL(url).port) > startingPort);
  assert.equal(new URL(url).hostname, "127.0.0.1");
  assert.equal(config.build.beforeDevCommand, "");
  assert.ok(!record.args.includes("--no-watch"));
  assert.ok(!record.args.includes("--no-dev-server-wait"));
  assert.equal(record.env.TAURI_APP_PATH, path.join(rootDir, "crates/artcraftx"));
  assert.equal(record.env.TAURI_FRONTEND_PATH, path.join(rootDir, "frontend"));
  assert.ok(config.app.security.devCsp["connect-src"].includes(url.replace("http:", "ws:")));
  assert.match(await (await fetch(url)).text(), /\/src\/index\.tsx/);
  assert.match(await (await fetch(`${url}/@vite/client`)).text(), /WebSocket/);

  run.proc.kill("SIGINT");
  assert.deepEqual(await run.exited, [130, null], run.output());
  await until(() => !alive(record.pid) && !alive(record.childPid));
  await assert.rejects(fetch(url));
  assert.equal(occupied.listening, true, "must not terminate the existing listener");
});

test("Tauri failure propagates and stops Vite and leftover descendants", { timeout: 20000 }, async (t) => {
  const run = await fixture(t, { TEST_EXIT_CODE: "17" });
  const record = await run.record();
  assert.deepEqual(await run.exited, [17, null], run.output());
  await until(() => !alive(record.childPid));
  await assert.rejects(fetch(overrides(record).build.devUrl));
});

test("SIGTERM shuts down the complete process tree", { timeout: 20000 }, async (t) => {
  const run = await fixture(t);
  const record = await run.record();
  run.proc.kill("SIGTERM");
  assert.deepEqual(await run.exited, [143, null], run.output());
  await until(() => !alive(record.pid) && !alive(record.childPid));
  await assert.rejects(fetch(overrides(record).build.devUrl));
});

test("invalid ports fail before starting Tauri", { timeout: 20000 }, async (t) => {
  const run = await fixture(t, { ARTCRAFTX_DEV_PORT: "65536" });
  assert.deepEqual(await run.exited, [1, null]);
  assert.match(run.output(), /ARTCRAFTX_DEV_PORT must be an integer/);
});
