//! Yggdrasil texture upload / deletion and how textures surface through
//! join sessions, profile lookups and the General API.

import assert from "node:assert/strict";
import { after, before, test } from "node:test";

import {
  type ExchangeableGameProfile,
  type HttpResult,
  type ProfilePayload,
  type TestServer,
  type TestUser,
  type YggdrasilErrorBody,
  create_profile,
  create_user,
  delete_profile,
  fetch_asset,
  general_login,
  get_profile_detail,
  http,
  make_png,
  parse_json,
  png_size,
  property_of,
  rename_profile,
  start_server,
  textures_of,
  unhyphenated,
  unique_color,
  unique_server_id,
  verify_signature,
  ygg_authenticate,
  ygg_delete_texture,
  ygg_has_joined,
  ygg_join,
  ygg_meta,
  ygg_profile,
  ygg_put_texture,
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

async function bound_session(
  player: Player,
): Promise<{ access_token: string; profile: ProfilePayload }> {
  assert.equal(player.profiles.length, 1, "bound_session expects a single-profile player");
  const auth = await ygg_authenticate(srv, player.user.email, player.user.password);
  assert.equal(auth.status, 200, auth.text);
  return { access_token: auth.body.accessToken, profile: player.profiles[0] };
}

function expect_no_content(result: HttpResult): void {
  assert.equal(result.status, 204, result.text);
}

function expect_ygg_error(
  result: HttpResult,
  status: number,
  error = "ForbiddenOperationException",
): YggdrasilErrorBody {
  assert.equal(result.status, status, result.text);
  const body = parse_json<YggdrasilErrorBody>(result);
  assert.equal(body.error, error, result.text);
  return body;
}

// Pixel colours used below are unique per upload. Assets are de-duplicated
// by content across the whole server, so reusing a colour between tests
// would keep the shared asset alive via reference counting.

// ── Happy path ──────────────────────────────────────────────────────────

test("uploaded skin is exposed through profile lookup, hasJoined and the General API", async () => {
  const player = await new_player();
  const session = await bound_session(player);
  const id = unhyphenated(session.profile.id);

  const png = make_png(64, 32, unique_color());
  const upload = await ygg_put_texture(srv, session.access_token, id, "skin", png, "slim");
  expect_no_content(upload);

  const lookup = await ygg_profile(srv, id, false);
  assert.equal(lookup.status, 200, lookup.text);
  const profile = lookup.body as ExchangeableGameProfile;
  const textures = textures_of(profile);
  const skin = textures.textures.SKIN;
  assert.ok(skin, "profile must expose a SKIN texture");
  assert.match(skin.url, new RegExp(`^${srv.base_url}/assets/[0-9a-f]{64}$`));
  assert.equal(skin.metadata?.model, "slim");

  const property = property_of(profile, "textures");
  assert.ok(property?.signature);
  assert.ok(verify_signature(public_key, property.value, property.signature));

  const asset = await fetch_asset(skin.url);
  assert.equal(asset.status, 200);
  assert.equal(asset.headers.get("content-type"), "image/png");
  assert.deepEqual(png_size(asset.bytes), { width: 64, height: 32 });

  // The General API exposes the same URL and model.
  const detail = await get_profile_detail(srv, session.profile.id);
  assert.equal(detail.skin?.skin, skin.url);
  assert.equal(detail.skin?.model, "slim");
  assert.equal(detail.skin?.cape, undefined);

  // join/hasJoined carries the texture with a valid signature.
  const server_id = unique_server_id();
  expect_no_content(await ygg_join(srv, session.access_token, id, server_id));
  const joined = await ygg_has_joined(srv, session.profile.name, server_id);
  assert.equal(joined.status, 200, joined.text);
  const joined_profile = parse_json<ExchangeableGameProfile>(joined);
  const joined_textures = textures_of(joined_profile);
  assert.equal(joined_textures.textures.SKIN?.url, skin.url);
  assert.equal(joined_textures.textures.SKIN?.metadata?.model, "slim");
  const joined_property = property_of(joined_profile, "textures");
  assert.ok(joined_property?.signature);
  assert.ok(verify_signature(public_key, joined_property.value, joined_property.signature));
});

test("the model field controls the skin model and defaults to classic", async () => {
  const player = await new_player();
  const session = await bound_session(player);
  const id = unhyphenated(session.profile.id);
  const png = make_png(64, 32, unique_color());

  const model_of = async (): Promise<string | undefined> =>
    (await get_profile_detail(srv, session.profile.id)).skin?.model;

  expect_no_content(await ygg_put_texture(srv, session.access_token, id, "skin", png));
  assert.equal(await model_of(), "default");

  expect_no_content(await ygg_put_texture(srv, session.access_token, id, "skin", png, "classic"));
  assert.equal(await model_of(), "default");

  expect_no_content(await ygg_put_texture(srv, session.access_token, id, "skin", png, "default"));
  assert.equal(await model_of(), "default");

  expect_no_content(await ygg_put_texture(srv, session.access_token, id, "skin", png, "slim"));
  assert.equal(await model_of(), "slim");

  // The api accepts explicit "default" too, and the empty model means classic.
  expect_no_content(await ygg_put_texture(srv, session.access_token, id, "skin", png, ""));
  assert.equal(await model_of(), "default");
});

test("replacing a texture removes the previous asset", async () => {
  const player = await new_player();
  const session = await bound_session(player);
  const id = unhyphenated(session.profile.id);

  expect_no_content(
    await ygg_put_texture(srv, session.access_token, id, "skin", make_png(64, 32, unique_color())),
  );
  const first_url = (await get_profile_detail(srv, session.profile.id)).skin?.skin;
  assert.ok(first_url);
  assert.equal((await fetch_asset(first_url)).status, 200);

  expect_no_content(
    await ygg_put_texture(srv, session.access_token, id, "skin", make_png(64, 32, unique_color())),
  );
  const second_url = (await get_profile_detail(srv, session.profile.id)).skin?.skin;
  assert.ok(second_url);
  assert.notEqual(second_url, first_url);

  assert.equal((await fetch_asset(first_url)).status, 404, "replaced asset must be gone");
  assert.equal((await fetch_asset(second_url)).status, 200);
});

test("skins and capes coexist and are deleted independently", async () => {
  const player = await new_player();
  const session = await bound_session(player);
  const id = unhyphenated(session.profile.id);

  expect_no_content(
    await ygg_put_texture(srv, session.access_token, id, "skin", make_png(64, 32, unique_color())),
  );
  expect_no_content(
    await ygg_put_texture(srv, session.access_token, id, "cape", make_png(64, 32, unique_color())),
  );

  const both = await get_profile_detail(srv, session.profile.id);
  assert.ok(both.skin?.skin);
  assert.ok(both.skin?.cape);
  assert.notEqual(both.skin?.skin, both.skin?.cape);

  const payload = textures_of((await ygg_profile(srv, id)).body as ExchangeableGameProfile);
  assert.equal(payload.textures.SKIN?.metadata?.model, "default");
  assert.ok(payload.textures.CAPE);
  assert.equal(payload.textures.CAPE?.metadata, undefined);

  expect_no_content(await ygg_delete_texture(srv, session.access_token, id, "cape"));
  const skin_only = await get_profile_detail(srv, session.profile.id);
  assert.ok(skin_only.skin?.skin, "the skin must survive cape deletion");
  assert.equal(skin_only.skin?.cape, undefined);
  const after_cape = textures_of((await ygg_profile(srv, id)).body as ExchangeableGameProfile);
  assert.deepEqual(Object.keys(after_cape.textures), ["SKIN"]);

  expect_no_content(await ygg_delete_texture(srv, session.access_token, id, "skin"));
  const empty = await get_profile_detail(srv, session.profile.id);
  assert.equal(empty.skin?.skin, undefined);
  assert.equal(empty.skin?.cape, undefined);
});

test("deleting a texture removes the asset and is idempotent", async () => {
  const player = await new_player();
  const session = await bound_session(player);
  const id = unhyphenated(session.profile.id);

  expect_no_content(
    await ygg_put_texture(srv, session.access_token, id, "skin", make_png(64, 32, unique_color())),
  );
  const url = (await get_profile_detail(srv, session.profile.id)).skin?.skin;
  assert.ok(url);

  expect_no_content(await ygg_delete_texture(srv, session.access_token, id, "skin"));
  assert.equal((await fetch_asset(url)).status, 404);

  const payload = textures_of((await ygg_profile(srv, id)).body as ExchangeableGameProfile);
  assert.deepEqual(payload.textures, {});

  // Deleting an absent texture still succeeds.
  expect_no_content(await ygg_delete_texture(srv, session.access_token, id, "skin"));
  expect_no_content(await ygg_delete_texture(srv, session.access_token, id, "cape"));
});

test("identical files are deduplicated across profiles and reference counted", async () => {
  const player = await new_player(2);
  const [first, second] = player.profiles;
  const auth = await ygg_authenticate(srv, player.user.email, player.user.password);
  assert.equal(auth.status, 200, auth.text);
  const token = auth.body.accessToken;
  const png = make_png(64, 32, unique_color());

  expect_no_content(await ygg_put_texture(srv, token, unhyphenated(first.id), "skin", png));
  expect_no_content(await ygg_put_texture(srv, token, unhyphenated(second.id), "skin", png));

  const first_url = (await get_profile_detail(srv, first.id)).skin?.skin;
  const second_url = (await get_profile_detail(srv, second.id)).skin?.skin;
  assert.ok(first_url);
  assert.equal(second_url, first_url, "identical content must share one asset");
  assert.equal((await fetch_asset(first_url)).status, 200);

  expect_no_content(await ygg_delete_texture(srv, token, unhyphenated(first.id), "skin"));
  assert.equal(
    (await fetch_asset(first_url)).status,
    200,
    "the asset must survive while another profile references it",
  );

  expect_no_content(await ygg_delete_texture(srv, token, unhyphenated(second.id), "skin"));
  assert.equal((await fetch_asset(first_url)).status, 404);
});

test("textures stay attached when the profile is renamed", async () => {
  const player = await new_player();
  const session = await bound_session(player);
  const id = unhyphenated(session.profile.id);

  expect_no_content(
    await ygg_put_texture(srv, session.access_token, id, "skin", make_png(64, 32, unique_color())),
  );
  const url = (await get_profile_detail(srv, session.profile.id)).skin?.skin;
  assert.ok(url);

  const renamed = await rename_profile(
    srv,
    player.access_token,
    session.profile.id,
    "TextureKeeper",
  );
  assert.equal(renamed.name, "TextureKeeper");

  const detail = await get_profile_detail(srv, session.profile.id);
  assert.equal(detail.metadata.name, "TextureKeeper");
  assert.equal(detail.skin?.skin, url);
});

// ── Validation ──────────────────────────────────────────────────────────

test("multipart uploads without a usable PNG are rejected", async () => {
  const player = await new_player();
  const session = await bound_session(player);
  const id = unhyphenated(session.profile.id);

  // Not multipart at all.
  const not_multipart = await http(srv, "PUT", `/api/yggdrasil/api/user/profile/${id}/skin`, {
    token: session.access_token,
    raw_body: "hello",
  });
  assert.equal(not_multipart.status, 400, not_multipart.text);

  // Multipart without a file field.
  const no_file = new FormData();
  no_file.append("model", "slim");
  const missing_file = await http(srv, "PUT", `/api/yggdrasil/api/user/profile/${id}/skin`, {
    token: session.access_token,
    raw_body: no_file,
  });
  assert.equal(missing_file.status, 400, missing_file.text);

  // Not a PNG.
  const junk = await ygg_put_texture(
    srv,
    session.access_token,
    id,
    "skin",
    Buffer.from("definitely not a png"),
  );
  assert.equal(junk.status, 400, junk.text);

  // PNG whose dimensions are not a multiple of 64x32.
  const bad_size = await ygg_put_texture(srv, session.access_token, id, "skin", make_png(100, 100));
  assert.equal(bad_size.status, 400, bad_size.text);
});

test("oversized textures are rejected", async () => {
  const player = await new_player();
  const session = await bound_session(player);
  const id = unhyphenated(session.profile.id);

  // 512x512 is the documented upper bound; 576 is a valid multiple but too big.
  const big_pixels = await ygg_put_texture(
    srv,
    session.access_token,
    id,
    "skin",
    make_png(576, 32),
  );
  assert.equal(big_pixels.status, 413, big_pixels.text);

  // The file itself may not exceed 8 MiB.
  const big_file = Buffer.alloc(8 * 1024 * 1024 + 1, 0);
  const too_large = await ygg_put_texture(srv, session.access_token, id, "skin", big_file);
  assert.equal(too_large.status, 413, too_large.text);
});

test("invalid skin models are rejected before anything is stored", async () => {
  const player = await new_player();
  const session = await bound_session(player);
  const id = unhyphenated(session.profile.id);

  const result = await ygg_put_texture(
    srv,
    session.access_token,
    id,
    "skin",
    make_png(64, 32),
    "beanpole",
  );
  assert.equal(result.status, 400, result.text);

  const detail = await get_profile_detail(srv, session.profile.id);
  assert.equal(detail.skin, undefined, "a rejected upload must not create textures");
});

test("a cape ignores the model field", async () => {
  const player = await new_player();
  const session = await bound_session(player);
  const id = unhyphenated(session.profile.id);

  expect_no_content(
    await ygg_put_texture(
      srv,
      session.access_token,
      id,
      "cape",
      make_png(64, 32, unique_color()),
      "slim",
    ),
  );
  const detail = await get_profile_detail(srv, session.profile.id);
  assert.equal(detail.skin?.model, "default");
  const payload = textures_of((await ygg_profile(srv, id)).body as ExchangeableGameProfile);
  assert.equal(payload.textures.CAPE?.metadata, undefined);
});

// ── Authorization ───────────────────────────────────────────────────────

test("texture endpoints require a token that may manage the profile", async () => {
  const owner = await new_player();
  const stranger = await new_player();
  const session = await bound_session(owner);
  const id = unhyphenated(session.profile.id);
  const png = make_png(64, 32, unique_color());

  assert.equal(
    (
      await http(srv, "PUT", `/api/yggdrasil/api/user/profile/${id}/skin`, {
        raw_body: new FormData(),
      })
    ).status,
    401,
  );
  assert.equal(
    (
      await http(srv, "PUT", `/api/yggdrasil/api/user/profile/${id}/skin`, {
        token: "not-a-token",
        raw_body: new FormData(),
      })
    ).status,
    401,
  );

  // A valid token bound to a different account is forbidden.
  const stranger_auth = await ygg_authenticate(srv, stranger.user.email, stranger.user.password);
  expect_ygg_error(
    await ygg_put_texture(srv, stranger_auth.body.accessToken, id, "skin", png),
    403,
  );
  expect_ygg_error(await ygg_delete_texture(srv, stranger_auth.body.accessToken, id, "skin"), 403);

  // Unknown profiles cannot be managed either.
  expect_ygg_error(
    await ygg_put_texture(
      srv,
      session.access_token,
      "00000000000000000000000000000001",
      "skin",
      png,
    ),
    403,
  );

  // DELETE follows the same rules as PUT.
  assert.equal(
    (await http(srv, "DELETE", `/api/yggdrasil/api/user/profile/${id}/skin`)).status,
    401,
  );
  expect_ygg_error(await ygg_delete_texture(srv, stranger_auth.body.accessToken, id, "skin"), 403);
});

test("a General API token without a bound profile may upload to owned profiles", async () => {
  const player = await new_player();
  const id = unhyphenated(player.profiles[0].id);

  // General login tokens carry no Yggdrasil profile binding, but the profile
  // owner check still passes.
  expect_no_content(
    await ygg_put_texture(srv, player.access_token, id, "skin", make_png(64, 32, unique_color())),
  );
  const url = (await get_profile_detail(srv, player.profiles[0].id)).skin?.skin;
  assert.ok(url);
  expect_no_content(await ygg_delete_texture(srv, player.access_token, id, "skin"));
  assert.equal((await fetch_asset(url)).status, 404);
});

test("deleted profiles no longer accept texture uploads", async () => {
  const player = await new_player();
  const id = unhyphenated(player.profiles[0].id);
  await delete_profile(srv, player.access_token, player.profiles[0].id);

  expect_ygg_error(
    await ygg_put_texture(srv, player.access_token, id, "skin", make_png(64, 32, unique_color())),
    403,
  );
  expect_ygg_error(await ygg_delete_texture(srv, player.access_token, id, "skin"), 403);
});

test("an unknown but well-formed token is an authentication failure", async () => {
  const player = await new_player();
  const id = unhyphenated(player.profiles[0].id);
  const unknown = "0123456789abcdef0123456789abcdef";

  const put = await ygg_put_texture(srv, unknown, id, "skin", make_png(64, 32, unique_color()));
  assert.equal(put.status, 401, put.text);

  const deleted = await ygg_delete_texture(srv, unknown, id, "skin");
  assert.equal(deleted.status, 401, deleted.text);
});
