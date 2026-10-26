import { SvelteMap } from "svelte/reactivity";

export interface Toast {
  id: number;
  i18n_key: string;
  i18n_args?: Record<string, unknown>;
}

let NEXT_ID = $state(0);
let toasts = $state<Toast[]>([]);
const timers = new SvelteMap<number, ReturnType<typeof setTimeout>>();

export function toast(i18n_key: string, i18n_args?: Record<string, unknown>) {
  const id = NEXT_ID++;
  toasts.push({ id, i18n_key, i18n_args });
  const timer = setTimeout(() => dismiss(id), 4000);
  timers.set(id, timer);
}

export function dismiss(id: number) {
  clear_timer(id);
  toasts = toasts.filter(t => t.id !== id);
}

export function pause_timer(id: number) {
  const t = timers.get(id);
  if (t) clearTimeout(t);
}

export function resume_timer(id: number) {
  if (toasts.some(t => t.id === id)) {
    const timer = setTimeout(() => dismiss(id), 4000);
    timers.set(id, timer);
  }
}

function clear_timer(id: number) {
  const t = timers.get(id);
  if (t) {
    clearTimeout(t);
    timers.delete(id);
  }
}

export function get_toasts() {
  return toasts;
}
