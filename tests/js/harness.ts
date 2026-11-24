//! Black-box HTTP test harness for Aphanite.
//!
//! Spawns the real server binary with a generated configuration (temporary
//! Turso database, local storage, random port) and talks to it over HTTP only.

import assert from "node:assert/strict";
import { type ChildProcess, spawn } from "node:child_process";
import {
  createHash,
  createHmac,
  createPublicKey,
  generateKeyPairSync,
  randomBytes,
} from "node:crypto";
import { once } from "node:events";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { deflateSync } from "node:zlib";

const REPO_ROOT = path.resolve(import.meta.dirname, "..", "..");

export const BINARY_PATH =
  process.env.APHANITE_BIN ??
  path.join(
    REPO_ROOT,
    "target",
    "debug",
    process.platform === "win32" ? "aphanite.exe" : "aphanite",
  );

export const ADMIN_EMAIL = "admin@aphanite.test";
export const ADMIN_PASSWORD = "admin-password-123";

export type ServerRef = { base_url: string };

export type TestServer = ServerRef & {
  port: number;
  tmp_dir: string;
  server_name: string;
  admin_token: string;
  admin_user: UserPayload;
  logs: () => string;
  stop: () => Promise<void>;
};

// ── Wire types ──────────────────────────────────────────────────────────

export type ApiResponse<T> = { success: true; payload: T } | { success: false; reason: string };

export type UserPayload = {
  id: string;
  name: string;
  email: string;
  permissions: string[];
};

export type ProfilePayload = {
  id: string;
  name: string;
  owner: string;
};

export type LoginPayload = {
  access_token: string;
  client_token: string;
  user: UserPayload;
};

export type ProfileProperty = {
  name: string;
  value: string;
  signature?: string;
};

export type ExchangeableGameProfile = {
  id: string;
  name: string;
  properties?: ProfileProperty[];
};

export type TexturesPayload = {
  timestamp: number;
  profileId: string;
  profileName: string;
  textures: Record<string, { url: string; metadata?: { model?: string } }>;
};

export type AuthenticateResponse = {
  accessToken: string;
  clientToken: string;
  availableProfiles: ExchangeableGameProfile[];
  selectedProfile?: ExchangeableGameProfile;
  user?: {
    id: string;
    properties: { name: string; value: string }[];
  };
};

export type RefreshResponse = {
  accessToken: string;
  clientToken: string;
  selectedProfile?: ExchangeableGameProfile;
  user?: AuthenticateResponse["user"];
};

export type YggdrasilErrorBody = {
  error: string;
  errorMessage: string;
  cause?: string;
};

export type MetaResponse = {
  meta: {
    serverName?: string;
    implementationName: string;
    implementationVersion: string;
    links?: { homepage?: string; register?: string };
  };
  skinDomains: string[];
  signaturePublickey: string;
};

export type ProfileDetail = {
  metadata: ProfilePayload;
  skin?: { skin?: string; model?: string; cape?: string };
};

export type TestUser = UserPayload & { password: string };

// ── HTTP primitives ─────────────────────────────────────────────────────

export type HttpResult = { status: number; headers: Headers; text: string };

export type HttpOptions = {
  token?: string;
  json?: unknown;
  raw_body?: BodyInit;
  headers?: Record<string, string>;
};

export async function http(
  srv: ServerRef,
  method: string,
  url_path: string,
  options: HttpOptions = {},
): Promise<HttpResult> {
  const headers = new Headers(options.headers);
  let body: BodyInit | undefined = options.raw_body;
  if (options.json !== undefined) {
    headers.set("content-type", "application/json");
    body = JSON.stringify(options.json);
  }
  if (options.token !== undefined) {
    headers.set("authorization", `Bearer ${options.token}`);
  }

  const request = (): Promise<Response> =>
    fetch(new URL(url_path, srv.base_url), {
      method,
      headers,
      body,
      redirect: "manual",
    });

  // A server may answer before the request body is fully sent (e.g. an
  // early 401/403 on a multipart upload), which the client can observe as
  // a connection-level failure on a pooled connection. Idempotent methods
  // are safe to retry once.
  const idempotent = ["GET", "HEAD", "PUT", "DELETE", "OPTIONS"].includes(method.toUpperCase());
  let response: Response;
  try {
    response = await request();
  } catch (error) {
    if (!idempotent) throw error;
    response = await request();
  }
  return {
    status: response.status,
    headers: response.headers,
    text: await response.text(),
  };
}

