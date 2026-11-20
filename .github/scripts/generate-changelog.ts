import { execFileSync } from "node:child_process";
import { appendFileSync } from "node:fs";

function require_env(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`Missing required environment variable: ${name}`);
  }
  return value;
}

const FORGEJO_BASE_URL = require_env("FORGEJO_BASE_URL");
const FORGEJO_OWNER = require_env("FORGEJO_OWNER");
const FORGEJO_REPO = require_env("FORGEJO_REPO");
const FORGEJO_TOKEN = require_env("FORGEJO_TOKEN");
const GH_TOKEN = require_env("GH_TOKEN");
const GITHUB_REPOSITORY = require_env("GITHUB_REPOSITORY");
const GITHUB_SERVER_URL = require_env("GITHUB_SERVER_URL");
const TAG_NAME = require_env("TAG_NAME");
const CDN_BASE_URL = process.env.CDN_BASE_URL;

interface ChangelogEntry {
  title: string;
  author_login: string | null;
  author_url: string | null;
  number: number;
  source: "github" | "forgejo";
  url: string;
}

interface PullRequestUser {
  login?: string;
  html_url?: string;
}

interface PullRequest {
  number: number;
  title: string;
  html_url: string;
  user?: PullRequestUser | null;
}

function git(...args: string[]): string {
  return execFileSync("git", args, { encoding: "utf8" }).trim();
}

async function forgejoApi<T>(path: string): Promise<T | null> {
  const url = new URL(path, FORGEJO_BASE_URL);
  const response = await fetch(url, {
    headers: {
      Accept: "application/json",
      Authorization: `token ${FORGEJO_TOKEN}`,
    },
  });

  if (!response.ok) {
    return null;
  }

  return (await response.json()) as T;
}

async function githubApi<T>(path: string): Promise<T | null> {
  const url = `https://api.github.com${path}`;
  const response = await fetch(url, {
    headers: {
      Accept: "application/vnd.github+json",
      Authorization: `Bearer ${GH_TOKEN}`,
      "X-GitHub-Api-Version": "2022-11-28",
    },
  });

  if (!response.ok) {
    return null;
  }

  return (await response.json()) as T;
}

function getPreviousTag(): string | null {
  try {
    const tags = git("tag", "--list", "v*", "--sort=-version:refname").split("\n").filter(Boolean);
    const currentIndex = tags.indexOf(TAG_NAME);
    if (currentIndex === -1 || currentIndex === tags.length - 1) {
      return null;
    }
    return tags[currentIndex + 1];
  } catch {
    return null;
  }
}

function getCommitShasBetween(from: string | null, to: string): string[] {
  // Merge, squash and fast-forward merges all leave their representative
  // commit on the first-parent chain, so this keeps the API lookups small.
  const range = from ? `${from}..${to}` : to;
  const log = git("log", "--first-parent", range, "--pretty=format:%H");
  return log ? log.split("\n").filter(Boolean) : [];
}

async function getForgejoPrForCommit(sha: string): Promise<PullRequest | null> {
  return await forgejoApi<PullRequest>(
    `/api/v1/repos/${FORGEJO_OWNER}/${FORGEJO_REPO}/commits/${sha}/pull`,
  );
}

async function getGitHubPr(number: number): Promise<PullRequest | null> {
  return await githubApi<PullRequest>(`/repos/${GITHUB_REPOSITORY}/pulls/${number}`);
}

async function buildChangelog(): Promise<ChangelogEntry[]> {
  const previousTag = getPreviousTag();
  const shas = getCommitShasBetween(previousTag, TAG_NAME);

  const entries: ChangelogEntry[] = [];
  const seen = new Set<string>();

  for (const sha of shas) {
    const pr = await getForgejoPrForCommit(sha);
    if (!pr) continue;

    // Synchronization pull requests merely mirror GitHub contributions; report
    // the original GitHub pull request instead of the internal sync PR.
    const ghMatch = pr.title.match(/\(GH-(\d+)\)/);
    if (ghMatch) {
      const ghNumber = parseInt(ghMatch[1]);
      if (seen.has(`github-${ghNumber}`)) continue;
      seen.add(`github-${ghNumber}`);

      const ghPr = await getGitHubPr(ghNumber);
      if (ghPr) {
        entries.push({
          title: ghPr.title,
          author_login: ghPr.user?.login ?? null,
          author_url: ghPr.user?.html_url ?? null,
          number: ghPr.number,
          source: "github",
          url: ghPr.html_url,
        });
        continue;
      }
    }

    if (seen.has(`forgejo-${pr.number}`)) continue;
    seen.add(`forgejo-${pr.number}`);

    entries.push({
      title: pr.title,
      author_login: pr.user?.login ?? null,
      author_url: pr.user?.html_url ?? null,
      number: pr.number,
      source: "forgejo",
      url: pr.html_url,
    });
  }

  return entries;
}

