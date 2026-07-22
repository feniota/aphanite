import { type DetailProfile, refresh, type User, validate, get_my_profiles } from "@/lib/api.ts";
import { Option } from "@/lib/utils.ts";

class AuthState {
  public token = $state<string | null>(localStorage.getItem("aphanite_token"));
  public user = $state<User | null>(null);
  public profiles = $state<Option<DetailProfile[]>>(Option.none());

  /** The timestamp(ms) of the time the token last acquired at */
  private acquired_on: number = -1; // $state is not needed since UI elements don't rely on this

  constructor() {
    const stored = localStorage.getItem("aphanite_user");
    if (stored) {
      try {
        const ts = Number.parseInt(localStorage.getItem("aphanite_token_acquired_on") || "-1");
        this.acquired_on = ts;
        this.user = JSON.parse(stored);
      } catch {
        this.acquired_on = -1;
      }
    }
  }

  set_session(token: string, user: User) {
    this.token = token;
    this.user = user;
    this.acquired_on = Date.now();
    localStorage.setItem("aphanite_token", token);
    localStorage.setItem("aphanite_user", JSON.stringify(user));
    localStorage.setItem("aphanite_token_acquired_on", this.acquired_on.toString());
  }

  logout() {
    this.token = null;
    this.user = null;
    this.acquired_on = -1;
    localStorage.removeItem("aphanite_token");
    localStorage.removeItem("aphanite_user");
    localStorage.removeItem("aphanite_token_acquired_on");
  }

  async init_profiles(): Promise<boolean> {
    if (this.profiles.is_none()) {
      if (!this.token) return false;
      const r = await get_my_profiles(this.token, true);
      if (!r.success) return false;
      this.profiles = Option.some(r.payload);
    }
    return true;
  }

  get is_logged_in() {
    return this.token !== null;
  }

  /** Check if the stored token is not expired; would refresh the token if it is older than a day */
  async validate(): Promise<boolean> {
    if (this.acquired_on === -1 || !this.is_logged_in) {
      return false;
    }

    // >1d, try to refresh
    if (Date.now() - this.acquired_on >= 24 * 3600 * 1000) {
      try {
        const res = await refresh(this.token!);
        if (!res.success) return false;
        this.set_session(res.payload.access_token, res.payload.user);
        return true;
      } catch {
        return false;
      }
    } else {
      try {
        const res = await validate(this.token!);
        return res.success;
      } catch {
        return false;
      }
    }
  }
}

export const AUTH = new AuthState();
