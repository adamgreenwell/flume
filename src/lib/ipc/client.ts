/**
 * Typed wrappers around Tauri `invoke` calls.
 *
 * Every backend call in Flume goes through this module. Centralising them
 * keeps command-name strings in one place and gives each call a real return
 * type instead of `invoke`'s `unknown`.
 */

import { invoke } from "@tauri-apps/api/core";

import type { CoreStatus } from "./types";

/**
 * Fetches a snapshot of torrent engine and DHT status.
 *
 * @returns The current {@link CoreStatus}.
 * @throws A {@link CommandError} if the engine has not finished starting;
 *   check with `isCommandError`.
 */
export async function getCoreStatus(): Promise<CoreStatus> {
  return invoke<CoreStatus>("get_core_status");
}