function formatAuthor(entry: ChangelogEntry): string {
  if (!entry.author_login) {
    return "";
  }
  if (!entry.author_url) {
    return ` by @${entry.author_login}`;
  }
  return ` by [@${entry.author_login}](${entry.author_url})`;
}

function formatChangelog(entries: ChangelogEntry[]): string {
  const previousTag = getPreviousTag();
  const compareUrl = previousTag
    ? `${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/compare/${previousTag}...${TAG_NAME}`
    : `${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/commits/${TAG_NAME}`;

  const ghBase = `${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/releases/download/${TAG_NAME}`;
  const cdnBase = CDN_BASE_URL || `https://artifacts.feniota.org/aphanite/${TAG_NAME}`;

  const lines = ["## What's Changed", ""];

  for (const entry of entries) {
    const marker = entry.source === "github" ? `#${entry.number}` : `!${entry.number}`;
    lines.push(`- ${entry.title}${formatAuthor(entry)} ([${marker}](${entry.url}))`);
  }

  return lines
    .concat([
      "",
      `**Full Changelog**: ${compareUrl}`,
      "",
      "## How to Verify Signatures",
      "",
      "Import the public key:",
      "",
      "```bash",
      "curl -O https://feniota.org/artifacts-signing-pubkey.gpg",
      "gpg --import artifacts-signing-pubkey.gpg",
      "```",
      "",
      "Verify the signature of the zip archive:",
      "",
      "```bash",
      "gpg --verify aphanite-<target>.zip.sig aphanite-<target>.zip",
      "```",
      "",
      "Or verify the signature of the binary inside the zip:",
      "",
      "```bash",
      "unzip aphanite-<target>.zip",
      "gpg --verify aphanite.sig aphanite",
      "```",
      "",
      "The signing key fingerprint is",
      "`03E4 5910 85AA 699B 79F5  AA8F E205 AF93 3EE2 C3BF`",
      "",
      "## Download",
      "",
      `- x64 Windows: [GitHub Release](${ghBase}/aphanite-x86_64-pc-windows-msvc.zip) / [CDN](${cdnBase}/aphanite-x86_64-pc-windows-msvc.zip) ([ZIP Signature](${ghBase}/aphanite-x86_64-pc-windows-msvc.zip.sig))`,
      `- x64 Linux with glibc: [GitHub Release](${ghBase}/aphanite-x86_64-unknown-linux-gnu.zip) / [CDN](${cdnBase}/aphanite-x86_64-unknown-linux-gnu.zip) ([ZIP Signature](${ghBase}/aphanite-x86_64-unknown-linux-gnu.zip.sig))`,
      `- x64 Linux with musl libc: [GitHub Release](${ghBase}/aphanite-x86_64-unknown-linux-musl.zip) / [CDN](${cdnBase}/aphanite-x86_64-unknown-linux-musl.zip) ([ZIP Signature](${ghBase}/aphanite-x86_64-unknown-linux-musl.zip.sig))`,
      "",
      "Binary signatures are included inside each zip archive.",
    ])
    .join("\n");
}

async function main(): Promise<void> {
  const entries = await buildChangelog();
  const changelog = formatChangelog(entries);

  const outputFile = process.env.GITHUB_OUTPUT;
  if (outputFile) {
    const delimiter = "CHANGELOG_EOF_" + Date.now();
    appendFileSync(outputFile, `content<<${delimiter}\n${changelog}\n${delimiter}\n`);
  } else {
    console.log(changelog);
  }
}

main().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
