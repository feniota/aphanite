import { execFileSync } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const env = process.env;

const required = ["GH_PUSH_REPO", "GH_PUSH_USER", "GH_PUSH_TOKEN", "FJ_PUSH_USER", "FJ_PUSH_TOKEN"];

for (const name of required) {
  if (!env[name]) {
    throw new Error(`Missing required environment variable: ${name}`);
  }
}

const { GH_PUSH_REPO, GH_PUSH_USER, GH_PUSH_TOKEN, FJ_PUSH_USER, FJ_PUSH_TOKEN } = env;

const SYNC_BOT_NAME = env.SYNC_BOT_NAME || "feniota-bot";
const SYNC_BOT_EMAIL = env.SYNC_BOT_EMAIL || "feniota-bot@users.noreply.github.com";
const MAX_ATTEMPTS = 5;

const GITHUB_MAIN = "main";
const GITHUB_INTEGRATION = "github";
const FORGEJO_MAIN = "main";
const FORGEJO_INTEGRATION = "github";

class FatalError extends Error {}

const redactions = [
  [GH_PUSH_TOKEN, "[GH_TOKEN]"],
  [GH_PUSH_USER, "[GH_USER]"],
  [FJ_PUSH_TOKEN, "[FJ_TOKEN]"],
  [FJ_PUSH_USER, "[FJ_USER]"],
].filter(([value]) => value);

function redact(value) {
  let result = String(value);
  for (const [secret, replacement] of redactions) {
    result = result.split(secret).join(replacement);
  }
  return result;
}

function git(...args) {
  console.log(`+ git ${args.map(redact).join(" ")}`);
  try {
    execFileSync("git", args, {
      stdio: "inherit",
      env: { ...process.env, GIT_TERMINAL_PROMPT: "0" },
    });
  } catch (error) {
    const sanitized = new Error(redact(error.message));
    sanitized.status = error.status;
    sanitized.stack = redact(error.stack ?? sanitized.stack);
    throw sanitized;
  }
}

function gitOutput(...args) {
  return execFileSync("git", args, {
    encoding: "utf8",
    env: { ...process.env, GIT_TERMINAL_PROMPT: "0" },
  }).trim();
}

function rev(ref) {
  return gitOutput("rev-parse", "--verify", ref);
}

function isAncestor(ancestor, descendant) {
  try {
    execFileSync("git", ["merge-base", "--is-ancestor", ancestor, descendant], { stdio: "ignore" });
    return true;
  } catch (error) {
    if (error.status === 1) {
      return false;
    }
    throw error;
  }
}

function branchExists(remote, branch) {
  return gitOutput("ls-remote", "--heads", remote, `refs/heads/${branch}`) !== "";
}

function withCredentials(raw, user, token) {
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

function ensureRemote(name, url) {
  try {
    execFileSync("git", ["remote", "get-url", name], { stdio: "ignore" });
  } catch {
    git("remote", "add", name, url);
    return;
  }
  git("remote", "set-url", name, url);
}

function fetchRef(remote, branch) {
  git("fetch", "--no-tags", remote, `+refs/heads/${branch}:refs/remotes/${remote}/${branch}`);
}

function mergeBranches(base, other, message) {
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

function syncOnce() {
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
    tip => tip !== null && !isAncestor(tip, forgejoMain),
  );

  let integration = null;
  for (const tip of tips) {
    if (integration === null) {
      integration = tip;
    } else if (isAncestor(tip, integration)) {
      continue;
    } else if (isAncestor(integration, tip)) {
      integration = tip;
    } else {
      integration = mergeBranches(
        integration,
        tip,
        "Merge GitHub 'github' into 'github' [skip ci]",
      );
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

function syncTags() {
  console.log("Syncing tags from Forgejo to GitHub...");

  // Fetch all tags from Forgejo
  git("fetch", "forgejo", "+refs/tags/*:refs/tags/forgejo-tags/*");

  // Get list of tags from Forgejo
  const forgejoTags = gitOutput("tag", "--list", "refs/tags/forgejo-tags/*")
    .split("\n")
    .filter(Boolean)
    .map(tag => tag.replace("refs/tags/forgejo-tags/", ""));

  if (forgejoTags.length === 0) {
    console.log("No tags found on Forgejo.");
    return;
  }

  // Fetch tags from GitHub to compare
  git("fetch", "github", "+refs/tags/*:refs/tags/*", "--force");

  // Get list of tags from GitHub
  const githubTags = new Set(
    gitOutput("tag", "--list")
      .split("\n")
      .filter(Boolean)
      .filter(tag => !tag.startsWith("forgejo-tags/"))
  );

  // Find new tags that exist on Forgejo but not on GitHub
  const newTags = forgejoTags.filter(tag => !githubTags.has(tag));

  if (newTags.length === 0) {
    console.log("No new tags to sync.");
    // Clean up temporary tag references
    for (const tag of forgejoTags) {
      try {
        git("tag", "--delete", `forgejo-tags/${tag}`);
      } catch {
        // Ignore errors if tag doesn't exist
      }
    }
    return;
  }

  console.log(`Found ${newTags.length} new tag(s) to sync: ${newTags.join(", ")}`);

  // Get the main branch commit to verify tags
  const mainCommit = rev(`refs/remotes/github/${GITHUB_MAIN}`);

  // Push new tags to GitHub
  for (const tag of newTags) {
    const tagRef = `refs/tags/forgejo-tags/${tag}`;
    const tagCommit = rev(tagRef);
    
    // Verify the tag points to a commit that exists on GitHub
    try {
      git("merge-base", "--is-ancestor", tagCommit, mainCommit);
      console.log(`Pushing tag ${tag} to GitHub...`);
      git("push", "github", `${tagRef}:refs/tags/${tag}`);
    } catch {
      console.warn(`Skipping tag ${tag}: commit not found on GitHub main branch.`);
    }
  }

  // Clean up temporary tag references
  for (const tag of forgejoTags) {
    try {
      git("tag", "--delete", `forgejo-tags/${tag}`);
    } catch {
      // Ignore errors if tag doesn't exist
    }
  }

  console.log("Tag sync completed.");
}

async function main() {
  git("config", "user.name", SYNC_BOT_NAME);
  git("config", "user.email", SYNC_BOT_EMAIL);

  const originRaw = env.FJ_PUSH_REPO || gitOutput("remote", "get-url", "origin");

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
      console.warn(`Sync attempt ${attempt} failed: ${error.message}`);
      await sleep(attempt * 2000);
    }
  }
}

main().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