export function parse_json<T>(result: { status: number; text: string }): T {
  assert.notEqual(result.text.length, 0, `expected a JSON body, got status ${result.status}`);
  return JSON.parse(result.text) as T;
}

export async function http_json<T>(
  srv: ServerRef,
  method: string,
  url_path: string,
  options: HttpOptions = {},
): Promise<HttpResult & { body: T }> {
  const result = await http(srv, method, url_path, options);
  return { ...result, body: parse_json<T>(result) };
}

export function expect_success<T>(result: {
  status: number;
  text: string;
  body: ApiResponse<T>;
}): T {
  assert.equal(result.status, 200, result.text);
  assert.equal(result.body.success, true, result.text);
  return (result.body as { success: true; payload: T }).payload;
}

// ── General API helpers ─────────────────────────────────────────────────

export async function general_login(
  srv: ServerRef,
  email: string,
  password: string,
): Promise<LoginPayload> {
  const result = await http_json<ApiResponse<LoginPayload>>(srv, "POST", "/api/auth/login", {
    json: { email, password },
  });
  return expect_success(result);
}

export type CreateUserOptions = {
  email?: string;
  name?: string;
  permissions?: string[];
};

export async function create_user(
  srv: ServerRef,
  admin_token: string,
  options: CreateUserOptions = {},
): Promise<TestUser> {
  const email = options.email ?? unique_email();
  const result = await http_json<ApiResponse<TestUser>>(srv, "POST", "/api/user", {
    token: admin_token,
    json: {
      email,
      name: options.name ?? unique_user_name(),
      permissions: options.permissions ?? [],
    },
  });
  const payload = expect_success(result);
  assert.equal(payload.email, email);
  return payload;
}

export async function create_profile(
  srv: ServerRef,
  token: string,
  name = unique_profile_name(),
): Promise<ProfilePayload> {
  const result = await http_json<ApiResponse<ProfilePayload>>(srv, "POST", "/api/profile", {
    token,
    json: { name },
  });
  return expect_success(result);
}

export async function get_profile_detail(
  srv: ServerRef,
  profile_id: string,
  with_skin = true,
): Promise<ProfileDetail> {
  const result = await http_json<ApiResponse<ProfileDetail>>(
    srv,
    "GET",
    `/api/profiles/${profile_id}?with_skin=${with_skin}`,
  );
  return expect_success(result);
}

export async function list_my_profiles(
  srv: ServerRef,
  token: string,
  with_skin = true,
): Promise<ProfileDetail[]> {
  const result = await http_json<ApiResponse<ProfileDetail[]>>(
    srv,
    "GET",
    `/api/users/me/profiles?with_skin=${with_skin}`,
    { token },
  );
  return expect_success(result);
}

export async function rename_profile(
  srv: ServerRef,
  token: string,
  profile_id: string,
  name: string,
): Promise<ProfilePayload> {
  const result = await http_json<ApiResponse<ProfilePayload>>(
    srv,
    "PATCH",
    `/api/profiles/${profile_id}`,
    { token, json: { name } },
  );
  return expect_success(result);
}

export async function delete_profile(
  srv: ServerRef,
  token: string,
  profile_id: string,
): Promise<ProfilePayload> {
  const result = await http_json<ApiResponse<ProfilePayload>>(
    srv,
    "DELETE",
    `/api/profiles/${profile_id}`,
    { token },
  );
  return expect_success(result);
}

export async function general_validate(srv: ServerRef, token: string): Promise<number> {
  return (await http(srv, "GET", "/api/auth/validate", { token })).status;
}

// ── Yggdrasil helpers ───────────────────────────────────────────────────

const YGG = "/api/yggdrasil";

export async function ygg_authenticate(
  srv: ServerRef,
  username: string,
  password: string,
  options: { client_token?: string; request_user?: boolean } = {},
): Promise<HttpResult & { body: AuthenticateResponse }> {
  const json: Record<string, unknown> = {
    username,
    password,
    requestUser: options.request_user ?? false,
    agent: { name: "Minecraft", version: 1 },
  };
  if (options.client_token !== undefined) json.clientToken = options.client_token;
  return http_json<AuthenticateResponse>(srv, "POST", `${YGG}/authserver/authenticate`, { json });
}

