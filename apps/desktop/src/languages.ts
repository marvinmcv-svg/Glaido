/**
 * Dictation languages (spec 4.1): chosen by language, not region. The codes
 * are those our speech providers accept; names come from the browser's
 * Intl data, so they need no translation table.
 */
export const LANGUAGE_CODES = [
  "af",
  "am",
  "ar",
  "as",
  "az",
  "ba",
  "be",
  "bg",
  "bn",
  "bo",
  "br",
  "bs",
  "ca",
  "cs",
  "cy",
  "da",
  "de",
  "el",
  "en",
  "es",
  "et",
  "eu",
  "fa",
  "fi",
  "fo",
  "fr",
  "gl",
  "gu",
  "ha",
  "haw",
  "he",
  "hi",
  "hr",
  "ht",
  "hu",
  "hy",
  "id",
  "is",
  "it",
  "ja",
  "ka",
  "kk",
  "km",
  "kn",
  "ko",
  "la",
  "lb",
  "ln",
  "lo",
  "lt",
  "lv",
  "mg",
  "mi",
  "mk",
  "ml",
  "mn",
  "mr",
  "ms",
  "mt",
  "my",
  "ne",
  "nl",
  "nn",
  "no",
  "oc",
  "pa",
  "pl",
  "ps",
  "pt",
  "ro",
  "ru",
  "sa",
  "sd",
  "si",
  "sk",
  "sl",
  "sn",
  "so",
  "sq",
  "sr",
  "su",
  "sv",
  "sw",
  "ta",
  "te",
  "tg",
  "th",
  "tk",
  "tl",
  "tr",
  "tt",
  "uk",
  "ur",
  "uz",
  "vi",
  "yi",
  "yo",
  "yue",
  "zh",
] as const;

export type Language = { code: string; name: string; native: string };

/** English names for codes some Intl builds can't name. */
const FALLBACK: Record<string, string> = {
  as: "Assamese",
  ba: "Bashkir",
  bo: "Tibetan",
  br: "Breton",
  fo: "Faroese",
  haw: "Hawaiian",
  ht: "Haitian Creole",
  la: "Latin",
  ln: "Lingala",
  mg: "Malagasy",
  mi: "Māori",
  oc: "Occitan",
  sa: "Sanskrit",
  sd: "Sindhi",
  sn: "Shona",
  su: "Sundanese",
  tg: "Tajik",
  tk: "Turkmen",
  tt: "Tatar",
  yi: "Yiddish",
};

function displayName(code: string, locale: string): string | null {
  try {
    const name = new Intl.DisplayNames([locale], { type: "language", fallback: "none" }).of(code);
    return name && name !== code ? name : null;
  } catch {
    return null;
  }
}

/** Every language with its name in the UI locale and in itself, sorted by name. */
export function languages(uiLocale = "en"): Language[] {
  return LANGUAGE_CODES.map((code) => {
    const name = displayName(code, uiLocale) ?? FALLBACK[code] ?? code;
    return { code, name, native: displayName(code, code) ?? name };
  }).sort((a, b) => a.name.localeCompare(b.name, uiLocale));
}

/** Case- and accent-insensitive match on either name or the code. */
export function matchesLanguage(l: Language, query: string): boolean {
  const fold = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
  const q = fold(query.trim());
  return !q || fold(l.name).includes(q) || fold(l.native).includes(q) || l.code === q;
}

/** The system locale's language if we support it, else English. */
export function defaultLanguage(locale: string = navigator.language): string {
  const lang = locale.split(/[-_]/)[0]?.toLowerCase() ?? "";
  return (LANGUAGE_CODES as readonly string[]).includes(lang) ? lang : "en";
}
