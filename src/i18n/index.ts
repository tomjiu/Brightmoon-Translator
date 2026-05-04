import { create } from "zustand";
import zh from "./zh.json";
import en from "./en.json";

export type Locale = "zh" | "en";

const locales: Record<Locale, Record<string, unknown>> = { zh, en };

interface I18nState {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (key: string, params?: Record<string, string | number>) => string;
}

function getNestedValue(obj: Record<string, unknown>, path: string): string | undefined {
  const parts = path.split(".");
  let current: unknown = obj;
  for (const part of parts) {
    if (current && typeof current === "object" && part in (current as Record<string, unknown>)) {
      current = (current as Record<string, unknown>)[part];
    } else {
      return undefined;
    }
  }
  return typeof current === "string" ? current : undefined;
}

export const useI18n = create<I18nState>((set, get) => ({
  locale: (localStorage.getItem("locale") as Locale) || "zh",
  setLocale: (locale: Locale) => {
    localStorage.setItem("locale", locale);
    set({ locale });
  },
  t: (key: string, params?: Record<string, string | number>) => {
    const { locale } = get();
    const messages = locales[locale] || locales.zh;
    let value = getNestedValue(messages as Record<string, unknown>, key);

    if (value === undefined) {
      // Fallback to Chinese
      value = getNestedValue(zh as Record<string, unknown>, key);
    }

    if (value === undefined) {
      return key; // Return key if not found
    }

    if (params) {
      Object.entries(params).forEach(([k, v]) => {
        value = value!.replace(`{${k}}`, String(v));
      });
    }

    return value;
  },
}));