export async function ygg_refresh(
  srv: ServerRef,
  access_token: string,
  options: {
    client_token?: string;
    request_user?: boolean;
    selected_profile?: { id: string; name: string };
  } = {},
): Promise<HttpResult & { body: RefreshResponse }> {
  const json: Record<string, unknown> = {
    accessToken: access_token,
    requestUser: options.request_user ?? false,
  };
  if (options.client_token !== undefined) json.clientToken = options.client_token;
  if (options.selected_profile !== undefined) json.selectedProfile = options.selected_profile;
  return http_json<RefreshResponse>(srv, "POST", `${YGG}/authserver/refresh`, { json });
}

export async function ygg_validate(
  srv: ServerRef,
  access_token: string,
  client_token?: string,
): Promise<HttpResult> {
  const json: Record<string, unknown> = { accessToken: access_token };
  if (client_token !== undefined) json.clientToken = client_token;
  return http(srv, "POST", `${YGG}/authserver/validate`, { json });
}

export async function ygg_invalidate(srv: ServerRef, access_token: string): Promise<HttpResult> {
  return http(srv, "POST", `${YGG}/authserver/invalidate`, { json: { accessToken: access_token } });
}

export async function ygg_signout(
  srv: ServerRef,
  username: string,
  password: string,
): Promise<HttpResult> {
  return http(srv, "POST", `${YGG}/authserver/signout`, { json: { username, password } });
}

export async function ygg_join(
  srv: ServerRef,
  access_token: string,
  profile_id: string,
  server_id: string,
): Promise<HttpResult> {
  return http(srv, "POST", `${YGG}/sessionserver/session/minecraft/join`, {
    json: { accessToken: access_token, selectedProfile: profile_id, serverId: server_id },
  });
}

export async function ygg_has_joined(
  srv: ServerRef,
  username: string,
  server_id: string,
  ip?: string,
): Promise<HttpResult> {
  const params = new URLSearchParams({ username, serverId: server_id });
  if (ip !== undefined) params.set("ip", ip);
  return http(srv, "GET", `${YGG}/sessionserver/session/minecraft/hasJoined?${params}`);
}

export async function ygg_profile(
  srv: ServerRef,
  profile_id: string,
  unsigned?: boolean,
): Promise<HttpResult & { body: ExchangeableGameProfile | undefined }> {
  const suffix = unsigned === undefined ? "" : `?unsigned=${unsigned}`;
  const result = await http(
    srv,
    "GET",
    `${YGG}/sessionserver/session/minecraft/profile/${profile_id}${suffix}`,
  );
  return {
    ...result,
    body: result.status === 200 ? parse_json<ExchangeableGameProfile>(result) : undefined,
  };
}

export async function ygg_profiles_by_name(
  srv: ServerRef,
  names: string[],
): Promise<HttpResult & { body: ExchangeableGameProfile[] }> {
  return http_json<ExchangeableGameProfile[]>(srv, "POST", `${YGG}/api/profiles/minecraft`, {
    json: names,
  });
}

export function ygg_meta(srv: ServerRef): Promise<HttpResult & { body: MetaResponse }> {
  return http_json<MetaResponse>(srv, "GET", `${YGG}/`);
}

export async function ygg_put_texture(
  srv: ServerRef,
  access_token: string,
  profile_id: string,
  texture_type: "skin" | "cape",
  png: Buffer,
  model?: string,
): Promise<HttpResult> {
  const form = new FormData();
  if (model !== undefined) form.append("model", model);
  form.append("file", new Blob([new Uint8Array(png)], { type: "image/png" }), "texture.png");
  return http(srv, "PUT", `${YGG}/api/user/profile/${profile_id}/${texture_type}`, {
    token: access_token,
    raw_body: form,
  });
}

export async function ygg_delete_texture(
  srv: ServerRef,
  access_token: string,
  profile_id: string,
  texture_type: "skin" | "cape",
): Promise<HttpResult> {
  return http(srv, "DELETE", `${YGG}/api/user/profile/${profile_id}/${texture_type}`, {
    token: access_token,
  });
}

