/**
 * Presentation helpers for byte counts, transfer rates, and durations.
 *
 * These are pure functions with no Tauri or React dependency so they can be
 * unit tested directly.
 */

const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB", "PB"] as const;

/**
 * Formats a byte count using binary (1024-based) units.
 *
 * Uses one decimal place for values below 10 in their unit, and none above, so
 * columns stay visually stable as numbers change.
 *
 * @param bytes - A non-negative byte count. Negative or non-finite input is
 *   treated as zero.
 * @returns A human-readable string such as `"1.4 GB"`.
 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";

  const exponent = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    BYTE_UNITS.length - 1,
  );
  const value = bytes / 1024 ** exponent;
  const decimals = exponent === 0 || value >= 10 ? 0 : 1;

  return `${value.toFixed(decimals)} ${BYTE_UNITS[exponent]}`;
}

/**
 * Formats a transfer rate.
 *
 * @param bytesPerSecond - Rate in bytes per second.
 * @returns A string such as `"2.3 MB/s"`.
 */
export function formatSpeed(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

/**
 * Formats a duration as a compact uptime string.
 *
 * @param totalSeconds - Duration in seconds. Negative or non-finite input is
 *   treated as zero.
 * @returns A string such as `"45s"`, `"3m 05s"`, or `"2h 07m"`.
 */
export function formatDuration(totalSeconds: number): string {
  if (!Number.isFinite(totalSeconds) || totalSeconds <= 0) return "0s";

  const seconds = Math.floor(totalSeconds);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);

  if (hours > 0) return `${hours}h ${String(minutes).padStart(2, "0")}m`;
  if (minutes > 0)
    return `${minutes}m ${String(seconds % 60).padStart(2, "0")}s`;
  return `${seconds}s`;
}
