import type { User } from "./api";

class AuthState {
  token = $state<string | null>(localStorage.getItem("aphanite_token"));
  user = $state<User | null>(null);

  constructor() {
    const stored = localStorage.getItem("aphanite_user");
    if (stored) {
      try {
        this.user = JSON.parse(stored);
      } catch {
        // ignore
      }
    }
  }

  set_session(token: string, user: User) {
    this.token = token;
    this.user = user;
    localStorage.setItem("aphanite_token", token);
    localStorage.setItem("aphanite_user", JSON.stringify(user));
  }

  logout() {
    this.token = null;
    this.user = null;
    localStorage.removeItem("aphanite_token");
    localStorage.removeItem("aphanite_user");
  }

  get is_logged_in() {
    return this.token !== null;
  }
}

export const AUTH = new AuthState();
