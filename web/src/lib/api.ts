const BASE = "/api";

type ApiResponse<T> =
  | { success: true; payload: T }
  | { success: false; reason: string };

async function request<T>(
  path: string,
  { token, ...init }: RequestInit & { token?: string } = {},
): Promise<T> {
  const headers: Record<string, string> = {};
  if (init.body && typeof init.body === "string") {
    headers["Content-Type"] = "application/json";
  }
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const res = await fetch(`${BASE}${path}`, { ...init, headers });
  const json: ApiResponse<T> = await res.json();

  if (!json.success) {
    throw new ApiError(json.reason, res.status);
  }

  return json.payload;
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

export function login(email: string, password?: string, otp_token?: string): Promise<LoginPayload> {
  return request<LoginPayload>("/auth/login", {
    method: "POST",
    body: JSON.stringify({ email, password, otp_token }),
  });
}

export function refresh(token: string): Promise<RefreshPayload> {
  return request<RefreshPayload>("/auth/refresh", { method: "POST", token });
}

export async function validate(token: string): Promise<void> {
  const res = await fetch(`${BASE}/auth/validate`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!res.ok) throw new ApiError("Token invalid", res.status);
}

// ── Register ──

export interface RegisterRequest {
  email: string;
  password: string;
  name?: string;
  turnstile_token?: string;
  register_token?: string;
}

export function register(data: RegisterRequest): Promise<User> {
  return request<User>("/register", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export function getTurnstileSiteKey(): Promise<TurnstilePayload> {
  return request<TurnstilePayload>("/turnstile");
}

// ── User Profile ──

export function getMe(token: string): Promise<User> {
  return request<User>("/users/me", { token });
}

export interface UpdateUserRequest {
  name?: string;
  email?: string;
}

export function updateMe(
  token: string,
  data: UpdateUserRequest,
): Promise<User> {
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

export async function changePassword(
  token: string,
  data: ChangePasswordRequest,
): Promise<void> {
  await fetch(`${BASE}/users/me/credentials/password`, {
    method: "PATCH",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify(data),
  });
}

// ── TOTP ──

export interface TotpPayload {
  secret: string;
  otpauth_url: string;
}

export function issueTotp(token: string): Promise<TotpPayload> {
  return request<TotpPayload>("/users/me/credentials/totp", {
    method: "POST",
    token,
  });
}

export async function activateTotp(
  token: string,
  otp_token: string,
): Promise<void> {
  await fetch(`${BASE}/users/me/credentials/totp`, {
    method: "PATCH",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ otp_token }),
  });
}

export async function deleteTotp(token: string): Promise<void> {
  await fetch(`${BASE}/users/me/credentials/totp`, {
    method: "DELETE",
    headers: { Authorization: `Bearer ${token}` },
  });
}

export interface VerificationPayload {
  id: string;
}

export function createVerification(
  email: string,
  method: string,
): Promise<VerificationPayload> {
  return request<VerificationPayload>("/verification", {
    method: "POST",
    body: JSON.stringify({ email, method }),
  });
}

export interface OtpTokenPayload {
  otp_token: string;
}

export function completeVerification(
  id: string,
  code: string,
): Promise<OtpTokenPayload> {
  return request<OtpTokenPayload>(`/verification/${id}`, {
    method: "POST",
    body: JSON.stringify({ code }),
  });
}
