/**
 * Presentation helpers for byte counts, transfer rates, and durations.
 *
 * These are pure functions with no Tauri or React dependency so they can be
 * unit tested directly.
 */

const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB", "PB"] as const;

/** Decimal, not binary. See {@link formatBytes}. */
const STEP = 1000;

/**
 * Splits a byte count into a value and its unit, decimally.
 *
 * @param bytes - A non-negative byte count.
 * @returns The scaled value and the unit to label it with.
 */
function scale(bytes: number): { value: number; unit: string } {
  const exponent = Math.min(
    Math.floor(Math.log(bytes) / Math.log(STEP)),
    BYTE_UNITS.length - 1,
  );
  return { value: bytes / STEP ** exponent, unit: BYTE_UNITS[exponent] };
}

/**
 * Formats a byte count using decimal (1000-based) units.
 *
 * Decimal because that is what disks and ISPs quote: a drive sold as 2 TB
 * holds 2×10¹² bytes, and a client that renders it as 1.82 TB is telling the
 * user their disk is smaller than the label. Piece length is the only binary
 * figure in Flume, written MiB, because that is what the wire format uses.
 *
 * Three significant figures — `46.1 GB`, `1.19 GB`, `231 GB`. Fewer loses real
 * distinctions between torrents; more is noise at the sizes involved.
 *
 * @param bytes - A non-negative byte count. Negative or non-finite input is
 *   treated as zero.
 * @returns A human-readable string such as `"46.1 GB"`.
 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";

  const { value, unit } = scale(bytes);
  if (unit === "B") return `${Math.round(value)} B`;

  const decimals = value < 10 ? 2 : value < 100 ? 1 : 0;
  return `${value.toFixed(decimals)} ${unit}`;
}

/**
 * Rates at or above this are written in MB/s rather than KB/s.
 *
 * 0.1 MB/s. The rate columns are read down, not across, and a column that
 * switches unit per row makes the reader parse every cell instead of comparing
 * shapes. Holding MB/s down to 0.1 keeps the common range in one unit; below
 * that, "0.0 MB/s" would read as nothing at all, so the unit does change.
 */
const MB_FLOOR = 100_000;

/**
 * Formats a transfer rate.
 *
 * One decimal place rather than {@link formatBytes}'s three significant
 * figures. A rate is redrawn every second, and two decimals on a number that
 * changes every tick is motion the eye tracks and the mind cannot use.
 *
 * @param bytesPerSecond - Rate in bytes per second. Negative or non-finite
 *   input is treated as zero.
 * @returns A string such as `"6.6 MB/s"`.
 */
export function formatSpeed(bytesPerSecond: number): string {
  if (!Number.isFinite(bytesPerSecond) || bytesPerSecond <= 0) return "0 B/s";

  if (bytesPerSecond >= MB_FLOOR) {
    const value = bytesPerSecond / STEP ** 2;
    return `${value.toFixed(value < 100 ? 1 : 0)} MB/s`;
  }

  const { value, unit } = scale(bytesPerSecond);
  if (unit === "B") return `${Math.round(value)} B/s`;
  return `${value.toFixed(value < 100 ? 1 : 0)} ${unit}/s`;
}

/**
 * Formats a duration the way the design writes them.
 *
 * Units are spaced — `2 min 30 s`, not `2m 30s`. At the row's 11px meta size
 * the unspaced form reads as one token and the eye has to stop to parse it;
 * these strings sit inside sentences ("2 min 30 s left") where that matters.
 *
 * **Must stay in step with `format_duration` in
 * `src-tauri/src/engine/torrent.rs`.** That one exists because the engine
 * decides what a torrent's detail line says; two formatters that drifted would
 * put "2 min 30 s left" next to "2m 30s" in the same row.
 *
 * @param totalSeconds - Duration in seconds. Negative or non-finite input is
 *   treated as zero.
 * @returns A string such as `"45 s"`, `"3 min 05 s"`, or `"2 h 07 min"`.
 */
export function formatDuration(totalSeconds: number): string {
  if (!Number.isFinite(totalSeconds) || totalSeconds <= 0) return "0 s";

  const seconds = Math.floor(totalSeconds);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);

  if (hours > 0) return `${hours} h ${String(minutes).padStart(2, "0")} min`;
  if (minutes > 0)
    return `${minutes} min ${String(seconds % 60).padStart(2, "0")} s`;
  return `${seconds} s`;
}
