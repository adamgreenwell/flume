import { describe, expect, it } from "vitest";

import { looksLikeMagnet } from "./magnet";

describe("looksLikeMagnet", () => {
  it("accepts a v1 info hash", () => {
    expect(
      looksLikeMagnet(
        "magnet:?xt=urn:btih:d160b8d8ea35a5b4e52837468fc8f03d55cef1f7",
      ),
    ).toBe(true);
  });

  it("accepts a v2 multihash", () => {
    expect(looksLikeMagnet("magnet:?xt=urn:btmh:1220caf1e1c30e81cb361")).toBe(
      true,
    );
  });

  it("accepts extra parameters and surrounding whitespace", () => {
    expect(
      looksLikeMagnet(
        "  magnet:?xt=urn:btih:abc123&dn=ubuntu.iso&tr=udp%3A%2F%2Ftracker  ",
      ),
    ).toBe(true);
  });

  it("is case-insensitive", () => {
    expect(looksLikeMagnet("MAGNET:?XT=URN:BTIH:ABC123")).toBe(true);
  });

  it("rejects a magnet for non-BitTorrent content", () => {
    // A bare `magnet:` scheme is not necessarily a torrent.
    expect(looksLikeMagnet("magnet:?xt=urn:sha1:ABCDEF")).toBe(false);
  });

  it("rejects ordinary text and URLs", () => {
    expect(looksLikeMagnet("")).toBe(false);
    expect(looksLikeMagnet("https://example.com/x.torrent")).toBe(false);
    expect(looksLikeMagnet("just some copied text")).toBe(false);
  });

  it("rejects a magnet with no info hash", () => {
    expect(looksLikeMagnet("magnet:?dn=something")).toBe(false);
  });
});
