import { execFileSync } from "node:child_process";

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

const FORGEJO_BASE_URL = require_env("FORGEJO_BASE_URL");
const FORGEJO_OWNER = require_env("FORGEJO_OWNER");
const FORGEJO_REPO = require_env("FORGEJO_REPO");
const FORGEJO_TOKEN = require_env("FORGEJO_TOKEN");
const FORGEJO_OPERATOR = require_env("FORGEJO_OPERATOR");
const GH_PR_NUMBER = process.env.GH_PR_NUMBER;
const GH_PR_URL = process.env.GH_PR_URL;

const SOURCE_BRANCH = "github";
const TARGET_BRANCH = "main";
const MAX_ATTEMPTS = 5;

const base_url = new URL(FORGEJO_BASE_URL);
const api_url = new URL(`/api/v1/repos/${FORGEJO_OWNER}/${FORGEJO_REPO}/`, base_url);
const git_url = new URL(
  process.env.FORGEJO_GIT_URL || `/${FORGEJO_OWNER}/${FORGEJO_REPO}.git`,
  base_url,
);
if (git_url.protocol === "http:" || git_url.protocol === "https:") {
  git_url.username = FORGEJO_OPERATOR;
  git_url.password = FORGEJO_TOKEN;
}

interface ForgejoPullRequest {
  number: number;
  html_url: string;
}

class FatalError extends Error {}

class ApiError extends Error {
  status: number;

  constructor(message: string, status: number) {
    super(message);
    this.status = status;
  }
}

const redactions: [string, string][] = [
  [FORGEJO_TOKEN, "[TOKEN]"],
  [FORGEJO_OPERATOR, "[OPERATOR]"],
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

function hasChanges(base: string, head: string): boolean {
  try {
    execFileSync("git", ["diff", "--quiet", `${base}...${head}`], {
      stdio: "ignore",
    });
    return false;
  } catch (error) {
    if (exec_status(error) === 1) {
      return true;
    }
    throw error;
  }
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

async function api<T>(url: URL, options: RequestInit = {}): Promise<T | null> {
  const response = await fetch(url, {
    ...options,
    headers: {
      Accept: "application/json",
      Authorization: `token ${FORGEJO_TOKEN}`,
      ...(options.body ? { "Content-Type": "application/json" } : {}),
      ...options.headers,
    },
  });

  const text = await response.text();

  if (!response.ok) {
    throw new ApiError(
      `Forgejo API ${response.status} ${response.statusText}: ${text}`,
      response.status,
    );
  }

  return text ? (JSON.parse(text) as T) : null;
}

async function findOpenPr(): Promise<ForgejoPullRequest[]> {
  const url = new URL("pulls", api_url);
  url.searchParams.set("state", "open");
  url.searchParams.set("head", `${FORGEJO_OWNER}:${SOURCE_BRANCH}`);
  url.searchParams.set("base", TARGET_BRANCH);
  url.searchParams.set("limit", "50");
  return (await api<ForgejoPullRequest[]>(url)) ?? [];
}

function prBody(tip: string): string {
  const lines = [
    "Synchronizes commits merged into the public GitHub `github` branch.",
    "",
    `Source tip: \`${tip}\``,
  ];

  if (GH_PR_NUMBER) {
    const link = GH_PR_URL ? ` (${GH_PR_URL})` : "";
    lines.push(`GitHub PR: GH-${GH_PR_NUMBER}${link}`);
  }

  lines.push("", "Merge with a merge commit. Do not squash or rebase.");
  return lines.join("\n");
}

async function syncOnce(): Promise<void> {
  git(
    "fetch",
    "--no-tags",
    "origin",
    `+refs/heads/${SOURCE_BRANCH}:refs/remotes/origin/${SOURCE_BRANCH}`,
  );

  const githubTip = rev(`refs/remotes/origin/${SOURCE_BRANCH}`);
  const forgejoHasBranch =
    gitOutput("ls-remote", "--heads", "forgejo", `refs/heads/${SOURCE_BRANCH}`) !== "";

  const refspecs = [`+refs/heads/${TARGET_BRANCH}:refs/remotes/forgejo/${TARGET_BRANCH}`];
  if (forgejoHasBranch) {
    refspecs.push(`+refs/heads/${SOURCE_BRANCH}:refs/remotes/forgejo/${SOURCE_BRANCH}`);
  }
  git("fetch", "--no-tags", "forgejo", ...refspecs);

  let forgejoTip: string | null = forgejoHasBranch
    ? rev(`refs/remotes/forgejo/${SOURCE_BRANCH}`)
    : null;

  if (forgejoTip === null) {
    git("push", "forgejo", `${githubTip}:refs/heads/${SOURCE_BRANCH}`);
    forgejoTip = githubTip;
  } else if (!isAncestor(githubTip, forgejoTip)) {
    if (!isAncestor(forgejoTip, githubTip)) {
      throw new FatalError(
        "Forgejo and GitHub github branches have diverged; run the Forgejo sync workflow, then retry.",
      );
    }
    git("push", "forgejo", `${githubTip}:refs/heads/${SOURCE_BRANCH}`);
    forgejoTip = githubTip;
  }

  git(
    "fetch",
    "--no-tags",
    "forgejo",
    `+refs/heads/${TARGET_BRANCH}:refs/remotes/forgejo/${TARGET_BRANCH}`,
  );
  const mainTip = rev(`refs/remotes/forgejo/${TARGET_BRANCH}`);

  if (isAncestor(forgejoTip, mainTip)) {
    console.log("Forgejo github is already merged into main; no PR created.");
    return;
  }

  if (!hasChanges(mainTip, forgejoTip)) {
    console.log("Forgejo github has no changes relative to main; no PR created.");
    return;
  }

  const existing = await findOpenPr();

  if (existing.length > 1) {
    throw new FatalError("Forgejo returned multiple matching open PRs; resolve them manually.");
  }

  if (existing.length === 1) {
    console.log(`Forgejo PR #${existing[0].number} is already open: ${existing[0].html_url}`);
    return;
  }

  const title = GH_PR_NUMBER
    ? `Sync GitHub contributions (GH-${GH_PR_NUMBER})`
    : "Sync GitHub contributions";

  // The create endpoint always returns the created pull request.
  const pr = (await api<ForgejoPullRequest>(new URL("pulls", api_url), {
    method: "POST",
    body: JSON.stringify({
      title,
      head: SOURCE_BRANCH,
      base: TARGET_BRANCH,
      body: prBody(forgejoTip),
    }),
  }))!;

  console.log(`Created Forgejo PR #${pr.number}: ${pr.html_url}`);
}

async function main(): Promise<void> {
  ensureRemote("forgejo", git_url.toString());

  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt += 1) {
    try {
      await syncOnce();
      return;
    } catch (error) {
      if (error instanceof FatalError || attempt === MAX_ATTEMPTS) {
        throw error;
      }
      console.warn(`Sync attempt ${attempt} failed: ${error_text(error)}`);
      await new Promise(resolve => setTimeout(resolve, attempt * 2000));
    }
  }
}

main().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
