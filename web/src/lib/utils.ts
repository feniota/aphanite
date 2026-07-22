import { clsx, type ClassValue } from "clsx";
import { tick } from "svelte";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/**
 * Wrapper around document.startViewTransition().
 *
 * If View Transition API is found, this function would call the input function in a
 * transition, in this case a Svelte tick() is automatically appended.
 *
 * Otherwise, this would call the function directly, without tick()-ing.
 *
 * @param f - the function that would change the page layout. Can be async or not.
 * @returns Some(ViewTransition) if the API is found; or None otherwise.
 */
export function transition_tick(f: () => unknown): Option<ViewTransition> {
  // @ts-expect-error: View Transition API is not THAT baseline yet
  if (window.document.startViewTransition) {
    return Option.some(
      document.startViewTransition(async () => {
        await f();
        await tick();
      }),
    );
  } else {
    f();
    return Option.none();
  }
}

/**
 * Simple JS representation of [Option<T>](https://doc.rust-lang.org/stable/std/option/enum.Option.html)
 */
export class Option<T> {
  private empty: boolean;
  public value?: T;

  constructor(empty: boolean, value?: T) {
    this.empty = empty;
    if (!empty) {
      this.value = value;
    }
  }

  public static some<T>(value: T) {
    return new Option(false, value);
  }

  public static none<T>() {
    return new Option<T>(true);
  }

  public is_some() {
    return !this.empty;
  }

  public is_none() {
    return this.empty;
  }

  /**
   * Equivalent to Rust `is_some_and()` method.
   *
   * @param f - The statements to run when this is not empty. If this is empty, nothing is performed.
   * @returns the return value of that function, if this is not empty, or `void` otherwise.
   */
  public is_some_and<R>(f: (arg0: T) => R): R | void {
    if (!this.empty) return f(this.value!);
  }
}

export function trim_start_matches(
  input: string,
  match: string | RegExp | ((arg0: string) => boolean),
): string {
  if (typeof match === "string") {
    let result = input;
    while (true) {
      if (result.length < match.length) return result;
      if (result.startsWith(match)) {
        result = result.slice(match.length);
      } else return result;
    }
  } else if (typeof match === "function") {
    for (let i = 0; i < input.length; i++) {
      if (!match(input.charAt(i))) {
        return input.slice(i);
      }
    }
    return input;
  } else if (match instanceof RegExp) {
    let src = match.source;
    const flags = match.flags;
    if (!src.startsWith("^")) {
      src = `^${src}`;
    }
    const regexp = new RegExp(src, flags);

    return input.replace(regexp, "");
  } else {
    throw new TypeError(
      `Excepted \`match\` to be string, RegExp, or a function; got ${typeof match}.`,
    );
  }
}

export function trim_end_matches(
  input: string,
  match: string | RegExp | ((arg0: string) => boolean),
): string {
  if (typeof match === "string") {
    let result = input;
    while (true) {
      if (result.length < match.length) return result;
      if (result.endsWith(match)) {
        result = result.slice(0, -match.length);
      } else return result;
    }
  } else if (typeof match === "function") {
    for (let i = input.length - 1; i >= 0; i--) {
      if (!match(input.charAt(i))) {
        return input.slice(0, i + 1);
      }
    }
    return "";
  } else if (match instanceof RegExp) {
    let src = match.source;
    const flags = match.flags;
    if (!src.endsWith("$")) {
      src = `${src}$`;
    }
    const regexp = new RegExp(src, flags);

    return input.replace(regexp, "");
  } else {
    throw new TypeError(
      `Excepted \`match\` to be string, RegExp, or a function; got ${typeof match}.`,
    );
  }
}
