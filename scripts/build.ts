#!/usr/bin/env deno
// Build release Aphanite binary and compress all the files in a .zip file.

import { join } from "node:path";

import { __dirname, run_command } from "./common.ts";

const repoRoot = join(__dirname, "..");

interface BuildTarget {
  triple: string;
  binaryName: string;
  env?: Record<string, string>;
}

const TARGETS: BuildTarget[] = [
  { triple: "x86_64-unknown-linux-gnu", binaryName: "aphanite" },
  { triple: "x86_64-unknown-linux-musl", binaryName: "aphanite" },
  { triple: "x86_64-pc-windows-gnu", binaryName: "aphanite.exe" },
];

// Use CLI args as targets. If none given, use the predefined list.
const targets: BuildTarget[] =
  Deno.args.length > 0
    ? Deno.args.map(triple => ({
        triple,
        binaryName: triple.includes("windows") ? "aphanite.exe" : "aphanite",
      }))
    : TARGETS;

async function getVersion(): Promise<string> {
  const cmd = new Deno.Command("cargo", {
    args: ["metadata", "--format-version", "1", "--no-deps"],
    cwd: repoRoot,
  });
  const { success, stdout } = await cmd.output();
  if (!success) throw new Error("Failed to get Cargo metadata");
  const metadata = JSON.parse(new TextDecoder().decode(stdout));
  return metadata.packages[0].version;
}

void (async () => {
  const version = await getVersion();
  const distDir = join(repoRoot, "dist");

  console.info(`[1/3] Installing npm dependencies...`);
  await run_command("deno", ["install"], { cwd: repoRoot });

  console.info(`[2/3] Building frontend...`);
  await run_command("deno", ["x", "vite", "build", "./web/"], { cwd: repoRoot });

  // Create dist directory
  await Deno.mkdir(distDir, { recursive: true });

  for (const target of targets) {
    console.info(`[3/3] Building ${target.triple}...`);

    const cargoEnv = target.env ? { ...Deno.env.toObject(), ...target.env } : undefined;

    await run_command("cargo", ["build", "--release", "--target", target.triple], {
      cwd: repoRoot,
      ...(cargoEnv ? { env: cargoEnv } : {}),
    });

    const zipName = `aphanite-${version}-${target.triple}.zip`;
    const zipPath = join(distDir, zipName);
    const binaryPath = join(repoRoot, "target", target.triple, "release", target.binaryName);

    await run_command(
      "zip",
      ["-j", zipPath, binaryPath, join(repoRoot, "README.md"), join(repoRoot, "LICENSE")],
      {
        cwd: repoRoot,
      },
    );

    console.info(`  -> ${zipName} created`);
  }

  console.info("\nAll done! Packages:");
  for (const target of targets) {
    console.info(`  dist/aphanite-${version}-${target.triple}.zip`);
  }
})();
