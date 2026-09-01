import { formatDuration } from "@/lib/format";
import type { GuardStatus, Note, Verdict } from "@/lib/ipc/types";

/**
 * Turns the guard's status into the sentence the UI shows.
 *
 * The frontend mirror of `src-tauri/src/engine/note.rs`, and it follows the
 * same rule: never a bare adjective. "Held" is not a status — "nothing is
 * running, because traffic is leaving through en7, an ordinary adapter" is one,
 * and it carries the next move.
 *
 * One function rather than strings scattered through components, because the
 * same sentence has to appear in the banner, the empty state and the settings
 * screen, and three copies drift.
 *
 * Two distinctions it must never blur:
 *
 * 1. A `pinned` interface is never described as a tunnel. Flume could not
 *    confirm it is one; transfer runs because the user said so. Saying
 *    otherwise would be repeating the user's assertion back as Flume's finding.
 * 2. Holding is not pausing. The engine is not running at all, so there is no
 *    peer traffic, no tracker announce, no DHT and no listening port — and no
 *    torrent has been modified. Both halves matter: the first is the guarantee,
 *    the second is why the library is not damaged.
 *
 * @param status - The guard's latest status, or `null` before one arrives.
 * @returns The note to show, or `null` when there is nothing to say.
 */
export function describeGuard(status: GuardStatus | null): Note | null {
  // Off is not a state with a message. The backend still publishes a full
  // verdict every tick, so this suppresses deliberately rather than by
  // accident.
  if (status === null || status.guard === "off") return null;

  const { report, held, resumesInSeconds } = status;
  const verdict = report.verdict;

  if (held) {
    if (resumesInSeconds !== null) return settling(verdict, resumesInSeconds);
    return holding(verdict);
  }

  // Running. Either the verdict permits, or the guard is set to Warn and is
  // reporting without stopping anything.
  if (verdict.verdict === "tunnelled" || verdict.verdict === "pinned") {
    if (verdict.otherFamilyOutside) return leaking(verdict, report);
    return permitting(verdict);
  }
  return warning(verdict);
}

/** The sentence while transfer is held and nothing is running. */
function holding(verdict: Verdict): Note {
  const untouched =
    "Your torrents were not paused and no files were touched; each comes back in the state it was in.";

  switch (verdict.verdict) {
    case "direct":
      return {
        severity: "err",
        title: `Nothing is running: traffic is leaving through ${verdict.interface}`,
        body: `${verdict.interface} is an ordinary adapter, not a tunnel, so Flume has not started its torrent engine at all — nothing is downloading, uploading, announcing to trackers or answering the DHT. ${untouched} Connect your tunnel and transfer resumes on its own once the route has held steady.`,
      };

    case "wrongTunnel":
      return {
        severity: "err",
        title: `Nothing is running: ${verdict.interface} is not the ${verdict.expected} you pinned`,
        body: `${verdict.interface} is a tunnel, but the guard accepts only the interface you pinned, so no torrent engine is running. A VPN that reconnects usually lands on a fresh interface, which on macOS happens on every connect. Pin ${verdict.interface} instead, or clear the pin to accept any tunnel; either takes effect the moment you save.`,
      };

    case "unknown":
      return {
        severity: "err",
        title:
          "Nothing is running: Flume cannot see where traffic would leave by",
        body: `Either there is no usable route out of this machine, or the interface holding one could not be identified — and neither is evidence of a tunnel, so the guard does not treat it as one. ${untouched} Reconnect, and transfer resumes once a route the guard accepts has held steady.`,
      };

    // Unreachable: these permit transfer, so the gate cannot be holding on
    // them. Handled rather than thrown so a future verdict cannot crash the
    // library screen.
    case "tunnelled":
    case "pinned":
      return {
        severity: "neutral",
        title: "Transfer is resuming",
        body: "Traffic is leaving through an interface the guard accepts.",
      };
  }
}

/** The sentence while the settle window runs and transfer is about to resume. */
function settling(verdict: Verdict, seconds: number): Note {
  if (seconds <= 0) {
    return {
      severity: "neutral",
      title: "Transfer is resuming now",
      body: "The route has held steady long enough. Flume is starting the torrent engine; your library fills in as it comes up.",
    };
  }

  const remaining = formatDuration(seconds);
  const where =
    verdict.verdict === "tunnelled"
      ? `Traffic is leaving through ${verdict.interface}, which Flume identifies as a tunnel.`
      : verdict.verdict === "pinned"
        ? `Traffic is leaving through ${verdict.interface}, the interface you pinned.`
        : "Traffic is leaving by an interface the guard accepts.";

  return {
    severity: "neutral",
    title: `A tunnel is back; transfer resumes in ${remaining}`,
    body: `${where} The guard waits for the route to stay put before rebuilding the torrent session, so one flapping reconnect does not re-announce your whole library to every tracker. Nothing needs doing.`,
  };
}

