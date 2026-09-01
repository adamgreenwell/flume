import type { IconName } from "@/components/Icon";
import { formatBytes, formatDuration, formatSpeed } from "@/lib/format";
import type { Settings } from "@/lib/ipc/types";

/**
 * The settings screen is generated from this table, not hand-written.
 *
 * Hand-written rows kill search within a month: the search field has to cover
 * every label, key and description, and a screen assembled by hand grows rows
 * that the search does not know about. Generating both from one table makes
 * that impossible by construction.
 *
 * Every definition carries a `consequence` — a sentence computed from the
 * *current* value, not static help text. That is the feature, and a setting
 * without one does not ship. "Maximum download rate" tells the user nothing
 * they could not read off the label; "a 4.7 GB ISO takes about 16 minutes at
 * this speed" tells them what they actually want to know.
 */

/** Which group a setting belongs to. */
export type SectionId = "speed" | "files" | "network" | "ui" | "privacy";

/** One group in the settings nav. */
export interface SectionDef {
  id: SectionId;
  name: string;
  description: string;
  icon: IconName;
}

/** The sections, in nav order. */
export const SECTIONS: readonly SectionDef[] = [
  {
    id: "speed",
    name: "Speed",
    description: "How much of your connection Flume may use.",
    icon: "clock",
  },
  {
    id: "files",
    name: "Where files go",
    description: "Folders and disk destinations.",
    icon: "folder",
  },
  {
    id: "network",
    name: "Network",
    description: "How Flume finds peers and how they reach you.",
    icon: "arrow-up",
  },
  {
    id: "ui",
    name: "Appearance",
    description: "How the app itself looks.",
    icon: "settings",
  },
  {
    id: "privacy",
    name: "Privacy",
    description: "What leaves this machine, and what does not.",
    icon: "shield",
  },
];

/** The control a setting is edited with. */
export type Control =
  | { kind: "toggle" }
  | { kind: "rate" }
  | { kind: "port" }
  | { kind: "path" }
  | { kind: "text"; placeholder: string }
  | {
      kind: "segment";
      options: ReadonlyArray<{ value: string; label: string }>;
    }
  /**
   * A picker over the machine's real network interfaces.
   *
   * The options are not in the definition because they are not knowable at
   * build time — they are the interfaces this machine has right now, which on
   * macOS changes every time a VPN reconnects. {@link SettingsDialog} loads
   * them and hands them down.
   */
  | { kind: "interface" };

/** One setting, bound to the field it edits. */
export interface SettingDef<K extends keyof Settings = keyof Settings> {
  /** The `Settings` field this edits. Also its identity. */
  id: K;
  /** Which group it lives in. */
  section: SectionId;
  /** Plain language, sentence case, no jargon. */
  label: string;
  /** The config key, shown in a small mono chip so power users can find it. */
  key: string;
  /** How it is edited. */
  control: Control;
  /**
   * What happens, given the value it currently holds.
   *
   * Required. Computed from the value, never static.
   */
  consequence: (value: Settings[K]) => string;
  /** Extra words the search should match — synonyms, old names, jargon. */
  keywords?: readonly string[];
  /** Whether changing this rebuilds the librqbit session. */
  restartsSession?: boolean;
}

/** Any setting definition, as a discriminated union over the settings fields. */
export type AnySettingDef = {
  [K in keyof Settings]: SettingDef<K>;
}[keyof Settings];

/** A representative download, for making a rate limit concrete. */
const REFERENCE_ISO_BYTES = 4_700_000_000;

/**
 * Describes a rate limit by what it does to a real download.
 *
 * "5.0 MB/s" is a number; "a 4.7 GB ISO takes about 16 min" is a consequence.
 * The second is the one that tells someone whether the limit they are about to
 * set is the one they want.
 */
function rateConsequence(direction: "down" | "up") {
  return (bps: number | null): string => {
    if (bps === null || bps <= 0) {
      return direction === "down"
        ? "No cap. Downloads take whatever the connection will give them, which can make everything else on your network feel slow."
        : "No cap. Uploading saturates your connection's upstream, which is what usually makes browsing feel slow while seeding.";
    }

    if (direction === "up") {
      return `Seeding is held to ${formatSpeed(bps)}, leaving the rest of your upstream for everything else.`;
    }

    return `Held to ${formatSpeed(bps)} — a ${formatBytes(
      REFERENCE_ISO_BYTES,
    )} ISO would take about ${formatDuration(REFERENCE_ISO_BYTES / bps)}.`;
  };
}

