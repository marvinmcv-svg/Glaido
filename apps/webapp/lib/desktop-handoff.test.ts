import { describe, expect, it, vi } from "vitest";
import {
  challengeFor,
  desktopCallbackUrl,
  exchange,
  type ExchangeDeps,
  hashCode,
  isChallenge,
  isState,
  isVerifier,
  loginRedirect,
  newCode,
} from "./desktop-handoff";

// S256 pair computed independently (Python hashlib + urlsafe_b64encode).
const VERIFIER = "vocalis-test-verifier-0123456789-abcdefghijkl";
const CHALLENGE = "6c-bTGbAeoNzckoIrB4Np1vr5_T2iIdZ8welx-D9f7E";

const session = {
  accessToken: "a",
  refreshToken: "r",
  expiresAt: 1,
  user: { id: "u1", email: "u@example.com" },
};

function deps(stored: { userId: string; challenge: string } | null): ExchangeDeps & {
  consumed: string[];
} {
  const consumed: string[] = [];
  return {
    consumed,
    consume: vi.fn((h: string) => {
      consumed.push(h);
      const s = stored;
      stored = null; // single use
      return Promise.resolve(s);
    }),
    mintSession: vi.fn(() => Promise.resolve(session)),
  };
}

describe("desktop handoff", () => {
  it("computes RFC 7636 S256 challenges", () => {
    expect(challengeFor(VERIFIER)).toBe(CHALLENGE);
    expect(challengeFor(VERIFIER + "x")).not.toBe(CHALLENGE);
  });

  it("validates inputs strictly", () => {
    expect(isChallenge(CHALLENGE)).toBe(true);
    expect(isChallenge(CHALLENGE + "=")).toBe(false);
    expect(isChallenge("short")).toBe(false);
    expect(isState("abcdefghijklmnop")).toBe(true);
    expect(isState("bad state with spaces")).toBe(false);
    expect(isVerifier(VERIFIER)).toBe(true);
    expect(isVerifier("x".repeat(42))).toBe(false);
    expect(isVerifier(42)).toBe(false);
  });

  it("makes unguessable codes and hashes them for storage", () => {
    const a = newCode();
    expect(a).toMatch(/^[A-Za-z0-9_-]{43}$/);
    expect(newCode()).not.toBe(a);
    expect(hashCode(a)).toMatch(/^[0-9a-f]{64}$/);
  });

  it("builds the deep link and the login round trip", () => {
    expect(desktopCallbackUrl("c0de", "st&te")).toBe(
      "vocalis://auth/callback?code=c0de&state=st%26te",
    );
    const login = new URL(loginRedirect(CHALLENGE, "abcdefghijklmnop"), "https://x");
    expect(login.pathname).toBe("/login");
    expect(login.searchParams.get("next")).toBe(
      `/auth/desktop?challenge=${CHALLENGE}&state=abcdefghijklmnop`,
    );
  });

  it("exchanges a valid code once", async () => {
    const code = newCode();
    const verifier = VERIFIER;
    const d = deps({ userId: "u1", challenge: CHALLENGE });
    expect(await exchange({ code, verifier }, d)).toEqual({ ok: true, session });
    expect(d.consumed).toEqual([hashCode(code)]);
    expect(d.mintSession).toHaveBeenCalledWith("u1");
    expect(await exchange({ code, verifier }, d)).toMatchObject({ error: "invalid_grant" });
  });

  it("rejects a wrong verifier and burns the code", async () => {
    const code = newCode();
    const d = deps({ userId: "u1", challenge: CHALLENGE });
    expect(await exchange({ code, verifier: "z".repeat(43) }, d)).toMatchObject({
      ok: false,
      error: "invalid_grant",
    });
    expect(d.mintSession).not.toHaveBeenCalled();
    expect(await exchange({ code, verifier: VERIFIER }, d)).toMatchObject({
      error: "invalid_grant",
    });
  });

  it("rejects malformed bodies without touching storage", async () => {
    const d = deps({ userId: "u1", challenge: CHALLENGE });
    for (const body of [null, {}, { code: "x", verifier: VERIFIER }, "str"]) {
      expect(await exchange(body, d)).toMatchObject({ status: 400, error: "invalid_request" });
    }
    expect(d.consumed).toEqual([]);
  });

  it("reports a server error when no session can be minted", async () => {
    const d = deps({ userId: "u1", challenge: CHALLENGE });
    d.mintSession = vi.fn(() => Promise.resolve(null));
    const verifier = VERIFIER;
    expect(await exchange({ code: newCode(), verifier }, d)).toMatchObject({ status: 500 });
  });
});
