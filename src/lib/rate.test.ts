import { describe, expect, it } from "vitest";

import { BYTES_PER_KB, fromKbInput, toKbInput } from "./rate";

describe("toKbInput", () => {
  it("renders null as empty, meaning unlimited", () => {
    expect(toKbInput(null)).toBe("");
  });

  it("converts bytes to whole kilobytes", () => {
    expect(toKbInput(BYTES_PER_KB)).toBe("1");
    expect(toKbInput(BYTES_PER_KB * 512)).toBe("512");
  });

  it("rounds rather than showing a fraction", () => {
    expect(toKbInput(1500)).toBe("1");
  });
});

describe("fromKbInput", () => {
  it("treats empty and whitespace as unlimited", () => {
    expect(fromKbInput("")).toBeNull();
    expect(fromKbInput("   ")).toBeNull();
  });

  it("converts kilobytes to bytes", () => {
    expect(fromKbInput("1")).toBe(BYTES_PER_KB);
    expect(fromKbInput("256")).toBe(BYTES_PER_KB * 256);
  });

  it("maps zero to unlimited, since the backend rejects a zero limit", () => {
    expect(fromKbInput("0")).toBeNull();
  });

  it("maps negatives to unlimited", () => {
    expect(fromKbInput("-5")).toBeNull();
  });

  it("maps unparseable input to unlimited rather than NaN", () => {
    expect(fromKbInput("abc")).toBeNull();
    expect(fromKbInput("1e999")).toBeNull();
  });

  it("round-trips a typical value", () => {
    expect(toKbInput(fromKbInput("512"))).toBe("512");
  });
});
