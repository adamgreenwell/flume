/**
 * Magnet-link recognition for the UI.
 *
 * This is a cheap shape check, not validation — the authoritative parse
 * happens in Rust before the link reaches the engine. Its only job is deciding
 * whether something is worth offering to the user.
 */

/** Matches a magnet URI carrying a BitTorrent info hash (BEP 9 / BEP 53). */
const MAGNET_PATTERN = /^magnet:\?.*xt=urn:bt(i|m)h:[a-z0-9]+/i;

/**
 * Whether `value` looks like a BitTorrent magnet link.
 *
 * Requires an actual `xt=urn:btih:`/`btmh:` parameter rather than just the
 * `magnet:` scheme, so a magnet for some other content type is not offered as
 * a torrent.
 *
 * @param value - Candidate text, typically from the clipboard.
 * @returns `true` if it is worth offering to add.
 */
export function looksLikeMagnet(value: string): boolean {
  return MAGNET_PATTERN.test(value.trim());
}
