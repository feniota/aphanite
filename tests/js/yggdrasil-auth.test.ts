//! Yggdrasil authentication flow: metadata, authenticate, refresh, validate,
//! invalidate and signout — exercised over real HTTP against a spawned server.

import assert from "node:assert/strict";
import { after, before, test } from "node:test";

import {
  type AuthenticateResponse,
  type HttpResult,
  type ProfilePayload,
  type RefreshResponse,
  type TestServer,
  type TestUser,
  type YggdrasilErrorBody,
  create_profile,
  create_user,
  general_login,
  general_validate,
  http,
  http_json,
  parse_json,
  start_server,
  unhyphenated,
  unique_email,
  unique_server_id,
  ygg_authenticate,
  ygg_invalidate,
  ygg_join,
  ygg_meta,
  ygg_refresh,
  ygg_signout,
  ygg_validate,
} from "./harness.ts";

let srv: TestServer;

before(
  async () => {
    srv = await start_server();
  },
  { timeout: 120_000 },
);

after(async () => {
  await srv.stop();
});

type Player = {
  user: TestUser;
  access_token: string;
  profiles: ProfilePayload[];
};

async function new_player(profile_count = 1): Promise<Player> {
  const user = await create_user(srv, srv.admin_token);
  const session = await general_login(srv, user.email, user.password);
  const profiles: ProfilePayload[] = [];
  for (let index = 0; index < profile_count; index++) {
    profiles.push(await create_profile(srv, session.access_token));
  }
  return { user, access_token: session.access_token, profiles };
}

function expect_ygg_error(
  result: HttpResult,
  status: number,
  error = "ForbiddenOperationException",
): YggdrasilErrorBody {
  assert.equal(result.status, status, result.text);
  const body = parse_json<YggdrasilErrorBody>(result);
  assert.equal(body.error, error, result.text);
  assert.ok(body.errorMessage.length > 0, "errorMessage must not be empty");
  return body;
}

// ── Metadata ────────────────────────────────────────────────────────────

test("metadata endpoint exposes server info, skin domains and signing key", async () => {
  const response = await ygg_meta(srv);
  assert.equal(response.status, 200);
  assert.equal(
    response.headers.get("x-authlib-injector-api-location"),
    `${srv.base_url}/api/yggdrasil/`,
  );

  const { meta, skinDomains, signaturePublickey } = response.body;
  assert.equal(meta.implementationName, "Aphanite");
  assert.equal(meta.serverName, srv.server_name);
  assert.match(meta.implementationVersion, /^\d+\.\d+\.\d+/);
  assert.equal(meta.links?.homepage, `${srv.base_url}/`);
  assert.equal(meta.links?.register, `${srv.base_url}/login#/register`);
  assert.ok(skinDomains.includes(`127.0.0.1:${srv.port}`), JSON.stringify(skinDomains));
  assert.match(signaturePublickey, /^-----BEGIN PUBLIC KEY-----\n/);
  assert.match(signaturePublickey.trimEnd(), /-----END PUBLIC KEY-----$/);
});

test("ALI header is attached to non-Yggdrasil responses too", async () => {
  const general = await http(srv, "GET", "/api/users/me");
  assert.equal(general.status, 401);
  assert.equal(
    general.headers.get("x-authlib-injector-api-location"),
    `${srv.base_url}/api/yggdrasil/`,
  );

  const unknown = await http(srv, "GET", "/api/yggdrasil/not-a-real-endpoint");
  assert.equal(unknown.status, 404);
  assert.equal(
    unknown.headers.get("x-authlib-injector-api-location"),
    `${srv.base_url}/api/yggdrasil/`,
  );
  const body = parse_json<YggdrasilErrorBody>(unknown);
  assert.equal(body.error, "Not Found");
});

// ── Authenticate ────────────────────────────────────────────────────────

test("authenticate binds the only profile and returns user info", async () => {
  const player = await new_player();
  const result = await ygg_authenticate(srv, player.user.email, player.user.password, {
    client_token: "client-abc",
    request_user: true,
  });

  assert.equal(result.status, 200, result.text);
  const body = result.body;
  assert.match(body.accessToken, /^[0-9a-f]{32}$/);
  assert.equal(body.clientToken, "client-abc");

  assert.equal(body.availableProfiles.length, 1);
  assert.equal(body.availableProfiles[0].id, unhyphenated(player.profiles[0].id));
  assert.equal(body.availableProfiles[0].name, player.profiles[0].name);
  assert.equal(body.availableProfiles[0].properties, undefined);

  assert.ok(body.selectedProfile, "the only profile should be selected automatically");
  assert.equal(body.selectedProfile.id, unhyphenated(player.profiles[0].id));

  assert.ok(body.user, "requestUser=true must include user info");
  assert.equal(body.user.id, unhyphenated(player.user.id));
  assert.ok(body.user.properties.some(p => p.name === "preferredLanguage"));
});

