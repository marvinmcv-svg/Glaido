/**
 * Desktop sign-in handoff (ADR-0012), framework-free so it can be unit tested.
 *
 * 1. The desktop app makes a PKCE verifier and opens
 *    `/auth/desktop?challenge=<S256(verifier)>&state=<random>`.
 * 2. A signed-in user confirms; the server stores a one-time code (hashed)
 *    bound to the challenge and sends the browser to
 *    `vocalis://auth/callback?code=…&state=…`.
 * 3. The desktop app checks `state` and POSTs `{ code, verifier }` to
 *    `/api/desktop/exchange`, which deletes the code (single use, 5 minutes),
 *    checks the verifier and returns a fresh session for that user only.
 */
import { createHash, randomBytes, timingSafeEqual } from "node:crypto";

export const DESKTOP_SCHEME = "vocalis";

const B64URL_43 = /^[A-Za-z0-9_-]{43}$/;
const STATE = /^[A-Za-z0-9_-]{16,128}$/;
const VERIFIER = /^[A-Za-z0-9._~-]{43,128}$/; // RFC 7636 §4.1

export const isChallenge = (s: unknown): s is string => typeof s === "string" && B64URL_43.test(s);
export const isState = (s: unknown): s is string => typeof s === "string" && STATE.test(s);
export const isVerifier = (s: unknown): s is string => typeof s === "string" && VERIFIER.test(s);
export const isCode = (s: unknown): s is string => typeof s === "string" && B64URL_43.test(s);

/** RFC 7636 S256: base64url(sha256(verifier)), no padding. */
export function challengeFor(verifier: string): string {
  return createHash("sha256").update(verifier, "ascii").digest("base64url");
}

/** 256-bit one-time code, base64url (43 chars). */
export function newCode(): string {
  return randomBytes(32).toString("base64url");
}

/** Codes are stored hashed, so a database leak can't be redeemed. */
export function hashCode(code: string): string {
  return createHash("sha256").update(code, "ascii").digest("hex");
}

export function sameChallenge(a: string, b: string): boolean {
  const x = Buffer.from(a);
  const y = Buffer.from(b);
  return x.length === y.length && timingSafeEqual(x, y);
}

export function desktopCallbackUrl(code: string, state: string): string {
  const q = new URLSearchParams({ code, state });
  return `${DESKTOP_SCHEME}://auth/callback?${q}`;
}

/** Where to send a signed-out user so they come back here after login. */
export function loginRedirect(challenge: string, state: string): string {
  const back = `/auth/desktop?${new URLSearchParams({ challenge, state })}`;
  return `/login?${new URLSearchParams({ next: back })}`;
}

export type Session = {
  accessToken: string;
  refreshToken: string;
  expiresAt: number;
  user: { id: string; email: string };
};

/** What the exchange needs from Supabase; injected so it can be tested. */
export type ExchangeDeps = {
  /** Delete the unexpired handoff with this code hash; return it if there was one. */
  consume: (codeHash: string) => Promise<{ userId: string; challenge: string } | null>;
  /** Mint a brand-new session for the user (independent of their browser session). */
  mintSession: (userId: string) => Promise<Session | null>;
};

export type ExchangeResult =
  | { ok: true; session: Session }
  | { ok: false; status: 400 | 500; error: "invalid_request" | "invalid_grant" | "server_error" };

export async function exchange(body: unknown, deps: ExchangeDeps): Promise<ExchangeResult> {
  const { code, verifier } = (body ?? {}) as Record<string, unknown>;
  if (!isCode(code) || !isVerifier(verifier))
    return { ok: false, status: 400, error: "invalid_request" };
  // Consume first: a wrong verifier still burns the code, so it can't be guessed at.
  const handoff = await deps.consume(hashCode(code));
  if (!handoff || !sameChallenge(challengeFor(verifier), handoff.challenge))
    return { ok: false, status: 400, error: "invalid_grant" };
  const session = await deps.mintSession(handoff.userId);
  return session ? { ok: true, session } : { ok: false, status: 500, error: "server_error" };
}
