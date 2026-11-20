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
const TAG_NAME = require_env("TAG_NAME");

interface ForgejoRelease {
  id: number;
}

interface ForgejoAsset {
  name: string;
}

interface GitHubRelease {
  name: string;
  body: string;
  prerelease: boolean;
  assets: GitHubAsset[];
}

interface GitHubAsset {
  name: string;
  url: string;
}

class ApiError extends Error {
  status: number;

  constructor(message: string, status: number) {
    super(message);
    this.status = status;
  }
}

async function forgejoApi<T>(path: string, options: RequestInit = {}): Promise<T> {
  const url = new URL(`/api/v1/repos/${FORGEJO_OWNER}/${FORGEJO_REPO}/${path}`, FORGEJO_BASE_URL);
  const response = await fetch(url, {
    ...options,
    headers: {
      Accept: "application/json",
      Authorization: `token ${FORGEJO_TOKEN}`,
      ...(options.body && !(options.body instanceof FormData)
        ? { "Content-Type": "application/json" }
        : {}),
    },
  });

  if (!response.ok) {
    const text = await response.text();
    throw new ApiError(
      `Forgejo API ${response.status} ${response.statusText}: ${text}`,
      response.status,
    );
  }

  const text = await response.text();
  if (!text) {
    throw new ApiError(`Forgejo API returned an empty body for ${path}`, response.status);
  }
  return JSON.parse(text) as T;
}

async function githubApi<T>(path: string): Promise<T> {
  const url = `https://api.github.com${path}`;
  const response = await fetch(url, {
    headers: {
      Accept: "application/vnd.github+json",
      Authorization: `Bearer ${GH_TOKEN}`,
      "X-GitHub-Api-Version": "2022-11-28",
    },
  });

  if (!response.ok) {
    throw new Error(`GitHub API ${response.status}: ${await response.text()}`);
  }

  return (await response.json()) as T;
}

async function downloadAsset(url: string): Promise<ArrayBuffer> {
  const response = await fetch(url, {
    headers: {
      Authorization: `Bearer ${GH_TOKEN}`,
      Accept: "application/octet-stream",
    },
    redirect: "follow",
  });

  if (!response.ok) {
    throw new Error(`Failed to download asset: ${response.status}`);
  }

  return response.arrayBuffer();
}

async function main(): Promise<void> {
  console.log(`Fetching GitHub release for tag ${TAG_NAME}...`);
  const ghRelease = await githubApi<GitHubRelease>(
    `/repos/${GITHUB_REPOSITORY}/releases/tags/${TAG_NAME}`,
  );

  console.log(`Creating Forgejo release for tag ${TAG_NAME}...`);

  let forgejoRelease: ForgejoRelease;
  try {
    forgejoRelease = await forgejoApi<ForgejoRelease>(`releases/tags/${TAG_NAME}`);
    console.log(`Release already exists (id: ${forgejoRelease.id}), updating...`);
    forgejoRelease = await forgejoApi<ForgejoRelease>(`releases/${forgejoRelease.id}`, {
      method: "PATCH",
      body: JSON.stringify({
        tag_name: TAG_NAME,
        target_commitish: "main",
        name: ghRelease.name,
        body: ghRelease.body,
        draft: false,
        prerelease: ghRelease.prerelease,
      }),
    });
  } catch (error) {
    if (error instanceof ApiError && error.status === 404) {
      forgejoRelease = await forgejoApi<ForgejoRelease>("releases", {
        method: "POST",
        body: JSON.stringify({
          tag_name: TAG_NAME,
          target_commitish: "main",
          name: ghRelease.name,
          body: ghRelease.body,
          draft: false,
          prerelease: ghRelease.prerelease,
        }),
      });
      console.log(`Created release (id: ${forgejoRelease.id})`);
    } else {
      throw error;
    }
  }

  const existingAssets = await forgejoApi<ForgejoAsset[]>(`releases/${forgejoRelease.id}/assets`);
  const existingNames = new Set(existingAssets.map(a => a.name));

  console.log("Uploading assets...");
  for (const asset of ghRelease.assets) {
    if (existingNames.has(asset.name)) {
      console.log(`  Skipping ${asset.name} (already exists)`);
      continue;
    }

    console.log(`  Downloading ${asset.name}...`);
    const data = await downloadAsset(asset.url);

    console.log(`  Uploading ${asset.name}...`);
    const formData = new FormData();
    formData.append("attachment", new Blob([data]), asset.name);

    await forgejoApi(`releases/${forgejoRelease.id}/assets`, {
      method: "POST",
      body: formData,
    });
    console.log(`  Uploaded ${asset.name}`);
  }

  console.log(
    `Release synced: ${FORGEJO_BASE_URL}/${FORGEJO_OWNER}/${FORGEJO_REPO}/releases/tag/${TAG_NAME}`,
  );
}

main().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
