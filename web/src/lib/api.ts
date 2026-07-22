const BASE = "/api";

type ApiResponse<T> =
  | { success: true; payload: T; status: number }
  | { success: false; reason: string; status: number };

/**
 * Wrapper around `fetch()` that automatically fills `Content-Type` and access token, and performs validations on the response type.
 * JSON bodies MUST be `JSON.stringify()`-ed (the same as how you use `fetch()`), otherwise it would be `[object Object]`.
 */
async function request<T>(
  path: string,
  { token, ...init }: RequestInit & { token?: string } = {},
): Promise<ApiResponse<T>> {
  const headers: Record<string, string> = {};
  if (init.body && typeof init.body === "string") {
    headers["Content-Type"] = "application/json";
  }
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const res = await fetch(`${BASE}${path}`, { ...init, headers });

  if (res.status === 204) {
    return { success: true, payload: null as T, status: 204 };
  }

  let json: unknown;
  try {
    json = await res.json();
  } catch {
    throw new ApiError("Server returned non-JSON response", res.status);
  }

  if (typeof json !== "object" || json === null || !("success" in json)) {
    throw new ApiError("Server returned malformed response", res.status);
  }

  return { ...(json as object), status: res.status } as ApiResponse<T>;
}

export class ApiError extends Error {
  status: number;
  constructor(message: string, status: number) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}

// ── Types ──

export type Permission = "management";

export interface User {
  id: string;
  name: string;
  email: string;
  permissions: Permission[];
}

export interface Profile {
  id: string;
  name: string;
  owner: string;
}

export interface ProfileSkin {
  skin?: string;
  model?: "default" | "slim";
  cape?: string;
}

export interface DetailProfile {
  metadata: Profile;
  skin?: ProfileSkin;
}

export interface LoginPayload {
  access_token: string;
  client_token: string;
  user: User;
}

export interface RefreshPayload {
  access_token: string;
  user: User;
}

export interface TurnstilePayload {
  site_key: string;
}

// ── Auth ──

export function login(
  email: string,
  password?: string,
  otp_token?: string,
): Promise<ApiResponse<LoginPayload>> {
  return request<LoginPayload>("/auth/login", {
    method: "POST",
    body: JSON.stringify({ email, password, otp_token }),
  });
}

export function refresh(token: string): Promise<ApiResponse<RefreshPayload>> {
  return request<RefreshPayload>("/auth/refresh", { method: "POST", token });
}

export function validate(token: string): Promise<ApiResponse<null>> {
  return request<null>("/auth/validate", { token });
}

// ── Register ──

export interface RegisterRequest {
  email: string;
  password: string;
  name?: string;
  turnstile_token?: string;
  register_token?: string;
}

export function register(data: RegisterRequest): Promise<ApiResponse<User>> {
  return request<User>("/register", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export function get_turnstile_site_key(): Promise<ApiResponse<TurnstilePayload>> {
  return request<TurnstilePayload>("/turnstile");
}

// ── User Profile ──

export function get_my_profiles(
  token: string,
  with_skin?: boolean,
): Promise<ApiResponse<DetailProfile[]>> {
  const query = with_skin !== undefined ? `?with_skin=${with_skin}` : "";
  return request<DetailProfile[]>(`/users/me/profiles${query}`, { token });
}

export function get_me(token: string): Promise<ApiResponse<User>> {
  return request<User>("/users/me", { token });
}

export interface UpdateUserRequest {
  name?: string;
  email?: string;
}

export function update_me(token: string, data: UpdateUserRequest): Promise<ApiResponse<User>> {
  return request<User>("/users/me", {
    method: "PATCH",
    body: JSON.stringify(data),
    token,
  });
}

export interface ChangePasswordRequest {
  old_password?: string;
  otp_token?: string;
  new_password: string;
}

export function change_password(
  token: string,
  data: ChangePasswordRequest,
): Promise<ApiResponse<null>> {
  return request<null>("/users/me/credentials/password", {
    method: "PATCH",
    body: JSON.stringify(data),
    token,
  });
}

// ── TOTP ──

export interface TotpPayload {
  secret: string;
  otpauth_url: string;
}

export function issue_totp(token: string): Promise<ApiResponse<TotpPayload>> {
  return request<TotpPayload>("/users/me/credentials/totp", {
    method: "POST",
    token,
  });
}

export function activate_totp(token: string, otp_token: string): Promise<ApiResponse<null>> {
  return request<null>("/users/me/credentials/totp", {
    method: "PATCH",
    body: JSON.stringify({ otp_token }),
    token,
  });
}

export function delete_totp(token: string): Promise<ApiResponse<null>> {
  return request<null>("/users/me/credentials/totp", {
    method: "DELETE",
    token,
  });
}

export interface VerificationPayload {
  id: string;
}

export function create_verification(
  email: string,
  method: string,
): Promise<ApiResponse<VerificationPayload>> {
  return request<VerificationPayload>("/verification", {
    method: "POST",
    body: JSON.stringify({ email, method }),
  });
}

export interface OtpTokenPayload {
  otp_token: string;
}

export function complete_verification(
  id: string,
  code: string,
): Promise<ApiResponse<OtpTokenPayload>> {
  return request<OtpTokenPayload>(`/verification/${id}`, {
    method: "POST",
    body: JSON.stringify({ code }),
  });
}

// ── Admin ──

export function list_users(token: string): Promise<ApiResponse<User[]>> {
  return request<User[]>("/users", { token });
}

export interface CreateUserRequest {
  email: string;
  name?: string;
  permissions: Permission[];
}

export interface CreateUserResponse {
  id: string;
  name: string;
  email: string;
  permissions: Permission[];
  password: string;
}

export function create_user(
  token: string,
  data: CreateUserRequest,
): Promise<ApiResponse<CreateUserResponse>> {
  return request<CreateUserResponse>("/user", {
    method: "POST",
    body: JSON.stringify(data),
    token,
  });
}

export function get_user_profiles(
  token: string,
  id: string,
  with_skin?: boolean,
): Promise<ApiResponse<DetailProfile[]>> {
  const query = with_skin !== undefined ? `?with_skin=${with_skin}` : "";
  return request<DetailProfile[]>(`/users/${id}/profiles${query}`, { token });
}
