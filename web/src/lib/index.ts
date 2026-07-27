/**
 * shadcn-svelte shared libraries
 *
 * Re-exports cn() from "./utils.ts" so that we don't have to
 * have 2 copies of the same function. This re-export is only
 * for compability. Don't import cn() from here.
 *
 * @module
 */
export { cn } from "./utils.ts";

// deno-lint-ignore no-explicit-any
export type WithoutChild<T> = T extends { child?: any } ? Omit<T, "child"> : T;
// deno-lint-ignore no-explicit-any
export type WithoutChildren<T> = T extends { children?: any } ? Omit<T, "children"> : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & { ref?: U | null };
