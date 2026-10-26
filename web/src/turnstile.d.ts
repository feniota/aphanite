interface TurnstileRenderOptions {
  sitekey: string;
  size?: "normal" | "compact" | "flexible";
  theme?: "light" | "dark" | "auto";
  callback?: (token: string) => void;
}

interface Turnstile {
  render(container: string | HTMLElement, options: TurnstileRenderOptions): string;
  reset(widget_id?: string): void;
  remove(widget_id: string): void;
  getResponse(widget_id?: string): string | undefined;
}

// api.js assigns window.turnstile once loaded; window is globalThis.
declare var turnstile: Turnstile | undefined;