/**
 * Every setting Flume has.
 *
 * Ordered within each section by how likely someone is to want it, not
 * alphabetically — the first row of a section should be the one that brought
 * the user there.
 */
export const SETTING_DEFS: readonly AnySettingDef[] = [
  {
    id: "downloadLimitBps",
    section: "speed",
    label: "Limit download speed",
    key: "speed.download",
    control: { kind: "rate" },
    keywords: ["throttle", "bandwidth", "cap", "rate limit"],
    consequence: rateConsequence("down"),
  },
  {
    id: "uploadLimitBps",
    section: "speed",
    label: "Limit upload speed",
    key: "speed.upload",
    control: { kind: "rate" },
    keywords: ["throttle", "bandwidth", "cap", "seeding", "rate limit"],
    consequence: rateConsequence("up"),
  },
  {
    id: "downloadDir",
    section: "files",
    label: "Save downloads to",
    key: "files.downloadDir",
    control: { kind: "path" },
    keywords: ["folder", "directory", "destination", "location"],
    restartsSession: true,
    consequence: (dir) =>
      dir === ""
        ? "Not set yet. Flume will use your Downloads folder."
        : `New torrents are written to ${dir}. Torrents already added keep the folder they started in.`,
  },
  {
    id: "enableDht",
    section: "network",
    label: "Find peers through the DHT",
    key: "net.dht",
    control: { kind: "toggle" },
    keywords: [
      "magnet",
      "distributed hash table",
      "trackerless",
      "peer discovery",
    ],
    restartsSession: true,
    consequence: (on) =>
      on
        ? "Magnet links work, and torrents can find peers even when their trackers are down."
        : "Magnet links will not work at all — they have no file list without the DHT. Only .torrent files with a reachable tracker can be added.",
  },
  {
    id: "listenPort",
    section: "network",
    label: "Listen for peers on port",
    key: "net.listenPort",
    control: { kind: "port" },
    keywords: ["incoming", "firewall", "forward", "tcp"],
    restartsSession: true,
    consequence: (port) =>
      `Peers reach you on port ${port}. If your router does not forward it you can still download, but only peers you contact first will connect — which is slower, and contributes less back to the swarm.`,
  },
  {
    id: "enableUpnp",
    section: "network",
    label: "Ask the router to forward that port",
    key: "net.upnp",
    control: { kind: "toggle" },
    keywords: ["port forwarding", "nat", "pmp", "router"],
    restartsSession: true,
    consequence: (on) =>
      on
        ? "Flume asks your router to open the port automatically. Most home routers agree; some have UPnP switched off, in which case nothing happens and nothing breaks."
        : "You will need to forward the port yourself, or accept that fewer peers can start a connection with you.",
  },
  {
    id: "proxyUrl",
    section: "network",
    label: "Route peer connections through a proxy",
    key: "net.proxy",
    control: {
      kind: "text",
      placeholder: "socks5://host:port",
    },
    keywords: ["socks5", "socks", "vpn", "tunnel", "privacy"],
    restartsSession: true,
    consequence: (url) =>
      url === null || url === ""
        ? "Peer connections go out directly from this machine, so peers see your real address."
        : `Outgoing peer connections go through ${url}. The DHT and tracker announces do not — a proxy here is not the same as a VPN.`,
  },
  {
    id: "egressGuard",
    section: "network",
    label: "Only transfer while traffic leaves through a tunnel",
    key: "net.egressGuard",
    control: {
      kind: "segment",
      options: [
        { value: "off", label: "Off" },
        { value: "warn", label: "Warn" },
        { value: "hold", label: "Hold" },
      ],
    },
    keywords: [
      "vpn",
      "wireguard",
      "openvpn",
      "kill switch",
      "killswitch",
      "tunnel",
      "leak",
      "privacy",
      "torguard",
    ],
    consequence: (mode) => {
      if (mode === "off") {
        return "Flume does not look at which interface your traffic leaves by. Nothing is checked and nothing is blocked.";
      }
      const checked =
        "Flume checks which network interface traffic would actually leave by — the route, not just whether a tunnel exists on the machine. It reads this locally and sends nothing to do it.";
      return mode === "hold"
        ? `${checked} While traffic is leaving by anything it does not accept, every torrent is held, and they resume on their own when a tunnel is back. An interface you pin below counts as accepted even where Flume cannot confirm it is a tunnel. This holds torrents, not Flume itself: peer discovery keeps running.`
        : `${checked} You are told when traffic is leaving outside a tunnel, and nothing is stopped.`;
    },
  },
  {
    id: "egressInterface",
    section: "network",
    label: "Accept only this interface",
    key: "net.egressInterface",
    control: { kind: "interface" },
    keywords: ["interface", "utun", "wg0", "adapter", "pin", "device"],
    consequence: (name) =>
      name === null || name === ""
        ? "Any tunnel interface is accepted. This survives a VPN reconnecting onto a different interface, which macOS does routinely."
        : `Only ${name} is accepted; any other interface counts as untunnelled, even another tunnel. Stricter, and on macOS it will trip: a new utun appears on every connect and the old ones stay, so the number you pin today is not the one carrying traffic tomorrow. Windows and Linux name the adapter after the VPN config and stay put.`,
  },
  {
    id: "theme",
    section: "ui",
    label: "Colour scheme",
    key: "ui.theme",
    control: {
      kind: "segment",
      options: [
        { value: "system", label: "System" },
        { value: "light", label: "Light" },
        { value: "dark", label: "Dark" },
      ],
    },
    keywords: ["dark mode", "light mode", "appearance"],
    consequence: (theme) =>
      theme === "system"
        ? "Follows the operating system, and changes with it while Flume is open."
        : `Always ${theme}, whatever the system is set to.`,
  },
  {
    id: "usageReporting",
    section: "privacy",
    label: "Send anonymous usage counts",
    key: "privacy.usage",
    control: { kind: "toggle" },
    keywords: [
      "telemetry",
      "analytics",
      "tracking",
      "data collection",
      "opt in",
      "diagnostics",
    ],
    // Spells out the whole payload rather than gesturing at it. Anyone who
    // opens this row is asking exactly one question — what gets sent — and a
    // vague answer is the reason nobody trusts a toggle like this.
    consequence: (on) =>
      on === true
        ? "Sending: a random ID for this install, Flume's version, your OS and CPU type, and counts of things like launches, torrents added and errors — bucketed, and timed to the hour. Never torrent names, file names, info hashes, tracker addresses, IP addresses or folder paths. Turning this off deletes the ID and anything not yet sent."
        : "Nothing is sent. Flume makes no network requests except to trackers, peers and the DHT.",
    // Not `restartsSession`: consent applies immediately, and a torrent client
    // that made you restart to stop being counted would deserve the reaction.
  },
  {
    id: "density",
    section: "ui",
    label: "Row height",
    key: "ui.density",
    control: {
      kind: "segment",
      options: [
        { value: "comfortable", label: "Comfortable" },
        { value: "compact", label: "Compact" },
      ],
    },
    keywords: ["compact", "dense", "spacing", "list"],
    consequence: (density) =>
      density === "comfortable"
        ? "58px rows, each with a second line saying what that torrent is doing."
        : "40px rows. About 40% more fit on screen, and the explanation line under each name is hidden.",
  },
];

/**
 * Settings matching a search query.
 *
 * Matches the label, the config key, the section name, the keywords, and the
 * consequence text as it currently reads. Searching the live consequence is
 * deliberate: someone who remembers "magnet links will not work" can find the
 * setting that says it, which is often the only phrasing they saw.
 *
 * An empty query returns everything, so clearing the field restores the list.
 *
 * @param query - The raw query, untrimmed.
 * @param settings - Current values, for matching consequence text.
 * @returns The matching definitions, in table order.
 */
export function searchSettings(
  query: string,
  settings: Settings,
): AnySettingDef[] {
  const needle = query.trim().toLowerCase();
  if (needle === "") return [...SETTING_DEFS];

  return SETTING_DEFS.filter((def) => {
    const section = SECTIONS.find((s) => s.id === def.section);
    const haystack = [
      def.label,
      def.key,
      section?.name ?? "",
      ...(def.keywords ?? []),
      // Cast: the union has already bound `consequence` to this entry's own
      // field type, but TypeScript cannot see that through the mapped union.
      (def.consequence as (v: unknown) => string)(settings[def.id]),
    ];

    return haystack.some((text) => text.toLowerCase().includes(needle));
  });
}
