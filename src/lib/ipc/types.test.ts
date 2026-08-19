import { describe, expect, it } from "vitest";

import { isCommandError } from "./types";

describe("isCommandError", () => {
  it("accepts a well-formed command error", () => {
    expect(isCommandError({ kind: "engineNotReady", message: "…" })).toBe(true);
  });

  it("rejects values missing either field", () => {
    expect(isCommandError({ kind: "x" })).toBe(false);
    expect(isCommandError({ message: "x" })).toBe(false);
  });

  it("rejects wrongly typed fields", () => {
    expect(isCommandError({ kind: 1, message: "x" })).toBe(false);
  });

  it("rejects null and primitives, which invoke can reject with", () => {
    expect(isCommandError(null)).toBe(false);
    expect(isCommandError("boom")).toBe(false);
    expect(isCommandError(undefined)).toBe(false);
  });
});
