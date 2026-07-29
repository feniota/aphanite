#!/usr/bin/env -S deno run --allow-read --allow-write
/**
 * Reformat Svelte component files to break long `cn()` class strings
 * at word boundaries around an 80-char line width limit.
 *
 * Keeps multiple class names on the same line when they fit, only wrapping
 * when the line would exceed the limit.
 */

import { readFile, writeFile } from "node:fs/promises";

async function* walk_svelte_files(dir: string): AsyncGenerator<string> {
  for await (const entry of Deno.readDir(dir)) {
    const path = `${dir}/${entry.name}`;
    if (entry.isDirectory) {
      yield* walk_svelte_files(path);
    } else if (entry.isFile && entry.name.endsWith(".svelte")) {
      yield path;
    }
  }
}

const LINE_LIMIT = 80;

/**
 * Break a class string into chunks that fit within LINE_LIMIT (including the
 * quotes and any indent), splitting at word boundaries.
 */
function word_wrap_class_string(s: string, _indent: string): string[] {
  const parts = s.trim().split(/\s+/);
  if (parts.length === 0) return [];
  if (parts.length === 1) return [JSON.stringify(parts[0])];

  // Each chunk becomes:  indent"class1 class2 ..."
  // We need to measure:  `"${classes}"`
  const chunks: string[] = [];
  let current: string[] = [];

  for (const p of parts) {
    const candidate = current.concat(p);
    const serialized = JSON.stringify(candidate.join(" "));
    if (serialized.length <= LINE_LIMIT) {
      current = candidate;
    } else {
      if (current.length > 0) {
        chunks.push(JSON.stringify(current.join(" ")));
      }
      current = [p];
    }
  }

  if (current.length > 0) {
    chunks.push(JSON.stringify(current.join(" ")));
  }

  return chunks;
}

/**
 * Process the content of a .svelte file, reformatting long `cn()` calls.
 */
function reformat_cn(content: string): string {
  const result: string[] = [];
  let i = 0;
  const len = content.length;

  while (i < len) {
    // Look for `cn(` pattern
    const cn_start = content.indexOf("cn(", i);
    if (cn_start === -1) {
      result.push(content.slice(i));
      break;
    }

    // Check it's not part of a longer identifier (e.g. `icn(`)
    if (cn_start > 0 && /[a-zA-Z0-9_$]/.test(content[cn_start - 1])) {
      result.push(content.slice(i, cn_start + 3));
      i = cn_start + 3;
      continue;
    }

    result.push(content.slice(i, cn_start));
    i = cn_start + 3; // skip "cn("

    // Find the matching closing paren, accounting for nesting
    let depth = 1;
    let j = i;
    while (j < len && depth > 0) {
      if (content[j] === "(") depth++;
      if (content[j] === ")") depth--;
      if (depth > 0) j++;
    }

    if (depth !== 0) {
      result.push(content.slice(cn_start));
      break;
    }

    const cn_args_str = content.slice(i, j);
    const args = parse_args(cn_args_str);
    if (args === null) {
      result.push(content.slice(cn_start, j + 1));
      i = j + 1;
      continue;
    }

    // Check if any string argument is too long
    const needs_split = args.some(a => a.type === "string" && a.value.length > LINE_LIMIT);
    if (!needs_split) {
      result.push(content.slice(cn_start, j + 1));
      i = j + 1;
      continue;
    }

    const indent = detect_indent(content, cn_start);
    const arg_indent = indent + "  ";
    const inner_indent = arg_indent + "  ";

    const new_args: string[] = [];
    for (const arg of args) {
      if (arg.type === "string" && arg.value.length > LINE_LIMIT) {
        const wrapped = word_wrap_class_string(arg.value, arg_indent);
        new_args.push(...wrapped);
      } else {
        new_args.push(arg.raw);
      }
    }

    // Re-wrap with cn() — both `cn(` and `)` were consumed, add them back
    result.push(
      "\n" +
        arg_indent +
        "cn(\n" +
        inner_indent +
        new_args.join(",\n" + inner_indent) +
        ",\n" +
        arg_indent +
        ")",
    );

    i = j + 1; // skip past the closing paren of cn()
  }

  return result.join("");
}

interface ParsedArg {
  type: "string" | "other";
  value: string;
  raw: string;
}

/** Parse comma-separated arguments in a `cn(...)` call, respecting string literals and nesting. */
function parse_args(s: string): ParsedArg[] | null {
  const args: ParsedArg[] = [];
  let i = 0;

  while (i < s.length) {
    // Skip whitespace
    while (i < s.length && /\s/.test(s[i])) i++;
    if (i >= s.length) break;

    const start = i;

    if (s[i] === '"' || s[i] === "'") {
      // String literal
      const quote = s[i];
      i++;
      let str = "";
      while (i < s.length) {
        if (s[i] === "\\") {
          str += s[i] + (s[i + 1] ?? "");
          i += 2;
        } else if (s[i] === quote) {
          i++;
          break;
        } else {
          str += s[i];
          i++;
        }
      }
      args.push({ type: "string", value: str, raw: s.slice(start, i) });
    } else {
      // Identifier, expression, or nested call — find until comma or end
      let depth = 0;
      while (i < s.length) {
        if (s[i] === "(" || s[i] === "[" || s[i] === "{") depth++;
        if (s[i] === ")" || s[i] === "]" || s[i] === "}") depth--;
        if (depth < 0) break; // we've crossed our closing boundary
        if (depth === 0 && s[i] === ",") break;
        if (s[i] === '"' || s[i] === "'") {
          // Skip string inside expression
          const q = s[i];
          i++;
          while (i < s.length && s[i] !== q) {
            if (s[i] === "\\") i++;
            i++;
          }
          i++; // skip closing quote
        } else {
          i++;
        }
      }
      const raw = s.slice(start, i).trim();
      if (raw) {
        args.push({ type: "other", value: raw, raw });
      }
      if (i < s.length && s[i] === ",") i++; // skip comma
    }
  }

  return args;
}

/** Detect the indentation at a given position in the source */
function detect_indent(content: string, pos: number): string {
  // Find the start of the line containing `pos`
  const line_start = content.lastIndexOf("\n", pos - 1);
  if (line_start === -1) return "";
  const line = content.slice(line_start + 1, pos);
  const m = line.match(/^(\s*)/);
  return m ? m[1] : "";
}

async function main() {
  const dirs = ["web/src/lib/components/ui", "web/src/components", "web/src/pages", "web/src/lib"];
  const files: string[] = [];
  for (const dir of dirs) {
    for await (const f of walk_svelte_files(dir)) {
      files.push(f);
    }
  }

  let modified_count = 0;

  for (const file of files.sort()) {
    const original = await readFile(file, "utf-8");
    const reformatted = reformat_cn(original);
    if (reformatted !== original) {
      await writeFile(file, reformatted, "utf-8");
      console.log(`✓ ${file}`);
      modified_count++;
    }
  }

  console.log(`\nReformatted ${modified_count} file(s).`);
}

await main();
