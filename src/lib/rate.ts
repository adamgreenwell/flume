/**
 * Conversions between the KB/s a user types and the bytes/s Flume stores.
 *
 * Kept out of the dialog component so the edge cases — empty, zero, negative,
 * unparseable — can be tested without rendering anything.
 */

/** Bytes in one kilobyte, using the binary convention the rest of the UI uses. */
export const BYTES_PER_KB = 1024;

/**
 * Formats a stored byte rate for a KB/s input field.
 *
 * @param bps - Rate in bytes per second, or `null` for unlimited.
 * @returns The value for the input; empty string means unlimited.
 */
export function toKbInput(bps: number | null): string {
  return bps === null ? "" : String(Math.round(bps / BYTES_PER_KB));
}

/**
 * Parses a KB/s input back into bytes per second.
 *
 * Empty, zero, negative, and unparseable input all mean *unlimited* rather
 * than zero. The backend rejects a zero limit outright — it would stop all
 * transfer — so mapping junk input to `null` keeps a half-typed value from
 * being an error the user has to understand.
 *
 * @param value - Raw input text.
 * @returns Bytes per second, or `null` for unlimited.
 */
export function fromKbInput(value: string): number | null {
  const trimmed = value.trim();
  if (trimmed === "") return null;
  const kb = Number(trimmed);
  if (!Number.isFinite(kb) || kb <= 0) return null;
  return Math.round(kb * BYTES_PER_KB);
}
