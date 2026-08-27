"use client";

import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";

import { formatBytes, formatSpeed } from "@/lib/format";
import {
  detectClients,
  getSettings,
  importClient,
  updateSettings,
} from "@/lib/ipc/client";
import {
  isCommandError,
  type DetectedClient,
  type ImportOutcome,
  type Settings,
} from "@/lib/ipc/types";

import { Button } from "./Button";
import { Chip } from "./Chip";
import { Icon } from "./Icon";
import { SettingControl } from "./SettingControl";

/** Rate limits offered on the way in, in bytes per second. */
const LIMIT_CHOICES: ReadonlyArray<{ value: number | null; label: string }> = [
  { value: null, label: "No limit" },
  { value: 20_000_000, label: "20 MB/s" },
  { value: 8_000_000, label: "8 MB/s" },
  { value: 2_000_000, label: "2 MB/s" },
];

/** One numbered question. */
function Question({
  index,
  title,
  aside,
  children,
}: {
  index: number;
  title: string;
  aside?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="border-line flex flex-col gap-3 border-t py-5">
      <div className="flex items-baseline gap-2.5">
        <span className="flume-num text-fg-3 text-[11px]">{index}</span>
        <b className="text-[13.5px] font-semibold tracking-[-0.008em]">
          {title}
        </b>
        {aside ? <span className="text-fg-3 text-[11px]">{aside}</span> : null}
      </div>
      {children}
    </section>
  );
}

/** Props for {@link FirstRun}. */
export interface FirstRunProps {
  /** Called once the user has finished, with the settings they chose. */
  onDone: () => void;
}

/**
 * The screen a new user meets before anything else exists.
 *
 * Three questions, not thirty settings. Everything here is also in Settings —
 * this exists to make the handful of choices that are annoying to discover
 * later, not to be a second settings screen.
 *
 * ## The import card
 *
 * The most useful thing Flume can offer someone who already has a torrent
 * client is to take their library over without downloading any of it again.
 * That is real rather than aspirational: each torrent is added over its
 * existing files and the engine verifies them in place, so anything the other
 * client had finished arrives complete.
 *
 * It does not offer to bring categories or seeding rules across, which the
 * design asks for, because Flume has neither and importing them would mean
 * dropping them on the floor while claiming otherwise.
 *
 * @param props - See {@link FirstRunProps}.
 * @returns The rendered screen.
 */
