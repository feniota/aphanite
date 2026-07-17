// Commonly used functions and variables across the scripts
import { type SpawnOptions, spawn } from "node:child_process";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

export const __dirname = dirname(fileURLToPath(import.meta.url));

export function run_command(command: string, args: string[], options: SpawnOptions = {}) {
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
