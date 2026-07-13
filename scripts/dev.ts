#!/usr/bin/env deno
// Starts Aphanite and Vite dev server at the same time
import { join } from "node:path";

import { __dirname, runCommand } from "./common.ts";

process.chdir(join(__dirname, ".."));

function sleep(timeout: number) {
  return new Promise((resolve) => {
    setTimeout(resolve, timeout);
  });
}

async function checkCommandExists(command: string) {
  const isWindows = process.platform === "win32";
  try {
    if (isWindows) {
      await runCommand("cmd.exe", ["/c", "where", command], {
        stdio: "ignore",
      });
    } else {
      await runCommand("sh", ["-c", `command -v ${command}`], {
        stdio: "ignore",
      });
    }
    return true;
  } catch {
    return false;
  }
}
(async () => {
  const isWindows = process.platform === "win32";

  if (!(await checkCommandExists("bacon"))) {
    console.error("[!] Please install Bacon first with:");
    console.error("    cargo install --locked bacon");
  }

  console.warn("[!] Installing NPM dependencies...");
  try {
    await (isWindows ? runCommand("deno.exe", ["install"]) : runCommand("deno", ["install"]));
    console.warn("[!] NPM dependencies installed。");
  } catch (installError) {
    console.error(`[!] Failed to install dependencies: ${installError}`);
    process.exit(1);
  }

  if (!isWindows) {
    // 检测 tmux
    if (!(await checkCommandExists("tmux"))) {
      console.error("[!] tmux not found, please install it first:");
      console.error("    Ubuntu/Debian: sudo apt install tmux");
      console.error("    Fedora: sudo dnf install tmux");
      console.error("    macOS: brew install tmux");
      process.exit(1);
    }

    // 启动 tmux 会话，左右分屏
    console.warn("[!] Press Ctrl-B then D to temporarily detach from the tmux session.");
    console.warn("[!] You can then attach to it using `tmux attach`.");
    await sleep(1500);

    // 创建 tmux 会话并设置左右分屏
    await runCommand("tmux", ["new-session", "-s", "aphanite-dev", "-d", "bacon run-long"]);
    await runCommand("tmux", [
      "split-window",
      "-h",
      "-t",
      "aphanite-dev",
      "deno x vite dev ./web/",
    ]);
    await runCommand("tmux", ["attach-session", "-t", "aphanite-dev"]);
  } else {
    console.warn("[!] 启动 Windows Terminal...");
    await runCommand("wt", [
      "new-tab",
      "-d",
      ".",
      "deno",
      "x",
      "vite",
      "dev",
      ".\\web\\",
      ";",
      "new-tab",
      "-d",
      ".\\",
      "bacon.exe",
      "run-long",
    ]);
  }
})();
