export interface Toast {
  id: number;
  message: string;
}

let NEXT_ID = $state(0);
let toasts = $state<Toast[]>([]);
const timers = new Map<number, ReturnType<typeof setTimeout>>();

export function show(message: string) {
  const id = NEXT_ID++;
  toasts = [...toasts, { id, message }];
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
