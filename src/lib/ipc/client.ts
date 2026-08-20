/**
 * Typed wrappers around Tauri `invoke` calls.
 *
 * Every backend call in Flume goes through this module. Centralising them
 * keeps command-name strings in one place and gives each call a real return
 * type instead of `invoke`'s `unknown`.
 */

import { invoke } from "@tauri-apps/api/core";

import type {
  CoreStatus,
  Settings,
  TorrentDetail,
  TorrentFileState,
  TelemetrySnapshot,
  TorrentPreview,
  TorrentSource,
} from "./types";

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

/**
 * Fetches the full telemetry snapshot: session status plus every torrent.
 *
 * The UI receives this continuously as a pushed event; this call exists so the
 * first paint does not wait up to a full tick for one.
 *
 * @returns The current {@link TelemetrySnapshot}.
 * @throws A {@link CommandError} if the engine has not finished starting;
 *   check with `isCommandError`.
 */
export async function getTelemetry(): Promise<TelemetrySnapshot> {
  return invoke<TelemetrySnapshot>("get_telemetry");
}

/**
 * Resolves a torrent's metadata and file list without downloading anything.
 *
 * For a magnet link this fetches metadata over the DHT and can take several
 * seconds; show a resolving state rather than blocking the UI.
 *
 * @param source - Magnet URI or `.torrent` bytes.
 * @returns The resolved {@link TorrentPreview}.
 * @throws A {@link CommandError} with kind `invalidMagnet` or `metadata`.
 */
export async function previewTorrent(
  source: TorrentSource,
): Promise<TorrentPreview> {
  return invoke<TorrentPreview>("preview_torrent", { source });
}

/**
 * Starts a previewed torrent, downloading only the selected files.
 *
 * @param infoHash - From the preview.
 * @param onlyFiles - File indices to download; `null` downloads everything.
 * @returns The new torrent's session id.
 * @throws A {@link CommandError} with kind `noPendingPreview` if the preview
 *   was already consumed or discarded.
 */
export async function confirmAdd(
  infoHash: string,
  onlyFiles: number[] | null,
): Promise<number> {
  return invoke<number>("confirm_add", { infoHash, onlyFiles });
}

/**
 * Releases a preview the user cancelled, so its metadata is not retained.
 *
 * @param infoHash - From the preview.
 */
export async function discardPreview(infoHash: string): Promise<void> {
  return invoke<void>("discard_preview", { infoHash });
}

/**
 * Pauses a torrent.
 *
 * @param id - Session id from the telemetry stream.
 */
export async function pauseTorrent(id: number): Promise<void> {
  return invoke<void>("pause_torrent", { id });
}

/**
 * Resumes a paused torrent.
 *
 * @param id - Session id from the telemetry stream.
 */
export async function resumeTorrent(id: number): Promise<void> {
  return invoke<void>("resume_torrent", { id });
}

/**
 * Removes a torrent, optionally deleting its files.
 *
 * @param id - Session id from the telemetry stream.
 * @param deleteFiles - Irreversible. Must be an explicit user choice, never a
 *   default.
 */
export async function removeTorrent(
  id: number,
  deleteFiles: boolean,
): Promise<void> {
  return invoke<void>("remove_torrent", { id, deleteFiles });
}

/**
 * Changes which files a torrent downloads.
 *
 * @param id - Session id from the telemetry stream.
 * @param files - File indices to keep downloading.
 */
export async function setOnlyFiles(id: number, files: number[]): Promise<void> {
  return invoke<void>("set_only_files", { id, files });
}

/**
 * Reads the current user settings.
 *
 * @returns The persisted {@link Settings}.
 */
export async function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

/**
 * Validates, persists, and applies new settings.
 *
 * Rate limits take effect immediately. Changing the port, DHT, UPnP, or
 * download directory restarts the torrent session, which takes a moment and
 * briefly makes the engine unavailable.
 *
 * @param settings - The complete settings object.
 * @returns The settings as persisted.
 * @throws A {@link CommandError} with kind `settingsInvalid`,
 *   `settingsSaveFailed`, or `engineFailed`.
 */
export async function updateSettings(settings: Settings): Promise<Settings> {
  return invoke<Settings>("update_settings", { settings });
}

/**
 * Lists a torrent's files with progress and current selection.
 *
 * @param id - Session id from the telemetry stream.
 * @returns One entry per file, in torrent order.
 * @throws A {@link CommandError} with kind `metadata` if the file list has not
 *   resolved yet, which happens briefly for a freshly added magnet.
 */
export async function getTorrentFiles(id: number): Promise<TorrentFileState[]> {
  return invoke<TorrentFileState[]>("get_torrent_files", { id });
}

/**
 * Fetches peers, trackers, and piece completion for one torrent.
 *
 * @param id - Session id from the telemetry stream.
 * @returns The current {@link TorrentDetail}.
 */
export async function getTorrentDetail(id: number): Promise<TorrentDetail> {
  return invoke<TorrentDetail>("get_torrent_detail", { id });
}
