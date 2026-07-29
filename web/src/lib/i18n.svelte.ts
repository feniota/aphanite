import i18next from "i18next";
import LanguageDetector from "i18next-browser-languagedetector";

import { resources } from "@/locales/index.ts";

let _t = $state<((key: string, opts?: Record<string, unknown>) => string) | null>(null);
let current_language = $state("zh-CN");

export async function init_i18n(): Promise<void> {
  const tf = await i18next.use(LanguageDetector).init({
    resources,
    fallbackLng: "en",
    detection: {
      order: ["localStorage", "navigator"],
      lookupLocalStorage: "aphanite_lang",
      caches: ["localStorage"],
    },
    interpolation: {
      escapeValue: false,
    },
    returnObjects: false,
  });
  _t = tf;
  current_language = i18next.language;
}

/**
 * Translate a key. MUST be called within a Svelte reactive context (component or `$derived`).
 * Switching language via `change_language()` triggers re-render of all components using `t()`.
 */
export function t(key: string, opts?: Record<string, unknown>): string {
  // This access makes `t()` reactive to language changes
  void current_language;
  return _t?.(key, opts) ?? key;
}

/**
 * Change the current language. Persists to localStorage and triggers reactive updates.
 */
export function change_language(lng: string): void {
  void i18next.changeLanguage(lng);
  current_language = lng;
  document.documentElement.lang = lng;
}

/**
 * Returns the current language code (e.g. "zh-CN", "en").
 */
export function get_current_language(): string {
  void current_language;
  return current_language;
}