export async function fetch_asset(
  url: string,
): Promise<{ status: number; headers: Headers; bytes: Buffer }> {
  const response = await fetch(url);
  return {
    status: response.status,
    headers: response.headers,
    bytes: Buffer.from(await response.arrayBuffer()),
  };
}

export function textures_of(profile: ExchangeableGameProfile): TexturesPayload {
  const property = profile.properties?.find(p => p.name === "textures");
  assert.ok(property, "profile has no textures property");
  return JSON.parse(Buffer.from(property.value, "base64").toString("utf8")) as TexturesPayload;
}

export function property_of(
  profile: ExchangeableGameProfile,
  name: string,
): ProfileProperty | undefined {
  return profile.properties?.find(p => p.name === name);
}

// ── Crypto helpers ──────────────────────────────────────────────────────

// System OpenSSL builds (e.g. Fedora's) refuse RSA-SHA1 signature operations,
// so verify PKCS#1 v1.5 ourselves: RSA public operation + DigestInfo compare.
const SHA1_DIGEST_INFO_PREFIX = Buffer.from("3021300906052b0e03021a05000414", "hex");

export function verify_signature(
  public_key_pem: string,
  value: string,
  signature_base64: string,
): boolean {
  const jwk = createPublicKey(public_key_pem).export({ format: "jwk" });
  assert.ok(jwk.n && jwk.e, "signature public key must be an RSA key");

  const modulus = base64url_to_bigint(jwk.n);
  const exponent = base64url_to_bigint(jwk.e);
  const signature = bytes_to_bigint(Buffer.from(signature_base64, "base64"));
  const key_size = Math.ceil(modulus.toString(2).length / 8);
  if (signature >= modulus) return false;

  const encoded = bigint_to_bytes(mod_pow(signature, exponent, modulus), key_size);
  const digest_info = Buffer.concat([
    SHA1_DIGEST_INFO_PREFIX,
    createHash("sha1").update(value, "utf8").digest(),
  ]);
  const padding_length = key_size - digest_info.length - 3;
  if (padding_length < 8) return false;

  return (
    encoded[0] === 0x00 &&
    encoded[1] === 0x01 &&
    encoded.subarray(2, 2 + padding_length).every(byte => byte === 0xff) &&
    encoded[2 + padding_length] === 0x00 &&
    encoded.subarray(3 + padding_length).equals(digest_info)
  );
}

function base64url_to_bigint(value: string): bigint {
  return bytes_to_bigint(Buffer.from(value, "base64url"));
}

function bytes_to_bigint(bytes: Buffer): bigint {
  return bytes.length === 0 ? 0n : BigInt(`0x${bytes.toString("hex")}`);
}

function bigint_to_bytes(value: bigint, length: number): Buffer {
  return Buffer.from(value.toString(16).padStart(length * 2, "0"), "hex");
}

function mod_pow(base: bigint, exponent: bigint, modulus: bigint): bigint {
  let result = 1n;
  let factor = base % modulus;
  let power = exponent;
  while (power > 0n) {
    if (power & 1n) result = (result * factor) % modulus;
    factor = (factor * factor) % modulus;
    power >>= 1n;
  }
  return result;
}

export function totp_code(secret_base32: string, at_ms = Date.now()): string {
  const counter = Math.floor(at_ms / 1000 / 30);
  const message = Buffer.alloc(8);
  message.writeBigUInt64BE(BigInt(counter));
  const digest = createHmac("sha1", base32_decode(secret_base32)).update(message).digest();
  const offset = digest[digest.length - 1] & 0x0f;
  const binary = digest.readUInt32BE(offset) & 0x7fffffff;
  return String(binary % 1_000_000).padStart(6, "0");
}

function base32_decode(input: string): Buffer {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  let bits = 0;
  let value = 0;
  const output: number[] = [];
  for (const character of input.replace(/=+$/, "").toUpperCase()) {
    const index = alphabet.indexOf(character);
    assert.notEqual(index, -1, `invalid Base32 character: ${character}`);
    value = (value << 5) | index;
    bits += 5;
    if (bits >= 8) {
      output.push((value >>> (bits - 8)) & 0xff);
      bits -= 8;
    }
  }
  return Buffer.from(output);
}