test("authenticate leaves profile selection open when the user has several profiles", async () => {
  const player = await new_player(2);
  const auth = await ygg_authenticate(srv, player.user.email, player.user.password, {
    client_token: "picker",
  });

  assert.equal(auth.status, 200, auth.text);
  assert.equal(auth.body.availableProfiles.length, 2);
  assert.equal(auth.body.selectedProfile, undefined);
  const ids = auth.body.availableProfiles.map(p => p.id).sort();
  assert.deepEqual(ids, player.profiles.map(p => unhyphenated(p.id)).sort());

  const chosen = player.profiles[1];
  const refreshed = await ygg_refresh(srv, auth.body.accessToken, {
    client_token: "picker",
    request_user: true,
    selected_profile: { id: unhyphenated(chosen.id), name: chosen.name },
  });
  assert.equal(refreshed.status, 200, refreshed.text);
  assert.equal(refreshed.body.clientToken, "picker");
  assert.equal(refreshed.body.selectedProfile?.id, unhyphenated(chosen.id));
  assert.ok(refreshed.body.user);

  const new_token = refreshed.body.accessToken;
  assert.notEqual(new_token, auth.body.accessToken);

  // The selected profile now authorizes a session join ...
  const join_ok = await ygg_join(srv, new_token, unhyphenated(chosen.id), "server-multi");
  assert.equal(join_ok.status, 204, join_ok.text);

  // ... but the other profile does not.
  const join_wrong = await ygg_join(
    srv,
    new_token,
    unhyphenated(player.profiles[0].id),
    "server-multi-wrong",
  );
  assert.equal(join_wrong.status, 403, join_wrong.text);
  expect_ygg_error(join_wrong, 403);

  // Refresh revokes the original token.
  assert.equal((await ygg_validate(srv, auth.body.accessToken)).status, 403);
  assert.equal((await ygg_validate(srv, new_token, "picker")).status, 204);
});

test("authenticate without profiles returns an empty profile list", async () => {
  const user = await create_user(srv, srv.admin_token);
  const result = await ygg_authenticate(srv, user.email, user.password, {
    request_user: true,
  });

  assert.equal(result.status, 200, result.text);
  assert.deepEqual(result.body.availableProfiles, []);
  assert.equal(result.body.selectedProfile, undefined);
  assert.ok(result.body.user);

  // The token is valid even though no profile is bound.
  assert.equal((await ygg_validate(srv, result.body.accessToken)).status, 204);
  assert.match(result.body.clientToken, /^[0-9a-f]{32}$/);
});

test("authenticate rejects wrong passwords and unknown users identically", async () => {
  const player = await new_player(0);

  const wrong_password = await ygg_authenticate(srv, player.user.email, "definitely-wrong", {
    request_user: false,
  });
  const body = expect_ygg_error(wrong_password, 403);
  assert.equal(body.errorMessage, "Invalid credentials. Invalid username or password.");

  const unknown_user = await ygg_authenticate(srv, unique_email(), "whatever-password", {
    request_user: false,
  });
  const unknown_body = expect_ygg_error(unknown_user, 403);
  assert.equal(unknown_body.errorMessage, "Invalid credentials. Invalid username or password.");
});

test("authenticate generates a client token when the request omits it", async () => {
  const player = await new_player();
  const result = await ygg_authenticate(srv, player.user.email, player.user.password);
  assert.equal(result.status, 200, result.text);

  const { accessToken, clientToken } = result.body;
  assert.match(clientToken, /^[0-9a-f]{32}$/);
  assert.notEqual(clientToken, accessToken);
  assert.equal((await ygg_validate(srv, accessToken, clientToken)).status, 204);
  assert.equal((await ygg_validate(srv, accessToken, "some-other-client")).status, 403);
});

// ── Refresh ─────────────────────────────────────────────────────────────

