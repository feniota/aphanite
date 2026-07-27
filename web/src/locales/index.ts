import type { Resource } from "i18next";

import en from "./en/translation.json" with { type: "json" };
import zh from "./zh-CN/translation.json" with { type: "json" };

export const resources: Resource = {
  "zh-CN": { translation: zh },
  en: { translation: en },
};
