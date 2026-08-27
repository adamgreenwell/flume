import { describe, expect, it } from "vitest";

import { serverWindowControls, windowControlsFor } from "./platform";

describe("windowControlsFor", () => {
  it("reserves the left inset on macOS, where the traffic lights are", () => {
    expect(
      windowControlsFor(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15",
      ),
    ).toBe("left");
  });

  it("reserves the right inset on Windows and Linux", () => {
    expect(windowControlsFor("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")).toBe(
      "right",
    );
    expect(windowControlsFor("Mozilla/5.0 (X11; Linux x86_64)")).toBe("right");
  });

  it("treats an unrecognised agent as not-a-Mac", () => {
    // macOS agents always say "Macintosh". An unrecognised string is far more
    // likely to be a Linux build, and guessing macOS would put an 88px hole in
    // the left of the title bar on every Linux machine.
    expect(windowControlsFor("")).toBe("right");
  });

  it("renders the macOS inset at build time, before any agent exists", () => {
    // The static export has no navigator. macOS is the smaller reservation,
    // so a briefly-too-small inset is less visible than a too-large one, and
    // the client corrects it before anything is painted.
    expect(serverWindowControls()).toBe("left");
  });
});
