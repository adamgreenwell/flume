"use client";

import { useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

import { getDiagnostics } from "@/lib/ipc/client";
import { isCommandError } from "@/lib/ipc/types";

import { Chip } from "./Chip";
import { Icon } from "./Icon";

/**
 * Builds a diagnostics bundle and shows it before offering to copy it.
 *
 * The bundle is rendered on screen first, deliberately. A privacy feature
 * whose payload you cannot read before it leaves is asking for trust it has
 * not earned — and the user is the only one who can spot a torrent name that
 * survived redaction, because they are the only one who knows what their
 * torrents are called.
 *
 * This is an action rather than a setting, so it has no row in
 * `SETTING_DEFS` — see the comment where it is rendered.
 *
 * @returns The rendered card.
 */
export function DiagnosticsCard() {
  const [bundle, setBundle] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);

  async function build() {
    setBusy(true);
    setError(null);
    setCopied(false);
    try {
      setBundle(await getDiagnostics());
    } catch (thrown) {
      setError(
        isCommandError(thrown)
          ? thrown.message
          : "The diagnostics report could not be built.",
      );
    } finally {
      setBusy(false);
    }
  }

  async function copy() {
    if (bundle === null) return;
    try {
      await writeText(bundle);
      setCopied(true);
    } catch {
      setError("Could not write to the clipboard.");
    }
  }

  return (
    <div className="border-line flex items-start gap-6 border-b py-4">
      <div className="min-w-0 grow">
        <div className="flex items-center gap-2.5">
          <b className="text-[13.5px] font-semibold tracking-[-0.008em]">
            Diagnostics report
          </b>
          <span className="flume-num border-line bg-bg-2 text-fg-3 rounded-[3px] border px-[5px] py-px text-[10px] whitespace-nowrap">
            privacy.diagnostics
          </span>
        </div>
        <p className="text-fg-1 mt-1 max-w-[620px] text-xs leading-[1.5]">
          Builds a report about this install to paste into a bug report —
          versions, which port bound, how many DHT nodes were found, and the end
          of the log. Paths, addresses, tracker URLs, info hashes and the names
          of torrents in your library are removed first. Nothing is sent
          anywhere; you copy it yourself.
        </p>

        {bundle !== null ? (
          <>
            <p className="text-fg-2 mt-2.5 max-w-[620px] text-[11.5px] leading-[1.5]">
              Read it before you paste it. Redaction cannot recognise the name
              of a torrent you have already removed, so a line naming one can
              survive.
            </p>
            <pre className="border-line bg-bg-2 text-fg-1 selectable mt-2 max-h-[260px] max-w-[620px] overflow-auto rounded-md border p-3 font-mono text-[11px] leading-[1.55] whitespace-pre">
              {bundle}
            </pre>
          </>
        ) : null}

        {error !== null ? (
          <p className="text-err mt-2 text-[11.5px]" role="alert">
            {error}
          </p>
        ) : null}
      </div>

      <div className="flex shrink-0 items-center gap-2 pt-0.5">
        <Chip onClick={() => void build()}>
          {busy ? "Building…" : bundle === null ? "Build report" : "Rebuild"}
        </Chip>
        {bundle !== null ? (
          <Chip onClick={() => void copy()}>
            {copied ? (
              <span className="flex items-center gap-1.5">
                <Icon name="check" size={13} />
                Copied
              </span>
            ) : (
              "Copy"
            )}
          </Chip>
        ) : null}
      </div>
    </div>
  );
}