test("refresh rotates the token and verifies the client token", async () => {
  const player = await new_player();
  const auth = await ygg_authenticate(srv, player.user.email, player.user.password, {
    client_token: "refresh-client",
  });
  assert.equal(auth.status, 200, auth.text);

  // A mismatching client token fails and leaves the original token valid.
  const mismatched = await ygg_refresh(srv, auth.body.accessToken, {
    client_token: "not-the-client",
  });
  expect_ygg_error(mismatched, 403);
  assert.equal((await ygg_validate(srv, auth.body.accessToken, "refresh-client")).status, 204);

  const refreshed = await ygg_refresh(srv, auth.body.accessToken, {
    client_token: "refresh-client",
    request_user: true,
  });
  assert.equal(refreshed.status, 200, refreshed.text);
  const body: RefreshResponse = refreshed.body;
  assert.notEqual(body.accessToken, auth.body.accessToken);
  assert.equal(body.clientToken, "refresh-client");
  assert.equal(body.selectedProfile?.id, unhyphenated(player.profiles[0].id));
  assert.ok(body.user);

  // The original token is revoked once refresh succeeds.
  assert.equal((await ygg_validate(srv, auth.body.accessToken, "refresh-client")).status, 403);
  assert.equal((await ygg_validate(srv, body.accessToken, "refresh-client")).status, 204);
});

test("refresh rejects unknown tokens", async () => {
  const result = await ygg_refresh(srv, "0123456789abcdef0123456789abcdef");
  assert.equal(result.status, 403, result.text);
  const body = parse_json<YggdrasilErrorBody>(result);
  assert.equal(body.error, "ForbiddenOperationException");
  assert.equal(body.errorMessage, "Invalid token.");
});

// ── Validate / Invalidate ───────────────────────────────────────────────

test("validate rejects malformed access tokens", async () => {
  const garbage = await http(srv, "POST", "/api/yggdrasil/authserver/validate", {
    json: { accessToken: "definitely-not-a-uuid" },
  });
  // Body deserialization failures are not covered by the spec.
  assert.equal(garbage.status, 400, garbage.text);
});

test("invalidate revokes a token and is idempotent", async () => {
  const player = await new_player();
  const auth = await ygg_authenticate(srv, player.user.email, player.user.password);
  assert.equal(auth.status, 200, auth.text);
  const token = auth.body.accessToken;

  assert.equal((await ygg_invalidate(srv, token)).status, 204);
  assert.equal((await ygg_validate(srv, token)).status, 403);

  // Revoking an already invalid token still reports success.
  assert.equal((await ygg_invalidate(srv, token)).status, 204);
  assert.equal((await ygg_invalidate(srv, "fedcba9876543210fedcba9876543210")).status, 204);
});

// ── Signout ─────────────────────────────────────────────────────────────

test("signout revokes every token of the account across both APIs", async () => {
  const player = await new_player();

  const auth_one = await ygg_authenticate(srv, player.user.email, player.user.password);
  const auth_two = await ygg_authenticate(srv, player.user.email, player.user.password);
  assert.equal(auth_one.status, 200, auth_one.text);
  assert.equal(auth_two.status, 200, auth_two.text);

  const general = await general_login(srv, player.user.email, player.user.password);

  const signout = await ygg_signout(srv, player.user.email, player.user.password);
  assert.equal(signout.status, 204, signout.text);

  assert.equal((await ygg_validate(srv, auth_one.body.accessToken)).status, 403);
  assert.equal((await ygg_validate(srv, auth_two.body.accessToken)).status, 403);
  assert.equal((await ygg_validate(srv, general.access_token)).status, 403);
  assert.equal(await general_validate(srv, general.access_token), 401);
});

test("signout with a wrong password leaves tokens intact", async () => {
  const player = await new_player();
  const auth = await ygg_authenticate(srv, player.user.email, player.user.password);
  assert.equal(auth.status, 200, auth.text);

  const signout = await ygg_signout(srv, player.user.email, "wrong-password");
  expect_ygg_error(signout, 403);

  assert.equal((await ygg_validate(srv, auth.body.accessToken)).status, 204);
});

test("authenticate response user id and profile ids use unhyphenated UUIDs", async () => {
  const player = await new_player();
  const auth = await ygg_authenticate(srv, player.user.email, player.user.password, {
    request_user: true,
  });
  const body: AuthenticateResponse = auth.body;
  assert.equal(body.user?.id, player.user.id.replaceAll("-", ""));
  assert.equal(body.availableProfiles[0].id, player.profiles[0].id.replaceAll("-", ""));
});

// ── Optional request fields ─────────────────────────────────────────────

