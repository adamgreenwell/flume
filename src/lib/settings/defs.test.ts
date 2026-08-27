import { describe, expect, it } from "vitest";

import type { Settings } from "@/lib/ipc/types";

import { SECTIONS, SETTING_DEFS, searchSettings } from "./defs";

const SETTINGS: Settings = {
  downloadDir: "/Volumes/Media/Linux",
  listenPort: 42221,
  enableDht: true,
  enableUpnp: true,
  downloadLimitBps: null,
  uploadLimitBps: 2_000_000,
  proxyUrl: null,
  theme: "system",
  density: "comfortable",
};

describe("the definition table", () => {
  it("covers every field of Settings", () => {
    // The screen is generated from this table, so a field missing here is a
    // setting the user simply cannot reach — and nothing else would notice.
    const defined = new Set(SETTING_DEFS.map((d) => d.id));
    for (const key of Object.keys(SETTINGS) as (keyof Settings)[]) {
      expect(defined.has(key), `${key} has no definition`).toBe(true);
    }
  });

  it("defines each field exactly once", () => {
    const ids = SETTING_DEFS.map((d) => d.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("puts every setting in a section that exists", () => {
    const sections = new Set(SECTIONS.map((s) => s.id));
    for (const def of SETTING_DEFS) {
      expect(sections.has(def.section), `${def.id} → ${def.section}`).toBe(
        true,
      );
    }
  });

  it("leaves no section empty", () => {
    // An empty section is a nav entry that leads nowhere.
    for (const section of SECTIONS) {
      expect(
        SETTING_DEFS.some((d) => d.section === section.id),
        `${section.id} has no settings`,
      ).toBe(true);
    }
  });

  it("gives every setting a consequence that says something", () => {
    // The feature. A setting without one does not ship.
    for (const def of SETTING_DEFS) {
      const text = (def.consequence as (v: unknown) => string)(
        SETTINGS[def.id],
      );
      expect(text.length, `${def.id} says nothing`).toBeGreaterThan(20);
    }
  });

  it("writes a consequence that changes with the value", () => {
    // Static help text dressed as a consequence is the failure mode this
    // guards against — it reads like the feature and is not.
    const dht = SETTING_DEFS.find((d) => d.id === "enableDht");
    const on = (dht?.consequence as (v: unknown) => string)(true);
    const off = (dht?.consequence as (v: unknown) => string)(false);

    expect(on).not.toBe(off);
  });

  it("makes a rate limit concrete rather than restating the number", () => {
    const limit = SETTING_DEFS.find((d) => d.id === "downloadLimitBps");
    const capped = (limit?.consequence as (v: unknown) => string)(5_000_000);

    // A duration for a real download, not just the rate echoed back.
    expect(capped).toMatch(/\d+\s*min/);
    expect(capped).toContain("4.70 GB");
  });

  it("says what unlimited actually costs", () => {
    const limit = SETTING_DEFS.find((d) => d.id === "downloadLimitBps");
    const uncapped = (limit?.consequence as (v: unknown) => string)(null);

    expect(uncapped.toLowerCase()).toContain("no cap");
  });

  it("marks the settings that rebuild the session", () => {
    // Getting this wrong either restarts the engine needlessly or promises a
    // change that silently does not take effect.
    const restarts = new Set(
      SETTING_DEFS.filter((d) => d.restartsSession).map((d) => d.id),
    );

    expect(restarts).toContain("listenPort");
    expect(restarts).toContain("enableDht");
    expect(restarts).toContain("proxyUrl");
    // Rate limits swap in live; claiming otherwise would be a lie.
    expect(restarts).not.toContain("downloadLimitBps");
    expect(restarts).not.toContain("uploadLimitBps");
    expect(restarts).not.toContain("theme");
  });
});

describe("searchSettings", () => {
  it("returns everything for a blank query", () => {
    expect(searchSettings("", SETTINGS)).toHaveLength(SETTING_DEFS.length);
    expect(searchSettings("   ", SETTINGS)).toHaveLength(SETTING_DEFS.length);
  });

  it("matches the plain-language label", () => {
    const found = searchSettings("download speed", SETTINGS);
    expect(found.map((d) => d.id)).toContain("downloadLimitBps");
  });

  it("matches the config key, for people who know it", () => {
    const found = searchSettings("net.dht", SETTINGS);
    expect(found.map((d) => d.id)).toEqual(["enableDht"]);
  });

  it("matches jargon the label deliberately avoids", () => {
    // The label says "Find peers through the DHT". Someone looking for
    // "trackerless" should still land on it.
    expect(searchSettings("trackerless", SETTINGS).map((d) => d.id)).toContain(
      "enableDht",
    );
    expect(searchSettings("socks5", SETTINGS).map((d) => d.id)).toContain(
      "proxyUrl",
    );
  });

  it("matches the section name", () => {
    const found = searchSettings("appearance", SETTINGS);
    expect(found.map((d) => d.id)).toContain("theme");
  });

  it("matches the consequence as it currently reads", () => {
    // Someone who remembers "magnet links will not work" can find the setting
    // that says it — often the only phrasing they ever saw.
    const found = searchSettings("magnet links work", SETTINGS);
    expect(found.map((d) => d.id)).toContain("enableDht");
  });

  it("searches the consequence for the value in force, not the default", () => {
    const off: Settings = { ...SETTINGS, enableDht: false };

    // "will not work at all" only appears in the disabled wording.
    expect(
      searchSettings("will not work at all", off).map((d) => d.id),
    ).toEqual(["enableDht"]);
    expect(searchSettings("will not work at all", SETTINGS)).toEqual([]);
  });

  it("ignores case and surrounding space", () => {
    expect(searchSettings("  DHT  ", SETTINGS).map((d) => d.id)).toContain(
      "enableDht",
    );
  });

  it("returns nothing for a query that matches nothing", () => {
    expect(searchSettings("kubernetes", SETTINGS)).toEqual([]);
  });

  it("keeps table order rather than reordering by relevance", () => {
    // A list that reshuffles as you type is a list you cannot aim at.
    const found = searchSettings("limit", SETTINGS);
    expect(found.map((d) => d.id)).toEqual([
      "downloadLimitBps",
      "uploadLimitBps",
    ]);
  });
});