// ── PNG helpers ─────────────────────────────────────────────────────────

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(data: Buffer): number {
  let crc = 0xffffffff;
  for (const byte of data) {
    crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function png_chunk(type: string, data: Buffer): Buffer {
  const type_bytes = Buffer.from(type, "ascii");
  const chunk = Buffer.alloc(12 + data.length);
  chunk.writeUInt32BE(data.length, 0);
  type_bytes.copy(chunk, 4);
  data.copy(chunk, 8);
  chunk.writeUInt32BE(crc32(Buffer.concat([type_bytes, data])), 8 + data.length);
  return chunk;
}

export function make_png(
  width: number,
  height: number,
  rgba: [number, number, number, number] = [0x33, 0x66, 0x99, 0xff],
): Buffer {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // color type: RGBA
  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const offset = y * (stride + 1) + 1 + x * 4;
      raw[offset] = rgba[0];
      raw[offset + 1] = rgba[1];
      raw[offset + 2] = rgba[2];
      raw[offset + 3] = rgba[3];
    }
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    png_chunk("IHDR", ihdr),
    png_chunk("IDAT", deflateSync(raw)),
    png_chunk("IEND", Buffer.alloc(0)),
  ]);
}

export function png_size(png: Buffer): { width: number; height: number } {
  assert.equal(png.subarray(0, 8).toString("hex"), "89504e470d0a1a0a");
  return { width: png.readUInt32BE(16), height: png.readUInt32BE(20) };
}

// ── Identifiers ─────────────────────────────────────────────────────────

let unique_counter = 0;

function unique_id(): string {
  unique_counter += 1;
  return `${process.pid.toString(36)}${unique_counter.toString(36)}${randomBytes(3).toString("hex")}`;
}

export function unique_email(): string {
  return `u${unique_id()}@aphanite.test`;
}

export function unique_user_name(): string {
  return `U${unique_id()}`.slice(0, 20);
}

export function unique_profile_name(prefix = "P"): string {
  return `${prefix}${randomBytes(5).toString("hex")}`.slice(0, 16);
}

export function unique_server_id(): string {
  return randomBytes(16).toString("hex");
}

let color_counter = 0;

/** Solid RGBA colour that differs on every call, for de-duplication-free tests. */
export function unique_color(): [number, number, number, number] {
  color_counter += 1;
  return [color_counter & 0xff, (color_counter * 7) & 0xff, (color_counter * 13) & 0xff, 0xff];
}

export function unhyphenated(uuid: string): string {
  return uuid.replaceAll("-", "");
}

// ── Server lifecycle ────────────────────────────────────────────────────

const running_servers = new Set<ChildProcess>();
process.once("exit", () => {
  for (const child of running_servers) child.kill("SIGKILL");
});

type SpawnedServer = {
  child: ChildProcess;
  logs: () => string;
};

async function assert_binary(): Promise<void> {
  try {
    await fs.access(BINARY_PATH);
  } catch {
    throw new Error(
      `Aphanite binary not found at ${BINARY_PATH}. Run \`cargo build\` first or set APHANITE_BIN.`,
    );
  }
}

async function free_port(): Promise<number> {
  const server = net.createServer();
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  assert.ok(address !== null && typeof address === "object");
  const port = address.port;
  await new Promise<void>(resolve => server.close(() => resolve()));
  return port;
}

async function run_cli(
  args: string[],
  timeout_ms: number,
): Promise<{ code: number | null; output: string }> {
  const child = spawn(BINARY_PATH, args, {
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, RUST_LOG: "aphanite=error" },
  });
  let output = "";
  child.stdout?.on("data", chunk => (output += chunk.toString()));
  child.stderr?.on("data", chunk => (output += chunk.toString()));
  const timer = setTimeout(() => child.kill("SIGKILL"), timeout_ms);
  const [code] = (await once(child, "exit")) as [number | null, NodeJS.Signals | null];
  clearTimeout(timer);
  return { code, output };
}

function toml_string(value: string): string {
  return JSON.stringify(value);
}

