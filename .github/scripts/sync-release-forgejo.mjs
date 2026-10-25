const env = process.env;

const required = ["FORGEJO_BASE_URL", "FORGEJO_OWNER", "FORGEJO_REPO", "FORGEJO_TOKEN", "TAG_NAME"];

for (const name of required) {
  if (!env[name]) {
    throw new Error(`Missing required environment variable: ${name}`);
  }
}

const {
  FORGEJO_BASE_URL,
  FORGEJO_OWNER,
  FORGEJO_REPO,
  FORGEJO_TOKEN,
  GH_TOKEN,
  GITHUB_REPOSITORY,
  TAG_NAME,
} = env;

async function forgejoApi(path, options = {}) {
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
    const error = new Error(`Forgejo API ${response.status} ${response.statusText}: ${text}`);
    error.status = response.status;
    throw error;
  }

  const text = await response.text();
  return text ? JSON.parse(text) : null;
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
    throw new Error(`GitHub API ${response.status}: ${await response.text()}`);
  }

  return response.json();
}

async function downloadAsset(url) {
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

async function main() {
  console.log(`Fetching GitHub release for tag ${TAG_NAME}...`);
  const ghRelease = await githubApi(`/repos/${GITHUB_REPOSITORY}/releases/tags/${TAG_NAME}`);

  console.log(`Creating Forgejo release for tag ${TAG_NAME}...`);

  let forgejoRelease;
  try {
    forgejoRelease = await forgejoApi(`releases/tags/${TAG_NAME}`);
    console.log(`Release already exists (id: ${forgejoRelease.id}), updating...`);
    forgejoRelease = await forgejoApi(`releases/${forgejoRelease.id}`, {
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
    if (error.status === 404) {
      forgejoRelease = await forgejoApi("releases", {
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

  const existingAssets = await forgejoApi(`releases/${forgejoRelease.id}/assets`);
  const existingNames = new Set((existingAssets || []).map(a => a.name));

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
