/**
 * Wording for magnets that were added before their file list arrived (#148).
 *
 * Status is never colour alone: each row gets a word and a sentence, and the
 * sentence always says that nothing downloads yet, because "never
 * downloading" is the promise the whole feature rests on.
 */

import { formatDuration } from "./format";
import type { PendingMagnetStatus } from "./ipc/types";

/** The word and the sentence for one pending magnet. */
export interface PendingMagnetWording {
  /** A short status word for the row's status column. */
  word: string;
  /** The sentence for the row's detail line. */
  sentence: string;
}

/**
 * Names a pending magnet.
 *
 * The real name once the file list has arrived, the link's own display name
 * before that, and a shortened info hash when the link carried neither.
 *
 * @param magnet - The pending magnet.
 * @returns What to show as its title.
 */
export function pendingMagnetName(magnet: PendingMagnetStatus): string {
  return magnet.name ?? `Magnet ${magnet.infoHash.slice(0, 8)}…`;
}

/**
 * Describes where a pending magnet stands.
 *
 * @param magnet - The pending magnet.
 * @returns Its status word and detail sentence.
 */
export function describePendingMagnet(
  magnet: PendingMagnetStatus,
): PendingMagnetWording {
  if (magnet.state === "ready") {
    return {
      word: "Ready",
      sentence:
        "The file list arrived. Choose files to start — nothing downloads until you do.",
    };
  }
  if (magnet.searchingNow) {
    return {
      word: "Finding files",
      sentence: "Asking peers for the file list now. Nothing is downloading.",
    };
  }
  if (magnet.failedAttempts > 0 && magnet.nextAttemptInSeconds !== null) {
    const tries =
      magnet.failedAttempts === 1
        ? "No peer has answered yet"
        : `No peer has answered in ${magnet.failedAttempts} tries`;
    return {
      word: "Finding files",
      sentence: `${tries}. Trying again in ${formatDuration(magnet.nextAttemptInSeconds)}. Nothing is downloading.`,
    };
  }
  return {
    word: "Finding files",
    sentence: "Waiting to ask peers for the file list. Nothing is downloading.",
  };
}