/** The sentence while transfer is running and the guard is satisfied. */
function permitting(verdict: Verdict): Note {
  if (verdict.verdict === "pinned") {
    return {
      severity: "ok",
      title: `Traffic leaves through ${verdict.interface}, the interface you pinned`,
      body: `Flume could not identify ${verdict.interface} as a tunnel from its name or its hardware address, so this is your word for it rather than Flume's finding. Transfer runs because you pinned it. If the route moves to any other interface, transfer stops on the next check.`,
    };
  }

  const interfaceName =
    verdict.verdict === "tunnelled" ? verdict.interface : "this interface";
  return {
    severity: "ok",
    title: `Traffic leaves through ${interfaceName}, a tunnel`,
    body: `Flume asked which interface a packet to the internet would actually use, rather than which interfaces exist — a machine with no VPN connected routinely has several idle tunnel devices and only one carries traffic. It reads this from the routing table and sends nothing to do it.`,
  };
}

/** The sentence when IPv4 is tunnelled and IPv6 is not. */
function leaking(verdict: Verdict, report: GuardStatus["report"]): Note {
  const inside =
    verdict.verdict === "tunnelled" || verdict.verdict === "pinned"
      ? verdict.interface
      : "the tunnel";
  const outside = report.path.v6?.interface ?? "another interface";

  return {
    severity: "warn",
    title: `IPv6 is leaving through ${outside}, outside ${inside}`,
    body: `IPv4 goes through ${inside}; IPv6 has its own route and leaves by ${outside}, so anything preferring IPv6 is not going through the tunnel. Transfer is not held over this — the guard judges on IPv4, where nearly all peer traffic is. Whether IPv6 can be carried on ${inside} is a matter for whatever provides it.`,
  };
}

/** The sentence in Warn mode, where the answer is reported and nothing stops. */
function warning(verdict: Verdict): Note {
  switch (verdict.verdict) {
    case "direct":
      return {
        severity: "warn",
        title: `Traffic is leaving through ${verdict.interface}, which is not a tunnel`,
        body: `Flume classifies ${verdict.interface} as an ordinary adapter — Wi-Fi, Ethernet or cellular — so tracker announces and peer traffic go out over it directly. The guard is set to Warn, so nothing has been stopped. Connect your tunnel, or set the guard to Hold if you would rather transfer waited for one.`,
      };

    case "wrongTunnel":
      return {
        severity: "warn",
        title: `Traffic leaves through ${verdict.interface}, not the ${verdict.expected} you pinned`,
        body: `${verdict.interface} is a tunnel, but the guard accepts only the interface you pinned, so this counts as untunnelled. A VPN that reconnects usually lands on a fresh interface. Pin ${verdict.interface} instead, or clear the pin to accept any tunnel; either takes effect the moment you save.`,
      };

    case "unknown":
      return {
        severity: "warn",
        title: "Flume cannot tell where traffic would leave by",
        body: "Either there is no usable route out of this machine, or the interface holding one could not be identified from its name or its hardware address. Flume does not guess in either direction, so nothing here says you are covered.",
      };

    case "tunnelled":
    case "pinned":
      return permitting(verdict);
  }
}

/**
 * The one-line label for the rail's network block.
 *
 * Words, not colour: the rail's dot carries severity and this carries the
 * fact, so the state is legible without relying on either alone.
 *
 * @param status - The guard's latest status, or `null` before one arrives.
 * @returns A short label, or `null` when the guard is off and says nothing.
 */
export function guardRailLabel(status: GuardStatus | null): string | null {
  if (status === null || status.guard === "off") return null;
  if (status.held) return "Transfer held";

  const verdict = status.report.verdict;
  switch (verdict.verdict) {
    case "tunnelled":
      return `Leaves by ${verdict.interface} · tunnel`;
    case "pinned":
      return `Leaves by ${verdict.interface} · pinned`;
    case "direct":
      return `Leaves by ${verdict.interface} · not a tunnel`;
    case "wrongTunnel":
      return `Leaves by ${verdict.interface} · not ${verdict.expected}`;
    case "unknown":
      return "Route unknown";
  }
}
