//! Yggdrasil session server and profile queries: join / hasJoined,
//! profile lookup, batch name lookup, and how General API profile
//! management is reflected back into Yggdrasil.

import assert from "node:assert/strict";
import { after, before, test } from "node:test";

import {
  type AuthenticateResponse,
  type ExchangeableGameProfile,
  type HttpResult,
  type ProfilePayload,
  type TestServer,
  type TestUser,
  type TexturesPayload,
  type YggdrasilErrorBody,
  create_profile,
  create_user,
  delete_profile,
  general_login,
  http,
  parse_json,
  property_of,
  rename_profile,
  start_server,
  textures_of,
  unhyphenated,
  unique_profile_name,
  unique_server_id,
  verify_signature,
  ygg_authenticate,
  ygg_has_joined,
  ygg_join,
  ygg_meta,
  ygg_profile,
  ygg_profiles_by_name,
  ygg_refresh,
} from "./harness.ts";

let srv: TestServer;
let public_key: string;

before(
  async () => {
    srv = await start_server();
    public_key = (await ygg_meta(srv)).body.signaturePublickey;
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

type BoundSession = { access_token: string; profile: ProfilePayload };

async function bound_session(player: Player): Promise<BoundSession> {
  assert.equal(player.profiles.length, 1, "bound_session expects a single-profile player");
  const auth = await ygg_authenticate(srv, player.user.email, player.user.password);
  assert.equal(auth.status, 200, auth.text);
  return { access_token: auth.body.accessToken, profile: player.profiles[0] };
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

function expect_no_content(result: HttpResult): void {
  assert.equal(result.status, 204, result.text);
  assert.equal(result.text, "", "204 responses must have an empty body");
}

function textures_property(profile: ExchangeableGameProfile) {
  const property = property_of(profile, "textures");
  assert.ok(property, "profile must carry a textures property");
  return property;
}

// ── Join / hasJoined ────────────────────────────────────────────────────

test("join records a session that hasJoined resolves to the signed profile", async () => {
  const player = await new_player();
  const session = await bound_session(player);
  const server_id = unique_server_id();

  const join = await ygg_join(
    srv,
    session.access_token,
    unhyphenated(session.profile.id),
    server_id,
  );
  expect_no_content(join);

  const joined = await ygg_has_joined(srv, session.profile.name, server_id);
  assert.equal(joined.status, 200, joined.text);
  const profile = parse_json<ExchangeableGameProfile>(joined);
  assert.equal(profile.id, unhyphenated(session.profile.id));
  assert.equal(profile.name, session.profile.name);

  // hasJoined always includes signed textures and uploadableTextures properties.
  const textures = textures_property(profile);
  assert.ok(textures.signature, "hasJoined textures property must be signed");
  assert.ok(
    verify_signature(public_key, textures.value, textures.signature),
    "textures signature must verify against the metadata key",
  );
  const uploadable = property_of(profile, "uploadableTextures");
  assert.ok(uploadable?.signature, "uploadableTextures property must be signed");
  assert.equal(uploadable.value, "skin,cape");
  assert.ok(verify_signature(public_key, uploadable.value, uploadable.signature));

  const payload = textures_of(profile);
  assert.equal(payload.profileId, unhyphenated(session.profile.id));
  assert.equal(payload.profileName, session.profile.name);
  assert.deepEqual(payload.textures, {});

  // Unknown server ids and mismatching usernames yield an empty 204.
  expect_no_content(await ygg_has_joined(srv, session.profile.name, unique_server_id()));
  expect_no_content(await ygg_has_joined(srv, session.profile.name, `${server_id}0`));
  expect_no_content(await ygg_has_joined(srv, "SomebodyElse", server_id));
});

test("hasJoined requires a prior join and ignores the ip parameter when client IP lookup is disabled", async () => {
  const player = await new_player();
  const session = await bound_session(player);
  const server_id = unique_server_id();

  // No join yet.
  expect_no_content(await ygg_has_joined(srv, session.profile.name, server_id));

  await ygg_join(srv, session.access_token, unhyphenated(session.profile.id), server_id);

  // client_ip = "disabled" in the test config: the ip parameter is never compared.
  const with_ip = await ygg_has_joined(srv, session.profile.name, server_id, "203.0.113.7");
  assert.equal(with_ip.status, 200, with_ip.text);
  const with_other_ip = await ygg_has_joined(srv, session.profile.name, server_id, "198.51.100.1");
  assert.equal(with_other_ip.status, 200, with_other_ip.text);
  // ... not even parsed.
  const with_garbage_ip = await ygg_has_joined(srv, session.profile.name, server_id, "not-an-ip");
  assert.equal(with_garbage_ip.status, 200, with_garbage_ip.text);
});

test("join accepts a profile owned by the token holder and rejects everything else", async () => {
  const owner = await new_player(2);
  const stranger = await new_player(1);

  // A token issued while several profiles exist has no bound profile, so the
  // client may join with either owned profile.
  const auth = await ygg_authenticate(srv, owner.user.email, owner.user.password);
  assert.equal(auth.status, 200, auth.text);
  assert.equal(auth.body.selectedProfile, undefined);

  expect_no_content(
    await ygg_join(
      srv,
      auth.body.accessToken,
      unhyphenated(owner.profiles[0].id),
      unique_server_id(),
    ),
  );
  expect_no_content(
    await ygg_join(
      srv,
      auth.body.accessToken,
      unhyphenated(owner.profiles[1].id),
      unique_server_id(),
    ),
  );

  // Profiles owned by someone else are forbidden.
  const foreign = await ygg_join(
    srv,
    auth.body.accessToken,
    unhyphenated(stranger.profiles[0].id),
    unique_server_id(),
  );
  expect_ygg_error(foreign, 403);

  // Unknown tokens and unknown profiles are forbidden.
  expect_ygg_error(
    await ygg_join(
      srv,
      "0123456789abcdef0123456789abcdef",
      unhyphenated(owner.profiles[0].id),
      unique_server_id(),
    ),
    403,
  );
  expect_ygg_error(
    await ygg_join(
      srv,
      auth.body.accessToken,
      "00000000000000000000000000000001",
      unique_server_id(),
    ),
    403,
  );

  // Malformed tokens fail body deserialization, like the other auth endpoints.
  const malformed = await http(srv, "POST", "/api/yggdrasil/sessionserver/session/minecraft/join", {
    json: {
      accessToken: "not-a-uuid",
      selectedProfile: unhyphenated(owner.profiles[0].id),
      serverId: unique_server_id(),
    },
  });
  assert.equal(malformed.status, 400, malformed.text);
});

test("a token bound by refresh may only join with the selected profile", async () => {
  const player = await new_player(2);
  const auth = await ygg_authenticate(srv, player.user.email, player.user.password);
  assert.equal(auth.status, 200, auth.text);

  const chosen = player.profiles[0];
  const refreshed = await ygg_refresh(srv, auth.body.accessToken, {
    selected_profile: { id: unhyphenated(chosen.id), name: chosen.name },
  });
  assert.equal(refreshed.status, 200, refreshed.text);
  const token = refreshed.body.accessToken;

  expect_no_content(await ygg_join(srv, token, unhyphenated(chosen.id), unique_server_id()));
  const other = await ygg_join(srv, token, unhyphenated(player.profiles[1].id), unique_server_id());
  expect_ygg_error(other, 403);
});

// ── Profile lookup ──────────────────────────────────────────────────────

test("profile lookup omits signatures by default and signs on demand", async () => {
  const player = await new_player();
  const profile = player.profiles[0];
  const id = unhyphenated(profile.id);

  for (const unsigned of [undefined, true]) {
    const result = await ygg_profile(srv, id, unsigned);
    assert.equal(result.status, 200, result.text);
    const body = result.body as ExchangeableGameProfile;
    assert.equal(body.id, id);
    assert.equal(body.name, profile.name);
    const textures = textures_property(body);
    assert.equal(textures.signature, undefined, "unsigned lookup must not sign properties");
    const payload = textures_of(body);
    assert.equal(payload.profileId, id);
    assert.equal(payload.profileName, profile.name);
  }

  const signed = await ygg_profile(srv, id, false);
  assert.equal(signed.status, 200, signed.text);
  const body = signed.body as ExchangeableGameProfile;
  const textures = textures_property(body);
  assert.ok(textures.signature, "unsigned=false must include a signature");
  assert.ok(verify_signature(public_key, textures.value, textures.signature));

  // UUIDs may be hyphenated even though the canonical form is not.
  const hyphenated = await ygg_profile(srv, profile.id);
  assert.equal(hyphenated.status, 200, hyphenated.text);
  assert.equal(hyphenated.body?.id, id);
});

test("profile lookup returns 204 for unknown profiles", async () => {
  const missing = await ygg_profile(srv, "00000000000000000000000000000001");
  expect_no_content(missing);
  assert.equal(missing.body, undefined);
});

// ── Batch name lookup ───────────────────────────────────────────────────

test("batch name lookup returns matching profiles without properties", async () => {
  const player = await new_player(2);
  const [first, second] = player.profiles;

  const result = await ygg_profiles_by_name(srv, [
    first.name,
    unique_profile_name("Missing"),
    second.name,
  ]);
  assert.equal(result.status, 200, result.text);
  assert.equal(result.body.length, 2);

  const by_id = new Map(result.body.map(profile => [profile.id, profile]));
  for (const profile of [first, second]) {
    const found = by_id.get(unhyphenated(profile.id));
    assert.ok(found, `profile ${profile.name} must be returned`);
    assert.equal(found.name, profile.name);
    assert.equal(found.properties, undefined, "batch lookup must not include properties");
  }

  const empty = await ygg_profiles_by_name(srv, []);
  assert.equal(empty.status, 200, empty.text);
  assert.deepEqual(empty.body, []);

  const unknown = await ygg_profiles_by_name(srv, [unique_profile_name("Nobody")]);
  assert.equal(unknown.status, 200, unknown.text);
  assert.deepEqual(unknown.body, []);

  const too_many = await ygg_profiles_by_name(
    srv,
    Array.from({ length: 51 }, (_, index) => `Name${index}`),
  );
  expect_ygg_error(too_many, 403);
});

// ── Cross-API profile management ────────────────────────────────────────

test("renaming a profile is reflected in lookups and in existing sessions", async () => {
  const player = await new_player();
  const session = await bound_session(player);
  const server_id = unique_server_id();
  const old_name = session.profile.name;

  await ygg_join(srv, session.access_token, unhyphenated(session.profile.id), server_id);
  assert.equal((await ygg_has_joined(srv, old_name, server_id)).status, 200);

  const new_name = unique_profile_name("Renamed");
  const renamed = await rename_profile(srv, player.access_token, session.profile.id, new_name);
  assert.equal(renamed.name, new_name);

  const looked_up = await ygg_profile(srv, unhyphenated(session.profile.id));
  assert.equal(looked_up.status, 200, looked_up.text);
  assert.equal(looked_up.body?.name, new_name);

  // The cached session still exists, but hasJoined matches against the
  // current profile name.
  expect_no_content(await ygg_has_joined(srv, old_name, server_id));
  const joined = await ygg_has_joined(srv, new_name, server_id);
  assert.equal(joined.status, 200, joined.text);
  assert.equal(parse_json<ExchangeableGameProfile>(joined).name, new_name);

  const auth = await ygg_authenticate(srv, player.user.email, player.user.password);
  assert.equal(auth.status, 200, auth.text);
  const body: AuthenticateResponse = auth.body;
  assert.equal(body.availableProfiles[0].name, new_name);
});

test("deleting a profile removes it from Yggdrasil and invalidates cached sessions", async () => {
  const player = await new_player();
  const session = await bound_session(player);
  const server_id = unique_server_id();
  const profile_id = unhyphenated(session.profile.id);
  const profile_name = session.profile.name;

  await ygg_join(srv, session.access_token, profile_id, server_id);
  assert.equal((await ygg_has_joined(srv, profile_name, server_id)).status, 200);

  await delete_profile(srv, player.access_token, session.profile.id);

  expect_no_content(await ygg_profile(srv, profile_id));
  const batch = await ygg_profiles_by_name(srv, [profile_name]);
  assert.equal(batch.status, 200, batch.text);
  assert.deepEqual(batch.body, []);

  // The join session is still cached, but the profile no longer resolves.
  expect_no_content(await ygg_has_joined(srv, profile_name, server_id));

  // Neither the still-valid bound token nor a fresh one can join it.
  expect_ygg_error(await ygg_join(srv, session.access_token, profile_id, unique_server_id()), 403);
  const fresh = await ygg_authenticate(srv, player.user.email, player.user.password);
  assert.equal(fresh.status, 200, fresh.text);
  assert.deepEqual(fresh.body.availableProfiles, []);
  expect_ygg_error(
    await ygg_join(srv, fresh.body.accessToken, profile_id, unique_server_id()),
    403,
  );
});

test("textures payload of an untouched profile is empty and unsigned lookup keeps it empty", async () => {
  const player = await new_player();
  const result = await ygg_profile(srv, unhyphenated(player.profiles[0].id));
  assert.equal(result.status, 200, result.text);
  const payload: TexturesPayload = textures_of(result.body as ExchangeableGameProfile);
  assert.deepEqual(payload.textures, {});
  assert.equal(payload.timestamp, 0);
});