function render_config(
  port: number,
  data_dir: string,
  server_name: string,
  private_key_pem: string,
): string {
  const domain = `127.0.0.1:${port}`;
  return `# Generated by tests/js/harness.ts
[service]
listen = "127.0.0.1"
port = ${port}
domain = ${toml_string(domain)}
name = ${toml_string(server_name)}
data_path = ${toml_string(data_dir)}
tls = false
client_ip = "disabled"
public = false

[service.turnstile]
enabled = false

[storage]
type = "local"

[storage.local]
path = ${toml_string(path.join(data_dir, "assets"))}

[database]
backend = "turso"

[yggdrasil]
private_key = '''
${private_key_pem}'''
homepage = ${toml_string(`http://${domain}/`)}
`;
}

async function spawn_server(config_path: string): Promise<SpawnedServer> {
  const child = spawn(BINARY_PATH, ["--config", config_path], {
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, RUST_LOG: process.env.RUST_LOG ?? "aphanite=warn" },
  });
  running_servers.add(child);

  let logs = "";
  const append = (chunk: Buffer) => {
    logs += chunk.toString();
    if (logs.length > 100_000) logs = logs.slice(-50_000);
  };
  child.stdout?.on("data", append);
  child.stderr?.on("data", append);
  return { child, logs: () => logs };
}

async function wait_until_ready(
  base_url: string,
  server: SpawnedServer,
  timeout_ms = 60_000,
): Promise<void> {
  const deadline = Date.now() + timeout_ms;
  while (Date.now() < deadline) {
    if (server.child.exitCode !== null) {
      throw new Error(`server exited with code ${server.child.exitCode}\n${server.logs()}`);
    }
    try {
      const response = await fetch(`${base_url}/api/yggdrasil/`, {
        signal: AbortSignal.timeout(1000),
      });
      if (response.status === 200) return;
    } catch {
      // Not listening yet.
    }
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw new Error(`timed out waiting for aphanite to listen\n${server.logs()}`);
}

export type StartServerOptions = {
  server_name?: string;
};

export async function start_server(options: StartServerOptions = {}): Promise<TestServer> {
  await assert_binary();

  const tmp_dir = await fs.mkdtemp(path.join(os.tmpdir(), "aphanite-js-"));
  const config_path = path.join(tmp_dir, "config.toml");
  const server_name = options.server_name ?? "Aphanite JS Test";

  const { privateKey } = generateKeyPairSync("rsa", { modulusLength: 2048 });
  const private_key_pem = privateKey.export({ type: "pkcs8", format: "pem" }).toString();

  try {
    for (let attempt = 1; attempt <= 3; attempt++) {
      const port = await free_port();
      const base_url = `http://127.0.0.1:${port}`;

      await fs.writeFile(config_path, render_config(port, tmp_dir, server_name, private_key_pem));

      if (attempt === 1) {
        const created = await run_cli(
          [
            "--config",
            config_path,
            "create-admin",
            "--email",
            ADMIN_EMAIL,
            "--password",
            ADMIN_PASSWORD,
          ],
          120_000,
        );
        if (created.code !== 0) {
          throw new Error(`failed to create admin user (exit ${created.code})\n${created.output}`);
        }
      }

      const server = await spawn_server(config_path);
      try {
        await wait_until_ready(base_url, server);
      } catch (error) {
        server.child.kill("SIGKILL");
        if (attempt === 3) throw error;
        continue;
      }

      try {
        const admin = await general_login({ base_url }, ADMIN_EMAIL, ADMIN_PASSWORD);
        let stopped = false;
        return {
          base_url,
          port,
          tmp_dir,
          server_name,
          admin_token: admin.access_token,
          admin_user: admin.user,
          logs: server.logs,
          stop: async () => {
            if (stopped) return;
            stopped = true;
            if (server.child.exitCode === null) {
              server.child.kill("SIGTERM");
              const exited = await Promise.race([
                once(server.child, "exit").then(() => true),
                new Promise<boolean>(resolve => setTimeout(() => resolve(false), 5000)),
              ]);
              if (!exited) server.child.kill("SIGKILL");
            }
            running_servers.delete(server.child);
            await fs.rm(tmp_dir, { recursive: true, force: true });
          },
        };
      } catch (error) {
        server.child.kill("SIGKILL");
        throw new Error(
          `server started but admin login failed: ${String(error)}\n${server.logs()}`,
          {
            cause: error,
          },
        );
      }
    }
  } catch (error) {
    await fs.rm(tmp_dir, { recursive: true, force: true });
    throw error;
  }

  throw new Error("unreachable");
}
