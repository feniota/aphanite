import { execFileSync } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

function require_env(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`Missing required environment variable: ${name}`);
  }
  return value;
}

function error_text(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function exec_status(error: unknown): number | null | undefined {
  if (typeof error === "object" && error !== null && "status" in error) {
    return (error as { status?: number | null }).status;
  }
  return undefined;
}

const GH_PUSH_REPO = require_env("GH_PUSH_REPO");
const GH_PUSH_USER = require_env("GH_PUSH_USER");
const GH_PUSH_TOKEN = require_env("GH_PUSH_TOKEN");
const FJ_PUSH_USER = require_env("FJ_PUSH_USER");
const FJ_PUSH_TOKEN = require_env("FJ_PUSH_TOKEN");

const SYNC_BOT_NAME = process.env.SYNC_BOT_NAME || "feniota-bot";
const SYNC_BOT_EMAIL = process.env.SYNC_BOT_EMAIL || "feniota-bot@users.noreply.github.com";
const MAX_ATTEMPTS = 5;

const GITHUB_MAIN = "main";
const GITHUB_INTEGRATION = "github";
const FORGEJO_MAIN = "main";
const FORGEJO_INTEGRATION = "github";

class FatalError extends Error {}

const redactions: [string, string][] = [
  [GH_PUSH_TOKEN, "[GH_TOKEN]"],
  [GH_PUSH_USER, "[GH_USER]"],
  [FJ_PUSH_TOKEN, "[FJ_TOKEN]"],
  [FJ_PUSH_USER, "[FJ_USER]"],
];

function redact(value: unknown): string {
  let result = String(value);
  for (const [secret, replacement] of redactions) {
    result = result.split(secret).join(replacement);
  }
  return result;
}

function git(...args: string[]): void {
  console.log(`+ git ${args.map(redact).join(" ")}`);
  try {
    execFileSync("git", args, {
      stdio: "inherit",
      env: { ...process.env, GIT_TERMINAL_PROMPT: "0" },
    });
  } catch (error) {
    const sanitized = Object.assign(new Error(redact(error_text(error))), {
      status: exec_status(error),
    });
    sanitized.stack = redact((error instanceof Error ? error.stack : undefined) ?? sanitized.stack);
    throw sanitized;
  }
}

function gitOutput(...args: string[]): string {
  return execFileSync("git", args, {
    encoding: "utf8",
    env: { ...process.env, GIT_TERMINAL_PROMPT: "0" },
  }).trim();
}

function rev(ref: string): string {
  return gitOutput("rev-parse", "--verify", ref);
}

function isAncestor(ancestor: string, descendant: string): boolean {
  try {
    execFileSync("git", ["merge-base", "--is-ancestor", ancestor, descendant], { stdio: "ignore" });
    return true;
  } catch (error) {
    if (exec_status(error) === 1) {
      return false;
    }
    throw error;
  }
}

function branchExists(remote: string, branch: string): boolean {
  return gitOutput("ls-remote", "--heads", remote, `refs/heads/${branch}`) !== "";
}

function remoteTags(remote: string): Set<string> {
  return new Set(
    gitOutput("ls-remote", "--tags", "--refs", remote, "refs/tags/*")
      .split("\n")
      .filter(Boolean)
      .map(line => line.split("\t")[1].replace(/^refs\/tags\//, "")),
  );
}

function withCredentials(raw: string, user: string, token: string): string {
  if (!/^(https?|file):\/\//.test(raw)) {
    throw new Error(`Remote URL must be HTTP(S) or file://, got: ${redact(raw)}`);
  }
  const url = new URL(raw);
  if (url.protocol === "http:" || url.protocol === "https:") {
    url.username = user;
    url.password = token;
  }
  return url.toString();
}

function ensureRemote(name: string, url: string): void {
  try {
    execFileSync("git", ["remote", "get-url", name], { stdio: "ignore" });
  } catch {
    git("remote", "add", name, url);
    return;
  }
  git("remote", "set-url", name, url);
}

function fetchRef(remote: string, branch: string): void {
  git("fetch", "--no-tags", remote, `+refs/heads/${branch}:refs/remotes/${remote}/${branch}`);
}

function mergeBranches(base: string, other: string, message: string): string {
  git("checkout", "-B", "sync-temp", base);
  try {
    git("merge", "--no-ff", "-m", message, other);
  } catch {
    try {
      git("merge", "--abort");
    } catch {}
    throw new FatalError(
      `Merge conflict while combining ${base} and ${other}; resolve it in the open Forgejo PR.`,
    );
  }
  return rev("HEAD");
}

function syncOnce(): void {
  const githubHasMain = branchExists("github", GITHUB_MAIN);
  if (githubHasMain) {
    fetchRef("github", GITHUB_MAIN);
  }

  const githubHasIntegration = branchExists("github", GITHUB_INTEGRATION);
  if (githubHasIntegration) {
    fetchRef("github", GITHUB_INTEGRATION);
  }

  fetchRef("forgejo", FORGEJO_MAIN);

  const forgejoHasIntegration = branchExists("forgejo", FORGEJO_INTEGRATION);
  if (forgejoHasIntegration) {
    fetchRef("forgejo", FORGEJO_INTEGRATION);
  }

  const githubMain = githubHasMain ? rev(`refs/remotes/github/${GITHUB_MAIN}`) : null;
  const githubIntegration = githubHasIntegration
    ? rev(`refs/remotes/github/${GITHUB_INTEGRATION}`)
    : null;
  const forgejoMain = rev(`refs/remotes/forgejo/${FORGEJO_MAIN}`);
  let forgejoIntegration = forgejoHasIntegration
    ? rev(`refs/remotes/forgejo/${FORGEJO_INTEGRATION}`)
    : null;

  if (githubMain !== forgejoMain) {
    if (githubMain !== null && !isAncestor(githubMain, forgejoMain)) {
      throw new FatalError(
        "GitHub main has commits not present in Forgejo main; refusing to overwrite it.",
      );
    }
    git("push", "github", `${forgejoMain}:refs/heads/${GITHUB_MAIN}`);
  }

  // Start from main, then advance the integration branch to include both
  // remotes' github tips. Tips already contained in main add no commit.
  const tips = [forgejoIntegration, githubIntegration].filter(
    (tip): tip is string => tip !== null && !isAncestor(tip, forgejoMain),
  );

  let integration: string | null = null;
  for (const tip of tips) {
    const current = integration;
    if (current === null) {
      integration = tip;
    } else if (isAncestor(tip, current)) {
      continue;
    } else if (isAncestor(current, tip)) {
      integration = tip;
    } else {
      integration = mergeBranches(current, tip, "Merge GitHub 'github' into 'github' [skip ci]");
    }
  }

  let next = forgejoMain;
  if (integration !== null) {
    if (!isAncestor(forgejoMain, integration)) {
      integration = mergeBranches(forgejoMain, integration, "Sync 'github' with 'main' [skip ci]");
    }
    next = integration;
  }

  if (githubIntegration !== next) {
    git("push", "github", `${next}:refs/heads/${GITHUB_INTEGRATION}`);
  }
  if (forgejoIntegration !== next) {
    git("push", "forgejo", `${next}:refs/heads/${FORGEJO_INTEGRATION}`);
  }

  console.log(
    `Synced github=${next} (main=${forgejoMain}, previous Forgejo github=${forgejoIntegration ?? "missing"})`,
  );

  syncTags();
}

function syncTags(): void {
  console.log("Syncing tags from Forgejo to GitHub...");

  const forgejoTags = remoteTags("forgejo");
  if (forgejoTags.size === 0) {
    console.log("No tags found on Forgejo.");
    return;
  }

  const githubTags = remoteTags("github");
  const newTags = [...forgejoTags].filter(tag => !githubTags.has(tag));

  if (newTags.length === 0) {
    console.log("No new tags to sync.");
    return;
  }

  console.log(`Found ${newTags.length} new tag(s) to sync: ${newTags.join(", ")}`);

  const mainCommit = rev(`refs/remotes/github/${GITHUB_MAIN}`);
  const tempRefs: string[] = [];

  try {
    for (const tag of newTags) {
      // Fetch into a dedicated namespace (not refs/tags/<tag>) so tags already
      // present locally from the Forgejo checkout are never clobbered. The ref
      // keeps pointing at the tag object, preserving annotated tag messages.
      const tempRef = `refs/tags/forgejo-tags/${tag}`;
      git("fetch", "--no-tags", "forgejo", `+refs/tags/${tag}:${tempRef}`);
      tempRefs.push(tempRef);

      let tagCommit: string;
      try {
        tagCommit = rev(`${tempRef}^{commit}`);
      } catch {
        console.warn(`Skipping tag ${tag}: ref does not point to a commit.`);
        continue;
      }

      if (!isAncestor(tagCommit, mainCommit)) {
        console.warn(`Skipping tag ${tag}: commit not found on GitHub main branch.`);
        continue;
      }

      console.log(`Pushing tag ${tag} to GitHub...`);
      git("push", "github", `${tempRef}:refs/tags/${tag}`);
    }
  } finally {
    for (const ref of tempRefs) {
      try {
        git("update-ref", "-d", ref);
      } catch {
        // Ignore cleanup errors
      }
    }
  }

  console.log("Tag sync completed.");
}

async function main(): Promise<void> {
  git("config", "user.name", SYNC_BOT_NAME);
  git("config", "user.email", SYNC_BOT_EMAIL);

  const originRaw = process.env.FJ_PUSH_REPO || gitOutput("remote", "get-url", "origin");

  ensureRemote("github", withCredentials(GH_PUSH_REPO, GH_PUSH_USER, GH_PUSH_TOKEN));
  ensureRemote("forgejo", withCredentials(originRaw, FJ_PUSH_USER, FJ_PUSH_TOKEN));

  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt += 1) {
    try {
      syncOnce();
      return;
    } catch (error) {
      if (error instanceof FatalError || attempt === MAX_ATTEMPTS) {
        throw error;
      }
      console.warn(`Sync attempt ${attempt} failed: ${error_text(error)}`);
      await sleep(attempt * 2000);
    }
  }
}

main().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
