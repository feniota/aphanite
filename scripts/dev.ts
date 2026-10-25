#!/usr/bin/env node
// Starts Aphanite and Vite dev server at the same time

import { type SpawnOptions, spawn } from "node:child_process";
import { join } from "node:path";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

function run_command(command: string, args: string[], options: SpawnOptions = {}) {
  return new Promise<void>((resolve, reject) => {
    const child = spawn(command, args, {
      stdio: "inherit",
      ...options,
    });

    child.on("close", (code: number | null) => {
      if (code !== 0) {
        reject(new Error(`Command "${command} ${args.join(" ")}" exits with code ${code}`));
        return;
      }
      resolve();
    });

    child.on("error", err => {
      reject(
        new Error(`Error occurred when executing "${command} ${args.join(" ")}": ${err.message}`),
      );
    });
  });
}

process.chdir(join(__dirname, ".."));

function sleep(timeout: number) {
  return new Promise(resolve => {
    setTimeout(resolve, timeout);
  });
}

async function check_command_exists(command: string) {
  const isWindows = process.platform === "win32";
  try {
    if (isWindows) {
      await run_command("cmd.exe", ["/c", "where", command], {
        stdio: "ignore",
      });
    } else {
      await run_command("sh", ["-c", `command -v ${command}`], {
        stdio: "ignore",
      });
    }
    return true;
  } catch {
    return false;
  }
}
void (async () => {
  const isWindows = process.platform === "win32";
  const pnpm = isWindows ? "pnpm.cmd" : "pnpm";

  if (!(await check_command_exists("bacon"))) {
    console.error("[!] Please install Bacon first with:");
    console.error("    cargo install --locked bacon");
  }

  console.warn("[!] Installing dependencies...");
  try {
    await run_command(pnpm, ["install"]);
    console.warn("[!] Dependencies installed.");
  } catch (installError) {
    console.error(`[!] Failed to install dependencies:`, installError);
    process.exit(1);
  }

  if (!isWindows) {
    // 检测 tmux
    if (!(await check_command_exists("tmux"))) {
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
    await run_command("tmux", ["new-session", "-s", "aphanite-dev", "-d", "bacon run-long"]);
    await run_command("tmux", [
      "split-window",
      "-h",
      "-t",
      "aphanite-dev",
      "pnpm exec vite dev ./web/",
    ]);
    await run_command("tmux", ["attach-session", "-t", "aphanite-dev"]);
  } else {
    console.warn("[!] 启动 Windows Terminal...");
    await run_command("wt", [
      "new-tab",
      "-d",
      ".",
      pnpm,
      "exec",
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
