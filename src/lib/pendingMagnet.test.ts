import { describe, expect, it } from "vitest";

import type { PendingMagnetStatus } from "./ipc/types";
import { describePendingMagnet, pendingMagnetName } from "./pendingMagnet";

const BASE: PendingMagnetStatus = {
  infoHash: "0123456789abcdef0123456789abcdef01234567",
  name: null,
  addedAt: null,
  state: "searching",
  searchingNow: false,
  failedAttempts: 0,
  nextAttemptInSeconds: null,
};

describe("describePendingMagnet", () => {
  it("never says downloading, in any state", () => {
    const states: PendingMagnetStatus[] = [
      BASE,
      { ...BASE, searchingNow: true },
      { ...BASE, failedAttempts: 1, nextAttemptInSeconds: 60 },
      { ...BASE, failedAttempts: 4, nextAttemptInSeconds: 900 },
      { ...BASE, state: "ready" },
    ];
    for (const magnet of states) {
      const { word, sentence } = describePendingMagnet(magnet);
      expect(word.toLowerCase()).not.toContain("download");
      // Every sentence promises the opposite, in so many words.
      expect(sentence).toMatch(/nothing (is )?download/i);
    }
  });

  it("says when it will try again, and how often it has", () => {
    expect(
      describePendingMagnet({
        ...BASE,
        failedAttempts: 1,
        nextAttemptInSeconds: 60,
      }).sentence,
    ).toMatch(/^No peer has answered yet\. Trying again in 1 min 00 s\./);
    expect(
      describePendingMagnet({
        ...BASE,
        failedAttempts: 3,
        nextAttemptInSeconds: 45,
      }).sentence,
    ).toMatch(/in 3 tries\. Trying again in 45 s\./);
  });

  it("asks the user to choose files once the list has arrived", () => {
    const { word, sentence } = describePendingMagnet({
      ...BASE,
      state: "ready",
    });
    expect(word).toBe("Ready");
    expect(sentence).toMatch(/choose files/i);
  });
});

describe("pendingMagnetName", () => {
  it("uses the name when there is one and a short hash when not", () => {
    expect(pendingMagnetName({ ...BASE, name: "something" })).toBe("something");
    expect(pendingMagnetName(BASE)).toBe("Magnet 01234567…");
  });
});
