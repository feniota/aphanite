import { execFileSync } from "node:child_process";
import { appendFileSync } from "node:fs";

const env = process.env;

const {
  FORGEJO_BASE_URL,
  FORGEJO_OWNER,
  FORGEJO_REPO,
  FORGEJO_TOKEN,
  GH_TOKEN,
  GITHUB_REPOSITORY,
  GITHUB_SERVER_URL,
  TAG_NAME,
  CDN_BASE_URL,
} = env;

function git(...args) {
  return execFileSync("git", args, { encoding: "utf8" }).trim();
}

async function forgejoApi(path) {
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

  return response.json();
}

async function githubApi(path) {
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

  return response.json();
}

function getPreviousTag() {
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

function getCommitsBetween(from, to) {
  const range = from ? `${from}..${to}` : to;
  const log = git("log", range, "--pretty=format:%H %s");
  if (!log) return [];
  return log
    .split("\n")
    .filter(Boolean)
    .map(line => {
      const spaceIndex = line.indexOf(" ");
      return {
        hash: line.substring(0, spaceIndex),
        message: line.substring(spaceIndex + 1),
      };
    });
}

function extractPrNumber(message) {
  const ghMatch = message.match(/\(#(\d+)\)$/);
  if (ghMatch) return { number: parseInt(ghMatch[1]), source: "github" };

  const fjMatch = message.match(/\(!(\d+)\)$/);
  if (fjMatch) return { number: parseInt(fjMatch[1]), source: "forgejo" };

  return null;
}

async function getForgejoPr(number) {
  return await forgejoApi(`/api/v1/repos/${FORGEJO_OWNER}/${FORGEJO_REPO}/pulls/${number}`);
}

async function getGitHubPr(number) {
  return await githubApi(`/repos/${GITHUB_REPOSITORY}/pulls/${number}`);
}

async function buildChangelog() {
  const previousTag = getPreviousTag();
  const commits = getCommitsBetween(previousTag, TAG_NAME);

  const entries = [];
  const seen = new Set();

  for (const commit of commits) {
    const prInfo = extractPrNumber(commit.message);

    if (prInfo) {
      if (seen.has(`${prInfo.source}-${prInfo.number}`)) continue;
      seen.add(`${prInfo.source}-${prInfo.number}`);

      if (prInfo.source === "github") {
        const pr = await getGitHubPr(prInfo.number);
        if (pr) {
          entries.push({
            title: pr.title,
            user: pr.user?.login || "unknown",
            id: `GH-${prInfo.number}`,
            url: pr.html_url,
          });
        }
      } else {
        const pr = await getForgejoPr(prInfo.number);
        if (!pr) continue;

        const ghMatch = pr.title.match(/\(GH-(\d+)\)/);
        if (ghMatch) {
          const ghNumber = parseInt(ghMatch[1]);
          const ghPr = await getGitHubPr(ghNumber);
          if (ghPr) {
            if (!seen.has(`github-${ghNumber}`)) {
              seen.add(`github-${ghNumber}`);
              entries.push({
                title: ghPr.title,
                user: ghPr.user?.login || "unknown",
                id: `GH-${ghNumber}`,
                url: ghPr.html_url,
              });
            }
            continue;
          }
        }

        entries.push({
          title: pr.title,
          user: pr.user?.login || "unknown",
          id: `!${prInfo.number}`,
          url: pr.html_url,
        });
      }
    } else {
      const shortMessage = commit.message.split("\n")[0];
      if (!seen.has(commit.hash)) {
        seen.add(commit.hash);
        entries.push({
          title: shortMessage,
          user: null,
          id: commit.hash.substring(0, 7),
          url: `${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/commit/${commit.hash}`,
        });
      }
    }
  }

  return entries;
}

function formatChangelog(entries) {
  const previousTag = getPreviousTag();
  const compareUrl = previousTag
    ? `${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/compare/${previousTag}...${TAG_NAME}`
    : `${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/commits/${TAG_NAME}`;

  const ghBase = `${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/releases/download/${TAG_NAME}`;
  const cdnBase = CDN_BASE_URL || `https://artifacts.feniota.org/releases/${TAG_NAME}`;

  const lines = [
    "## How to Verify Signatures",
    "",
    "Download the signature file (`.sig`) for the binary you want to verify, then run:",
    "",
    "```bash",
    "gpg --verify aphanite-<target>.sig aphanite-<target>/aphanite",
    "```",
    "",
    "The signing key fingerprint:",
    "`03E4 5910 85AA 699B 79F5  AA8F E205 AF93 3EE2 C3BF`",
    "",
    "## What's Changed",
    "",
  ];

  for (const entry of entries) {
    if (entry.user) {
      lines.push(`- ${entry.title} by @${entry.user} in [${entry.id}](${entry.url})`);
    } else {
      lines.push(`- ${entry.title} in [${entry.id}](${entry.url})`);
    }
  }

  lines.push("");
  lines.push(`**Full Changelog**: ${compareUrl}`);
  lines.push("");
  lines.push("## Download");
  lines.push("");
  lines.push(
    `- x64 Windows: [GitHub Release](${ghBase}/aphanite-x86_64-pc-windows-msvc.zip) [CDN](${cdnBase}/aphanite-x86_64-pc-windows-msvc.zip) ([Signature](${ghBase}/aphanite-x86_64-pc-windows-msvc.exe.sig))`,
  );
  lines.push(
    `- x64 Linux with glibc: [GitHub Release](${ghBase}/aphanite-x86_64-unknown-linux-gnu.zip) [CDN](${cdnBase}/aphanite-x86_64-unknown-linux-gnu.zip) ([Signature](${ghBase}/aphanite-x86_64-unknown-linux-gnu.sig))`,
  );
  lines.push(
    `- x64 Linux with musl libc: [GitHub Release](${ghBase}/aphanite-x86_64-unknown-linux-musl.zip) [CDN](${cdnBase}/aphanite-x86_64-unknown-linux-musl.zip) ([Signature](${ghBase}/aphanite-x86_64-unknown-linux-musl.sig))`,
  );

  return lines.join("\n");
}

async function main() {
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
