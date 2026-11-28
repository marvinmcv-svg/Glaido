/** Query params preserved through signup and login (spec 4.12 / §5.6). */
export const PRESERVED_PARAMS = ["plan", "cadence", "source", "ref"] as const;

const ALLOWED: Record<(typeof PRESERVED_PARAMS)[number], RegExp> = {
  plan: /^(free|pro|team)$/,
  cadence: /^(monthly|annual)$/,
  source: /^[a-z0-9_-]{1,40}$/i,
  ref: /^[a-z0-9]{4,32}$/i,
};

/** Keep only known, well-formed attribution params. */
export function pickPreserved(
  input: URLSearchParams | Record<string, string | string[] | undefined>,
): URLSearchParams {
  const get = (k: string) =>
    input instanceof URLSearchParams
      ? input.get(k)
      : ([] as (string | undefined)[]).concat(input[k])[0];
  const out = new URLSearchParams();
  for (const key of PRESERVED_PARAMS) {
    const v = get(key);
    if (v && ALLOWED[key].test(v)) out.set(key, v);
  }
  return out;
}

export function withParams(path: string, params: URLSearchParams): string {
  const qs = params.toString();
  return qs ? `${path}?${qs}` : path;
}

/** Only allow same-site relative redirects after auth. */
export function safeNext(next: string | null | undefined, fallback = "/account"): string {
  if (!next || !next.startsWith("/") || next.startsWith("//") || next.includes("\\"))
    return fallback;
  return next;
}