export function FirstRun({ onDone }: FirstRunProps) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [clients, setClients] = useState<DetectedClient[] | null>(null);
  const [importing, setImporting] = useState<string | null>(null);
  const [imported, setImported] = useState<ImportOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void getSettings()
      .then(setSettings)
      .catch(() => setSettings(null));
    // Scanning for other clients touches the filesystem, so it runs alongside
    // rather than blocking the screen: the questions are answerable whether or
    // not anything is found.
    void detectClients()
      .then(setClients)
      .catch(() => setClients([]));
  }, []);

  const patch = useCallback(
    async (change: Partial<Settings>) => {
      if (!settings) return;
      const next = { ...settings, ...change };
      setSettings(next);
      try {
        setSettings(await updateSettings(next));
      } catch (caught: unknown) {
        setSettings(settings);
        setError(
          isCommandError(caught)
            ? caught.message
            : "Could not save that choice.",
        );
      }
    },
    [settings],
  );

  const runImport = useCallback(async (client: DetectedClient) => {
    setImporting(client.name);
    setError(null);
    try {
      setImported(await importClient(client.torrentsDir, client.downloadDir));
    } catch (caught: unknown) {
      setError(
        isCommandError(caught)
          ? caught.message
          : `Could not import from ${client.name}.`,
      );
    } finally {
      setImporting(null);
    }
  }, []);

  const total = (clients ?? []).reduce((sum, c) => sum + c.torrentCount, 0);

  return (
    <div className="bg-bg-0 flex h-full items-center justify-center overflow-y-auto p-8">
      <div className="w-full max-w-[720px] py-8">
        <div className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase">
          First run
        </div>
        <h1 className="mt-1.5 text-[30px] leading-[1.1] font-semibold tracking-[-0.035em]">
          Three answers and you are done.
        </h1>
        <p className="text-fg-1 mt-2.5 max-w-[560px] text-[12.5px] leading-[1.55]">
          Everything here is in Settings too. These are only the choices that
          are irritating to find later.
        </p>

        {clients === null ? (
          <div className="border-line bg-bg-1 text-fg-2 mt-7 rounded-lg border px-5 py-4 text-[12.5px]">
            Looking for other torrent clients…
          </div>
        ) : null}

        {clients !== null && clients.length > 0 && imported === null ? (
          <div className="border-acc-dim bg-acc-deep/30 mt-7 flex items-start gap-3.5 rounded-lg border px-5 py-4">
            <span className="text-acc mt-0.5 shrink-0">
              <Icon name="arrow-down" size={22} />
            </span>
            <div className="grow">
              <div className="text-[13.5px] font-semibold">
                Found{" "}
                {clients.length === 1 ? "another client" : "other clients"} on
                this machine
              </div>
              <p className="text-fg-1 mt-1 text-[12.5px] leading-[1.5]">
                {clients
                  .map((c) => `${c.torrentCount} in ${c.name}`)
                  .join(" and ")}
                , with their save paths. Nothing is downloaded again — the files
                you already have are verified where they sit.
              </p>
              {clients.some((c) => c.downloadDir === null) ? (
                <p className="text-fg-3 mt-1.5 text-[11px]">
                  Some of their settings could not be read, so those torrents
                  will save to Flume&rsquo;s own folder.
                </p>
              ) : null}
            </div>
            <div className="flex shrink-0 flex-col gap-2">
              {clients.map((client) => (
                <Button
                  key={client.name}
                  variant="primary"
                  disabled={importing !== null}
                  onClick={() => void runImport(client)}
                >
                  {importing === client.name
                    ? "Importing…"
                    : `Import ${client.torrentCount} from ${client.name}`}
                </Button>
              ))}
            </div>
          </div>
        ) : null}

        {imported ? (
          <div className="border-ok/40 bg-ok/10 mt-7 flex items-start gap-3.5 rounded-lg border px-5 py-4">
            <span className="text-ok mt-0.5 shrink-0">
              <Icon name="check-circle" size={20} />
            </span>
            <p className="text-fg-1 text-[12.5px] leading-[1.5]">
              <b className="text-fg-0">
                {imported.added} {imported.added === 1 ? "torrent" : "torrents"}{" "}
                taken over.
              </b>{" "}
              Flume is hashing what is already on disk; anything complete will
              start seeding without downloading again.
              {imported.skipped > 0
                ? ` ${imported.skipped} ${imported.skipped === 1 ? "was" : "were"} already here.`
                : ""}
              {imported.failed > 0
                ? ` ${imported.failed} could not be read and ${imported.failed === 1 ? "was" : "were"} left alone.`
                : ""}
            </p>
          </div>
        ) : null}

        {clients !== null && clients.length === 0 ? (
          <div className="border-line bg-bg-1 text-fg-2 mt-7 rounded-lg border px-5 py-4 text-[12.5px]">
            No other torrent clients found, so there is nothing to bring across.
          </div>
        ) : null}

        <div className="mt-7">
          <Question
            index={1}
            title="Where should downloads go?"
            aside={settings ? undefined : "reading your settings…"}
          >
            <div className="flex items-center gap-2">
              <span
                className="border-line bg-bg-2 text-fg-1 flex h-[var(--flume-h-control)] min-w-0 grow items-center gap-2 rounded-md border px-2.5 text-[12.5px]"
                title={settings?.downloadDir}
              >
                <Icon name="folder" size={15} />
                <span className="truncate">{settings?.downloadDir ?? "…"}</span>
              </span>
              <Chip
                onClick={async () => {
                  const picked = await open({
                    directory: true,
                    multiple: false,
                  });
                  if (typeof picked === "string") {
                    void patch({ downloadDir: picked });
                  }
                }}
              >
                Choose another
              </Chip>
            </div>
          </Question>

          <Question
            index={2}
            title="How much of your connection may Flume use?"
            aside="you can change this at any time"
          >
            <div className="flex flex-wrap items-center gap-1.5">
              {LIMIT_CHOICES.map((choice) => (
                <Chip
                  key={choice.label}
                  selected={settings?.downloadLimitBps === choice.value}
                  onClick={() => void patch({ downloadLimitBps: choice.value })}
                >
                  {choice.label}
                </Chip>
              ))}
            </div>
            <p className="text-fg-1 text-xs leading-[1.5]">
              {settings?.downloadLimitBps == null
                ? "No cap. Downloads take whatever the connection will give them, which can make everything else on your network feel slow."
                : `Held to ${formatSpeed(settings.downloadLimitBps)} — a ${formatBytes(4_700_000_000)} ISO would take a little over ${Math.round(4_700_000_000 / settings.downloadLimitBps / 60)} minutes.`}
            </p>
          </Question>

          <Question
            index={3}
            title="Light or dark?"
            aside="follows the system unless you say otherwise"
          >
            {settings ? (
              <SettingControl
                control={{
                  kind: "segment",
                  options: [
                    { value: "system", label: "System" },
                    { value: "light", label: "Light" },
                    { value: "dark", label: "Dark" },
                  ],
                }}
                value={settings.theme}
                label="Colour scheme"
                onChange={(next) =>
                  void patch({ theme: next as Settings["theme"] })
                }
              />
            ) : null}
          </Question>
        </div>

        {error ? (
          <p
            className="border-err/30 bg-err/10 text-err mt-4 rounded-md border px-3 py-2 text-[12.5px]"
            role="alert"
          >
            {error}
          </p>
        ) : null}

        <div className="border-line mt-2 flex items-center gap-3 border-t pt-5">
          <span className="text-fg-3 text-[11px]">
            {total > 0 && imported === null
              ? `${total} torrents waiting in other clients.`
              : "Nothing here is permanent."}
          </span>
          <span className="grow" />
          <Button
            variant="primary"
            size="dialog"
            disabled={settings === null}
            onClick={onDone}
          >
            Start using Flume
          </Button>
        </div>
      </div>
    </div>
  );
}
