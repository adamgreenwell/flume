/**
 * TypeScript mirrors of the Rust types in `src-tauri/src/engine/status.rs`.
 *
 * These definitions are hand-maintained. If you change a `serde` struct on the
 * Rust side, change it here in the same commit — the compiler cannot check
 * across the IPC boundary for us.
 *
 * The Rust structs use `#[serde(rename_all = "camelCase")]`, so field names
 * here are camelCase.
 */

/** Health of the DHT subsystem. */
export interface DhtStatus {
  /** Whether the DHT was enabled in configuration at all. */
  enabled: boolean;
  /** Number of IPv4 nodes in the routing table. Zero while bootstrapping. */
  nodesV4: number;
  /** Number of IPv6 nodes in the routing table. */
  nodesV6: number;
  /** DHT queries currently awaiting a response. */
  outstandingRequests: number;
}

/**
 * Coarse, user-facing readiness of the engine.
 *
 * Mirrors the Rust `EngineHealth` enum, which serialises as a camelCase string.
 */
export type EngineHealth = "starting" | "connecting" | "ready" | "degraded";

/** A point-in-time snapshot of engine state. */
export interface CoreStatus {
  /** The librqbit client string, e.g. `"Flume 0.1.0"`. */
  clientVersion: string;
  /** Port bound for incoming peer connections, or `null` if not listening. */
  listenPort: number | null;
  /** Port announced to trackers and peers, or `null`. */
  announcePort: number | null;
  /** DHT subsystem health. */
  dht: DhtStatus;
  /** Absolute path downloads are written to. */
  downloadDir: string;
  /** Seconds since the session started. */
  uptimeSeconds: number;
  /** Current aggregate download rate in bytes per second. */
  downloadBps: number;
  /** Current aggregate upload rate in bytes per second. */
  uploadBps: number;
  /** Peers currently connected across all torrents. */
  livePeers: number;
  /** Derived readiness indicator. */
  health: EngineHealth;
}

/** An error returned by a Tauri command. Mirrors Rust `CommandError`. */
export interface CommandError {
  /** Stable identifier for the error class, e.g. `"engineNotReady"`. */
  kind: string;
  /** Human-readable description, safe to show in the UI. */
  message: string;
}

/**
 * Narrows an unknown thrown value to a {@link CommandError}.
 *
 * Tauri rejects `invoke` with whatever the Rust side serialised, so the value
 * arrives as `unknown` and must be checked before use.
 *
 * @param value - The value caught from a rejected `invoke` call.
 * @returns `true` if `value` has the shape of a `CommandError`.
 */
export function isCommandError(value: unknown): value is CommandError {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    "message" in value &&
    typeof (value as CommandError).kind === "string" &&
    typeof (value as CommandError).message === "string"
  );
}
