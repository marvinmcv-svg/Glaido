import { describe, expect, it } from "vitest";
import { pickPreserved, safeNext, withParams } from "./params";

describe("pickPreserved", () => {
  it("keeps valid attribution params and drops the rest", () => {
    const p = pickPreserved(new URLSearchParams("plan=pro&cadence=annual&source=pricing&ref=abc123&token=secret"));
    expect(p.toString()).toBe("plan=pro&cadence=annual&source=pricing&ref=abc123");
  });
  it("rejects malformed values", () => {
    const p = pickPreserved({ plan: "ultra", cadence: "weekly", source: "<script>", ref: "x" });
    expect(p.toString()).toBe("");
  });
  it("takes the first of repeated values", () => {
    expect(pickPreserved({ plan: ["pro", "free"] }).get("plan")).toBe("pro");
  });
});

describe("safeNext", () => {
  it("allows relative paths only", () => {
    expect(safeNext("/billing")).toBe("/billing");
    expect(safeNext("//evil.example")).toBe("/account");
    expect(safeNext("https://evil.example")).toBe("/account");
    expect(safeNext("/\\evil")).toBe("/account");
    expect(safeNext(null)).toBe("/account");
  });
});

describe("withParams", () => {
  it("appends only when there are params", () => {
    expect(withParams("/signup", new URLSearchParams())).toBe("/signup");
    expect(withParams("/signup", new URLSearchParams("plan=pro"))).toBe("/signup?plan=pro");
  });
});
