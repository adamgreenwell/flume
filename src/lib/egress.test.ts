import { describe, expect, it } from "vitest";

import { describeGuard, guardRailLabel } from "./egress";
import type { GuardStatus, Verdict } from "./ipc/types";

const status = (
  verdict: Verdict,
  over: Partial<GuardStatus> = {},
): GuardStatus => ({
  guard: "hold",
  report: {
    path: { v4: { interface: "en7", kind: "ordinary" }, v6: null },
    verdict,
  },
  held: false,
  resumesInSeconds: null,
  ...over,
});

const direct: Verdict = { verdict: "direct", interface: "en7" };
const tunnelled: Verdict = {
  verdict: "tunnelled",
  interface: "utun12",
  otherFamilyOutside: false,
};
const pinned: Verdict = {
  verdict: "pinned",
  interface: "Local Area Connection",
  otherFamilyOutside: false,
};

describe("describeGuard", () => {
  it("says nothing at all while the guard is off", () => {
    // The backend publishes a verdict every tick whatever the mode, so this
    // has to suppress deliberately rather than by accident.
    expect(describeGuard(status(direct, { guard: "off" }))).toBeNull();
    expect(describeGuard(null)).toBeNull();
  });

  it("never calls a pinned interface a tunnel", () => {
    // The distinction the whole Verdict::Pinned variant exists to preserve:
    // transfer runs because the user said so, not because Flume agrees.
    const note = describeGuard(status(pinned));
    expect(note).not.toBeNull();
    expect(note?.title).not.toMatch(/tunnel/i);
    expect(note?.body).toMatch(/could not identify/i);
    expect(note?.body).toMatch(/your word for it/i);
  });

  it("says the engine is stopped and the torrents are not", () => {
    // Both halves matter. The first is the guarantee; the second is why the
    // user's library is not damaged, which is the thing they will fear.
    const note = describeGuard(status(direct, { held: true }));

    expect(note?.severity).toBe("err");
    expect(note?.title).toMatch(/nothing is running/i);
    expect(note?.body).toMatch(/not started its torrent engine/i);
    expect(note?.body).toMatch(/were not paused/i);
    expect(note?.body).toMatch(/no files were touched/i);
  });

  it("names the interface that caused the hold", () => {
    // A status that does not say which interface leaves the user with nothing
    // to check and nothing to fix.
    const note = describeGuard(status(direct, { held: true }));
    expect(note?.title).toContain("en7");
  });

  it("offers re-pinning when the pin is what is holding transfer", () => {
    const note = describeGuard(
      status(
        { verdict: "wrongTunnel", interface: "utun12", expected: "utun6" },
        { held: true },
      ),
    );

    expect(note?.title).toContain("utun12");
    expect(note?.title).toContain("utun6");
    expect(note?.body).toMatch(/pin utun12 instead/i);
    expect(note?.body).toMatch(/clear the pin/i);
  });

  it("counts down rather than leaving the wait unexplained", () => {
    const note = describeGuard(
      status(tunnelled, { held: true, resumesInSeconds: 6 }),
    );

    expect(note?.severity).toBe("neutral");
    expect(note?.title).toMatch(/resumes in 6 s/);
    expect(note?.body).toMatch(/utun12/);
    expect(note?.body).toMatch(/re-announce/i);
  });

  it("stops counting down at zero rather than saying 'in 0 s'", () => {
    const note = describeGuard(
      status(tunnelled, { held: true, resumesInSeconds: 0 }),
    );
    expect(note?.title).toMatch(/resuming now/i);
    expect(note?.title).not.toMatch(/0 s/);
  });

  it("reports an IPv6 leak without claiming transfer is stopped", () => {
    // The decision on this feature: IPv4 decides, IPv6 is reported alongside.
    const note = describeGuard(
      status(
        { verdict: "tunnelled", interface: "utun12", otherFamilyOutside: true },
        {
          report: {
            path: {
              v4: { interface: "utun12", kind: "tunnel" },
              v6: { interface: "en7", kind: "ordinary" },
            },
            verdict: {
              verdict: "tunnelled",
              interface: "utun12",
              otherFamilyOutside: true,
            },
          },
        },
      ),
    );

    expect(note?.severity).toBe("warn");
    expect(note?.title).toMatch(/IPv6/);
    expect(note?.title).toContain("en7");
    expect(note?.body).toMatch(/not held over this/i);
  });

  it("warns without claiming anything was stopped in warn mode", () => {
    const note = describeGuard(status(direct, { guard: "warn" }));

    expect(note?.severity).toBe("warn");
    expect(note?.body).toMatch(/nothing has been stopped/i);
  });

  it("never says a bare state word as a title", () => {
    // Flume's rule: a status carries its cause. "Held" is not a status.
    const cases: GuardStatus[] = [
      status(direct, { held: true }),
      status(direct, { guard: "warn" }),
      status(tunnelled),
      status(pinned),
      status({ verdict: "unknown" }, { held: true }),
      status(tunnelled, { held: true, resumesInSeconds: 6 }),
    ];

    for (const candidate of cases) {
      const note = describeGuard(candidate);
      expect(note).not.toBeNull();
      expect(
        note!.title.split(/\s+/).length,
        `"${note!.title}" is too short to be a claim`,
      ).toBeGreaterThan(3);
      expect(note!.body.length).toBeGreaterThan(80);
    }
  });

  it("never claims a tunnel it could not confirm", () => {
    for (const candidate of [
      status({ verdict: "unknown" }, { held: true }),
      status({ verdict: "unknown" }, { guard: "warn" }),
    ]) {
      const note = describeGuard(candidate);
      // Matching on the words "protected" or "covered" was the first attempt
      // and it was wrong: the copy says "nothing here says you are covered",
      // which is the denial, and a bare word match cannot see the negation.
      // What actually matters is that an unsettled verdict never asserts the
      // interface IS a tunnel, and that it says so in the hedged voice.
      expect(note?.body).not.toMatch(/\bis a tunnel\b/i);
      expect(note?.title).not.toMatch(/\bis a tunnel\b/i);
      expect(note?.body).toMatch(
        /cannot|could not|does not guess|not evidence/i,
      );
    }
  });
});

describe("guardRailLabel", () => {
  it("is silent while the guard is off", () => {
    expect(guardRailLabel(status(direct, { guard: "off" }))).toBeNull();
  });

  it("carries the fact in words, not colour alone", () => {
    expect(guardRailLabel(status(tunnelled))).toBe("Leaves by utun12 · tunnel");
    expect(guardRailLabel(status(direct, { guard: "warn" }))).toBe(
      "Leaves by en7 · not a tunnel",
    );
    // The rail is the one place the user looks to see what the network is
    // doing, so "held" alone -- effect without cause -- is not enough there
    // either.
    expect(guardRailLabel(status(direct, { held: true }))).toBe(
      "Held · en7 is not a tunnel",
    );
    expect(guardRailLabel(status({ verdict: "unknown" }, { held: true }))).toBe(
      "Held · no route Flume can identify",
    );
    expect(
      guardRailLabel(status(tunnelled, { held: true, resumesInSeconds: 6 })),
    ).toBe("Held · resumes in 6 s");
  });

  it("does not call a pinned interface a tunnel in the rail either", () => {
    const label = guardRailLabel(status(pinned));
    expect(label).toMatch(/pinned/);
    expect(label).not.toMatch(/tunnel/i);
  });
});