test("authenticate treats a missing requestUser as false", async () => {
  const player = await new_player();
  const result = await http_json<AuthenticateResponse>(
    srv,
    "POST",
    "/api/yggdrasil/authserver/authenticate",
    {
      json: {
        username: player.user.email,
        password: player.user.password,
        agent: { name: "Minecraft", version: 1 },
      },
    },
  );
  assert.equal(result.status, 200, result.text);
  assert.ok(!("user" in result.body), "user info must be omitted when requestUser is absent");
});

test("refresh treats a missing requestUser as false", async () => {
  const player = await new_player();
  const auth = await ygg_authenticate(srv, player.user.email, player.user.password);
  const result = await http_json<RefreshResponse>(
    srv,
    "POST",
    "/api/yggdrasil/authserver/refresh",
    { json: { accessToken: auth.body.accessToken } },
  );
  assert.equal(result.status, 200, result.text);
  assert.ok(!("user" in result.body), "user info must be omitted when requestUser is absent");
});

// ── Profile binding rules on refresh ────────────────────────────────────

test("refresh keeps the original profile binding when selectedProfile is omitted", async () => {
  const player = await new_player(2);
  const [other, chosen] = player.profiles;
  const auth = await ygg_authenticate(srv, player.user.email, player.user.password, {
    client_token: "keep-binding",
  });
  assert.equal(auth.status, 200, auth.text);
  assert.equal(auth.body.selectedProfile, undefined);

  const bound = await ygg_refresh(srv, auth.body.accessToken, {
    client_token: "keep-binding",
    selected_profile: { id: unhyphenated(chosen.id), name: chosen.name },
  });
  assert.equal(bound.status, 200, bound.text);
  assert.equal(bound.body.selectedProfile?.id, unhyphenated(chosen.id));

  const refreshed = await ygg_refresh(srv, bound.body.accessToken, {
    client_token: "keep-binding",
  });
  assert.equal(refreshed.status, 200, refreshed.text);
  assert.equal(refreshed.body.selectedProfile?.id, unhyphenated(chosen.id));

  const token = refreshed.body.accessToken;
  assert.equal(
    (await ygg_join(srv, token, unhyphenated(chosen.id), unique_server_id())).status,
    204,
  );
  assert.equal(
    (await ygg_join(srv, token, unhyphenated(other.id), unique_server_id())).status,
    403,
  );
});

test("refresh rejects selecting a profile for an already bound token", async () => {
  const player = await new_player(2);
  const [first, second] = player.profiles;
  const auth = await ygg_authenticate(srv, player.user.email, player.user.password);
  assert.equal(auth.status, 200, auth.text);

  const bound = await ygg_refresh(srv, auth.body.accessToken, {
    selected_profile: { id: unhyphenated(first.id), name: first.name },
  });
  assert.equal(bound.status, 200, bound.text);

  // The spec allows (re)selection only while the token has no binding, even
  // when the requested profile is the one already bound.
  for (const target of [first, second]) {
    const rebind = await ygg_refresh(srv, bound.body.accessToken, {
      selected_profile: { id: unhyphenated(target.id), name: target.name },
    });
    const body = expect_ygg_error(rebind, 400, "IllegalArgumentException");
    assert.equal(body.errorMessage, "Access token already has a profile assigned.");
  }

  // A failed refresh leaves the original token valid and bound.
  assert.equal(
    (await ygg_join(srv, bound.body.accessToken, unhyphenated(first.id), unique_server_id()))
      .status,
    204,
  );
});

test("refresh rejects selecting a profile owned by another user", async () => {
  // Two profiles leave the owner's token unbound after authenticate.
  const owner = await new_player(2);
  const stranger = await new_player();
  const auth = await ygg_authenticate(srv, owner.user.email, owner.user.password);
  assert.equal(auth.status, 200, auth.text);

  const foreign = stranger.profiles[0];
  const result = await ygg_refresh(srv, auth.body.accessToken, {
    selected_profile: { id: unhyphenated(foreign.id), name: foreign.name },
  });
  expect_ygg_error(result, 403);

  // A failed refresh leaves the original token valid.
  assert.equal((await ygg_validate(srv, auth.body.accessToken)).status, 204);
});

test("refresh without clientToken keeps the original client token", async () => {
  const player = await new_player();
  const auth = await ygg_authenticate(srv, player.user.email, player.user.password, {
    client_token: "sticky-client",
  });
  assert.equal(auth.status, 200, auth.text);

  const refreshed = await ygg_refresh(srv, auth.body.accessToken);
  assert.equal(refreshed.status, 200, refreshed.text);
  assert.equal(refreshed.body.clientToken, "sticky-client");
  assert.equal((await ygg_validate(srv, refreshed.body.accessToken, "sticky-client")).status, 204);
});
