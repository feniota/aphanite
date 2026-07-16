export interface Toast {
  id: number;
  message: string;
}

let nextId = $state(0);
let toasts = $state<Toast[]>([]);
const timers = new Map<number, ReturnType<typeof setTimeout>>();

export function show(message: string) {
  const id = nextId++;
  toasts = [...toasts, { id, message }];
  const timer = setTimeout(() => dismiss(id), 4000);
  timers.set(id, timer);
}

export function dismiss(id: number) {
  clearTimer(id);
  toasts = toasts.filter((t) => t.id !== id);
}

export function pauseTimer(id: number) {
  const t = timers.get(id);
  if (t) clearTimeout(t);
}

export function resumeTimer(id: number) {
  if (toasts.some((t) => t.id === id)) {
    const timer = setTimeout(() => dismiss(id), 4000);
    timers.set(id, timer);
  }
}

function clearTimer(id: number) {
  const t = timers.get(id);
  if (t) {
    clearTimeout(t);
    timers.delete(id);
  }
}

export function getToasts() {
  return toasts;
}
