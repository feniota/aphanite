//! Cross-cutting behaviour: tokens shared between the General API and
//! Yggdrasil, TOTP login feeding Yggdrasil sessions, management permissions,
//! the shared login rate limiter and the per-user token pool.

import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { setTimeout as sleep } from "node:timers/promises";

import {
  type ApiResponse,
  type ExchangeableGameProfile,
  type LoginPayload,
  type ProfileDetail,
  type ProfilePayload,
  type TestServer,
  type TestUser,
  type UserPayload,
  type YggdrasilErrorBody,
  create_profile,
  create_user,
  general_login,
  general_validate,
  http,
  http_json,
  parse_json,
  start_server,
  totp_code,
  unhyphenated,
  unique_server_id,
  ygg_authenticate,
  ygg_has_joined,
  ygg_invalidate,
  ygg_join,
  ygg_refresh,
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

function expect_no_content(result: { status: number; text: string }): void {
  assert.equal(result.status, 204, result.text);
}

function expect_ygg_error(
  result: { status: number; text: string },
  status: number,
): YggdrasilErrorBody {
  assert.equal(result.status, status, result.text);
  const body = parse_json<YggdrasilErrorBody>(result);
  assert.equal(body.error, "ForbiddenOperationException", result.text);
  return body;
}

// ── Token sharing ───────────────────────────────────────────────────────

test("a General API token drives Yggdrasil sessions", async () => {
  const player = await new_player();
  const token = player.access_token;
  const profile = player.profiles[0];

  assert.equal((await ygg_validate(srv, token)).status, 204);

  const server_id = unique_server_id();
  expect_no_content(await ygg_join(srv, token, unhyphenated(profile.id), server_id));
  const joined = await ygg_has_joined(srv, profile.name, server_id);
  assert.equal(joined.status, 200, joined.text);
  assert.equal(parse_json<ExchangeableGameProfile>(joined).id, unhyphenated(profile.id));
});

test("a Yggdrasil access token authenticates General API requests", async () => {
  const player = await new_player();
  const auth = await ygg_authenticate(srv, player.user.email, player.user.password);
  assert.equal(auth.status, 200, auth.text);
  const token = auth.body.accessToken;

  const me = await http_json<ApiResponse<UserPayload>>(srv, "GET", "/api/users/me", { token });
  assert.equal(me.status, 200, me.text);
  assert.equal(me.body.success, true, me.text);
  assert.equal(me.body.success && me.body.payload.email, player.user.email);
  assert.equal(await general_validate(srv, token), 204);

  // Revoking through Yggdrasil also kills the General API session.
  expect_no_content(await ygg_invalidate(srv, token));
  assert.equal(await general_validate(srv, token), 401);
});

// ── Refreshing across APIs ──────────────────────────────────────────────

test("General refresh preserves the Yggdrasil client token and profile binding", async () => {
  const player = await new_player(2);
  const [chosen, other] = player.profiles;

  const auth = await ygg_authenticate(srv, player.user.email, player.user.password, {
    client_token: "interop-client",
  });
  assert.equal(auth.status, 200, auth.text);

  // Bind the token to one profile through Yggdrasil refresh.
  const bound = await ygg_refresh(srv, auth.body.accessToken, {
    client_token: "interop-client",
    selected_profile: { id: unhyphenated(chosen.id), name: chosen.name },
  });
  assert.equal(bound.status, 200, bound.text);
  const bound_token = bound.body.accessToken;

  // Refresh it through the General API.
  const refreshed = await http_json<ApiResponse<{ access_token: string; user: UserPayload }>>(
    srv,
    "POST",
    "/api/auth/refresh",
    { token: bound_token },
  );
  assert.equal(refreshed.status, 200, refreshed.text);
  assert.equal(refreshed.body.success, true, refreshed.text);
  const new_token = refreshed.body.success && refreshed.body.payload.access_token;
  assert.ok(new_token);
  assert.notEqual(new_token, bound_token);

  // The old token is revoked everywhere.
  expect_ygg_error(await ygg_validate(srv, bound_token), 403);
  assert.equal(await general_validate(srv, bound_token), 401);

  // Client token and profile binding survive the General refresh.
  assert.equal((await ygg_validate(srv, new_token, "interop-client")).status, 204);
  expect_no_content(await ygg_join(srv, new_token, unhyphenated(chosen.id), unique_server_id()));
  expect_ygg_error(await ygg_join(srv, new_token, unhyphenated(other.id), unique_server_id()), 403);

  // Yggdrasil can still refresh it, and both APIs accept the result.
  const again = await ygg_refresh(srv, new_token, { client_token: "interop-client" });
  assert.equal(again.status, 200, again.text);
  assert.equal(again.body.clientToken, "interop-client");
  const third = again.body.accessToken;
  assert.equal((await ygg_validate(srv, new_token, "interop-client")).status, 403);
  assert.equal(await general_validate(srv, third), 204);
});

test("Yggdrasil refresh rotates a General API token and revokes it everywhere", async () => {
  const player = await new_player();
  const login: LoginPayload = await general_login(srv, player.user.email, player.user.password);

  const refreshed = await ygg_refresh(srv, login.access_token, {
    client_token: login.client_token,
  });
  assert.equal(refreshed.status, 200, refreshed.text);
  assert.equal(refreshed.body.clientToken, login.client_token);
  // The General token was unbound, and refresh inherits the original binding.
  assert.equal(refreshed.body.selectedProfile, undefined);
  const new_token = refreshed.body.accessToken;

  // The General login token is gone from both APIs.
  assert.equal(await general_validate(srv, login.access_token), 401);
  expect_ygg_error(await ygg_validate(srv, login.access_token), 403);

  // The rotated token works as a General API token.
  const me = await http_json<ApiResponse<UserPayload>>(srv, "GET", "/api/users/me", {
    token: new_token,
  });
  assert.equal(me.status, 200, me.text);
  assert.equal(me.body.success && me.body.payload.email, player.user.email);

  // An unbound token may still join sessions for any of the owner's profiles.
  expect_no_content(
    await ygg_join(srv, new_token, unhyphenated(player.profiles[0].id), unique_server_id()),
  );
});

// ── TOTP login → Yggdrasil ──────────────────────────────────────────────

test("TOTP login yields a token that drives Yggdrasil", async () => {
  const player = await new_player();
  const profile = player.profiles[0];

  const created = await http_json<ApiResponse<{ secret: string; otpauth_url: string }>>(
    srv,
    "POST",
    "/api/users/me/credentials/totp",
    { token: player.access_token },
  );
  assert.equal(created.status, 200, created.text);
  assert.ok(created.body.success);
  const secret = created.body.success && created.body.payload.secret;
  assert.ok(secret);
  assert.ok(created.body.success && created.body.payload.otpauth_url.startsWith("otpauth://"));

  const verification = await http_json<ApiResponse<{ id: string }>>(
    srv,
    "POST",
    "/api/verification",
    {
      json: { method: "totp", email: player.user.email },
    },
  );
  assert.equal(verification.status, 200, verification.text);
  assert.ok(verification.body.success);
  const session_id = verification.body.success && verification.body.payload.id;
  assert.ok(session_id);

  // A wrong code is rejected without consuming the verification session.
  const valid_code = totp_code(secret);
  const wrong_code = valid_code === "000000" ? "111111" : "000000";
  const wrong = await http(srv, "POST", `/api/verification/${session_id}`, {
    json: { code: wrong_code },
  });
  assert.equal(wrong.status, 401, wrong.text);

  const completion = await http_json<ApiResponse<{ otp_token: string }>>(
    srv,
    "POST",
    `/api/verification/${session_id}`,
    { json: { code: totp_code(secret) } },
  );
  assert.equal(completion.status, 200, completion.text);
  assert.ok(completion.body.success);
  const otp_token = completion.body.success && completion.body.payload.otp_token;
  assert.ok(otp_token);

  // The OTP token is single-use.
  const login = await http_json<ApiResponse<LoginPayload>>(srv, "POST", "/api/auth/login", {
    json: { email: player.user.email, otp_token },
  });
  assert.equal(login.status, 200, login.text);
  assert.ok(login.body.success);
  const token = login.body.success && login.body.payload.access_token;
  assert.ok(token);

  const reused = await http(srv, "POST", "/api/auth/login", {
    json: { email: player.user.email, otp_token },
  });
  assert.equal(reused.status, 403, reused.text);

  // The TOTP-issued token is a full session token for both APIs.
  assert.equal(await general_validate(srv, token), 204);
  assert.equal((await ygg_validate(srv, token)).status, 204);
  const server_id = unique_server_id();
  expect_no_content(await ygg_join(srv, token, unhyphenated(profile.id), server_id));
  const joined = await ygg_has_joined(srv, profile.name, server_id);
  assert.equal(joined.status, 200, joined.text);
  assert.equal(parse_json<ExchangeableGameProfile>(joined).id, unhyphenated(profile.id));

  // Passwords keep working after TOTP is configured.
  await general_login(srv, player.user.email, player.user.password);
});

// ── Management permissions ──────────────────────────────────────────────

test("management permission gates cross-user access", async () => {
  const owner = await new_player();
  const stranger = await create_user(srv, srv.admin_token);
  const stranger_session = await general_login(srv, stranger.email, stranger.password);

  assert.ok(
    srv.admin_user.permissions.includes("management"),
    JSON.stringify(srv.admin_user.permissions),
  );
  assert.deepEqual(stranger.permissions, []);

  // Profile metadata is public; user data is not.
  const public_profile = await http_json<ApiResponse<ProfileDetail>>(
    srv,
    "GET",
    `/api/profiles/${owner.profiles[0].id}?with_skin=false`,
  );
  assert.equal(public_profile.status, 200, public_profile.text);
  assert.equal(
    public_profile.body.success && public_profile.body.payload.metadata.name,
    owner.profiles[0].name,
  );

  const forbidden_user = await http(srv, "GET", `/api/users/${owner.user.id}`, {
    token: stranger_session.access_token,
  });
  assert.equal(forbidden_user.status, 403, forbidden_user.text);
  const allowed_user = await http_json<ApiResponse<UserPayload>>(
    srv,
    "GET",
    `/api/users/${owner.user.id}`,
    { token: srv.admin_token },
  );
  assert.equal(allowed_user.status, 200, allowed_user.text);
  assert.equal(allowed_user.body.success && allowed_user.body.payload.email, owner.user.email);

  // Only management users may create accounts.
  const forbidden_create = await http(srv, "POST", "/api/user", {
    token: stranger_session.access_token,
    json: { email: `blocked-${Date.now()}@aphanite.test`, name: "Blocked", permissions: [] },
  });
  assert.equal(forbidden_create.status, 403, forbidden_create.text);

  // Only the owner or a management user may modify a profile.
  const new_name = `Admin${Date.now()}`.slice(0, 16);
  const forbidden_patch = await http(srv, "PATCH", `/api/profiles/${owner.profiles[0].id}`, {
    token: stranger_session.access_token,
    json: { name: new_name },
  });
  assert.equal(forbidden_patch.status, 403, forbidden_patch.text);
  const forbidden_delete = await http(srv, "DELETE", `/api/profiles/${owner.profiles[0].id}`, {
    token: stranger_session.access_token,
  });
  assert.equal(forbidden_delete.status, 403, forbidden_delete.text);

  // A management user may rename and then delete it; Yggdrasil follows.
  const renamed = await http_json<ApiResponse<ProfilePayload>>(
    srv,
    "PATCH",
    `/api/profiles/${owner.profiles[0].id}`,
    { token: srv.admin_token, json: { name: new_name } },
  );
  assert.equal(renamed.status, 200, renamed.text);
  assert.equal(renamed.body.success && renamed.body.payload.name, new_name);
  const lookup = await http_json<ApiResponse<ProfileDetail>>(
    srv,
    "GET",
    `/api/profiles/${owner.profiles[0].id}?with_skin=false`,
  );
  assert.equal(lookup.status, 200, lookup.text);
  assert.equal(lookup.body.success && lookup.body.payload.metadata.name, new_name);

  const deleted = await http_json<ApiResponse<ProfilePayload>>(
    srv,
    "DELETE",
    `/api/profiles/${owner.profiles[0].id}`,
    { token: srv.admin_token },
  );
  assert.equal(deleted.status, 200, deleted.text);
  const gone = await http(
    srv,
    "GET",
    `/api/yggdrasil/sessionserver/session/minecraft/profile/${unhyphenated(owner.profiles[0].id)}`,
  );
  assert.equal(gone.status, 204, gone.text);
  expect_ygg_error(
    await ygg_join(srv, owner.access_token, unhyphenated(owner.profiles[0].id), unique_server_id()),
    403,
  );
});

// ── Shared rate limiter ─────────────────────────────────────────────────

test("the login rate limiter is shared between the two APIs", async () => {
  const user = await create_user(srv, srv.admin_token);
  const wrong_login = () =>
    http(srv, "POST", "/api/auth/login", {
      json: { email: user.email, password: "definitely-wrong" },
    });

  // Capacity is 10 with a 1 token/s refill. Fire the burst, then probe while
  // the bucket is drained (well under a second later).
  const burst = Array.from({ length: 10 }, wrong_login);
  await sleep(150);

  const general_probe = await wrong_login();
  assert.equal(general_probe.status, 429, general_probe.text);

  const ygg_probe = await ygg_authenticate(srv, user.email, user.password);
  const ygg_body = expect_ygg_error(ygg_probe, 403);
  assert.equal(
    ygg_body.errorMessage,
    "Invalid credentials. Invalid username or password.",
    "Yggdrasil reports a rate-limited login with the same error as bad credentials",
  );

  for (const result of await Promise.all(burst)) {
    assert.equal(result.status, 403, result.text);
  }

  // The bucket refills: the correct password works again.
  await sleep(1500);
  await general_login(srv, user.email, user.password);
});

// ── Token pool ──────────────────────────────────────────────────────────

test("only the ten newest tokens per user are kept", async () => {
  const user = await create_user(srv, srv.admin_token);
  const tokens: string[] = [];
  for (let index = 0; index < 11; index++) {
    const login = await general_login(srv, user.email, user.password);
    tokens.push(login.access_token);
  }

  // The eleventh login evicted the oldest token ...
  assert.equal(await general_validate(srv, tokens[0]), 401);
  expect_ygg_error(await ygg_validate(srv, tokens[0]), 403);
  // ... while the second oldest and the newest are still valid.
  assert.equal(await general_validate(srv, tokens[1]), 204);
  assert.equal(await general_validate(srv, tokens[10]), 204);
  assert.equal((await ygg_validate(srv, tokens[10])).status, 204);
});
