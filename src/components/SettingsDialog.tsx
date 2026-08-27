"use client";

import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";

import { getSettings, updateSettings } from "@/lib/ipc/client";
import { fromKbInput, toKbInput } from "@/lib/rate";
import { applyTheme } from "@/lib/theme";
import { isCommandError, type Settings, type Theme } from "@/lib/ipc/types";

import { Button } from "./Button";
import { Skeleton } from "./Skeleton";

/** Props for {@link SettingsDialog}. */
export interface SettingsDialogProps {
  /** Called when the dialog should close. */
  onClose: () => void;
}

/**
 * Settings editor.
 *
 * Changes are applied on save, not per-field: several settings restart the
 * torrent session, and doing that on every keystroke would be hostile.
 *
 * @param props - See {@link SettingsDialogProps}.
 * @returns The rendered dialog.
 */
export function SettingsDialog({ onClose }: SettingsDialogProps) {
  const [draft, setDraft] = useState<Settings | null>(null);
  const [original, setOriginal] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    let active = true;
    void getSettings()
      .then((loaded) => {
        if (!active) return;
        setDraft(loaded);
        setOriginal(loaded);
      })
      .catch((caught: unknown) => {
        if (!active) return;
        setError(
          isCommandError(caught) ? caught.message : "Could not load settings.",
        );
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const patch = (changes: Partial<Settings>) => {
    setDraft((current) => (current ? { ...current, ...changes } : current));
  };

  const chooseFolder = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") patch({ downloadDir: picked });
  };

  const save = async () => {
    if (!draft) return;
    setIsSaving(true);
    setError(null);
    try {
      const saved = await updateSettings(draft);
      applyTheme(saved.theme);
      onClose();
    } catch (caught: unknown) {
      setError(
        isCommandError(caught) ? caught.message : "Could not save settings.",
      );
      setIsSaving(false);
    }
  };

  const willRestart =
    draft !== null &&
    original !== null &&
    (draft.downloadDir !== original.downloadDir ||
      draft.listenPort !== original.listenPort ||
      draft.enableDht !== original.enableDht ||
      draft.enableUpnp !== original.enableUpnp ||
      draft.proxyUrl !== original.proxyUrl);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
        className="border-line bg-bg-1 flex max-h-[85vh] w-full max-w-lg flex-col gap-5 overflow-y-auto rounded-xl border p-5 shadow-2xl"
      >
        <h2 className="text-fg-0 text-lg font-semibold">Settings</h2>

        {draft === null ? (
          <Skeleton rows={4} label="Loading settings" />
        ) : (
          <>
            <section className="flex flex-col gap-2">
              <label
                htmlFor="download-dir"
                className="text-fg-3 text-[11px] font-medium tracking-wider uppercase"
              >
                Download folder
              </label>
              <div className="flex gap-2">
                <input
                  id="download-dir"
                  value={draft.downloadDir}
                  onChange={(e) => patch({ downloadDir: e.target.value })}
                  spellCheck={false}
                  className="border-line bg-bg-0 text-fg-0 selectable min-w-0 flex-1 rounded-md border px-3 py-2 font-mono text-xs"
                />
                <Button onClick={() => void chooseFolder()}>Browse…</Button>
              </div>
              <p className="text-fg-3 text-xs">
                Applies to new torrents. Existing ones keep their location.
              </p>
            </section>

            <section className="grid grid-cols-2 gap-4">
              <div className="flex flex-col gap-2">
                <label
                  htmlFor="down-limit"
                  className="text-fg-3 text-[11px] font-medium tracking-wider uppercase"
                >
                  Download limit
                </label>
                <div className="flex items-center gap-2">
                  <input
                    id="down-limit"
                    type="number"
                    min={1}
                    inputMode="numeric"
                    placeholder="Unlimited"
                    value={toKbInput(draft.downloadLimitBps)}
                    onChange={(e) =>
                      patch({ downloadLimitBps: fromKbInput(e.target.value) })
                    }
                    className="border-line bg-bg-0 text-fg-0 placeholder:text-fg-3 w-full rounded-md border px-3 py-2 font-mono text-sm"
                  />
                  <span className="text-fg-2 shrink-0 text-xs">KB/s</span>
                </div>
              </div>

              <div className="flex flex-col gap-2">
                <label
                  htmlFor="up-limit"
                  className="text-fg-3 text-[11px] font-medium tracking-wider uppercase"
                >
                  Upload limit
                </label>
                <div className="flex items-center gap-2">
                  <input
                    id="up-limit"
                    type="number"
                    min={1}
                    inputMode="numeric"
                    placeholder="Unlimited"
                    value={toKbInput(draft.uploadLimitBps)}
                    onChange={(e) =>
                      patch({ uploadLimitBps: fromKbInput(e.target.value) })
                    }
                    className="border-line bg-bg-0 text-fg-0 placeholder:text-fg-3 w-full rounded-md border px-3 py-2 font-mono text-sm"
                  />
                  <span className="text-fg-2 shrink-0 text-xs">KB/s</span>
                </div>
              </div>
            </section>

            <section className="flex flex-col gap-2">
              <label
                htmlFor="listen-port"
                className="text-fg-3 text-[11px] font-medium tracking-wider uppercase"
              >
                Listen port
              </label>
              <input
                id="listen-port"
                type="number"
                min={1}
                max={65535}
                inputMode="numeric"
                value={draft.listenPort}
                onChange={(e) =>
                  patch({ listenPort: Number(e.target.value) || 0 })
                }
                className="border-line bg-bg-0 text-fg-0 w-40 rounded-md border px-3 py-2 font-mono text-sm"
              />
            </section>

            <section className="flex flex-col gap-2">
              <Toggle
                label="Enable DHT"
                hint="Required for magnet links to find peers."
                checked={draft.enableDht}
                onChange={(enableDht) => patch({ enableDht })}
              />
              <Toggle
                label="UPnP port forwarding"
                hint="Asks your router to open the listen port automatically."
                checked={draft.enableUpnp}
                onChange={(enableUpnp) => patch({ enableUpnp })}
              />
            </section>

            <section className="flex flex-col gap-2">
              <label
                htmlFor="proxy-url"
                className="text-fg-3 text-[11px] font-medium tracking-wider uppercase"
              >
                SOCKS5 proxy
              </label>
              <input
                id="proxy-url"
                value={draft.proxyUrl ?? ""}
                onChange={(e) =>
                  patch({ proxyUrl: e.target.value.trim() || null })
                }
                placeholder="Direct connection"
                spellCheck={false}
                className="border-line bg-bg-0 text-fg-0 placeholder:text-fg-3 selectable w-full rounded-md border px-3 py-2 font-mono text-xs"
              />
              <p className="text-fg-3 text-xs">
                Routes outgoing peer connections through a SOCKS5 proxy, for
                example <code>socks5://127.0.0.1:1080</code>.
              </p>
              {draft.proxyUrl ? (
                <p
                  className="border-warn/30 bg-warn/10 text-warn rounded-md border px-3 py-2 text-xs"
                  role="note"
                >
                  This covers <strong>outgoing peer connections only</strong>.
                  Incoming connections still arrive directly on your listen
                  port, and the DHT uses UDP, which a SOCKS5 proxy does not
                  carry. It is not a substitute for a VPN.
                </p>
              ) : null}
            </section>

            <section className="flex flex-col gap-2">
              <span className="text-fg-3 text-[11px] font-medium tracking-wider uppercase">
                Theme
              </span>
              <div
                className="border-line flex gap-1 rounded-md border p-1"
                role="radiogroup"
                aria-label="Theme"
              >
                {(["system", "light", "dark"] as Theme[]).map((option) => (
                  <button
                    key={option}
                    type="button"
                    role="radio"
                    aria-checked={draft.theme === option}
                    onClick={() => {
                      patch({ theme: option });
                      // Preview immediately; persisted on save.
                      applyTheme(option);
                    }}
                    className={`flex-1 rounded px-3 py-1.5 text-sm capitalize transition-colors ${
                      draft.theme === option
                        ? "bg-acc text-bg-0 font-medium"
                        : "text-fg-2 hover:text-fg-0"
                    }`}
                  >
                    {option}
                  </button>
                ))}
              </div>
            </section>

            {willRestart ? (
              <p
                className="border-warn/30 bg-warn/10 text-warn rounded-md border px-3 py-2 text-xs"
                role="status"
              >
                Saving will restart the torrent session. Transfers pause for a
                moment and resume automatically.
              </p>
            ) : null}
          </>
        )}

        {error ? (
          <p
            className="border-err/30 bg-err/10 text-err rounded-md border px-3 py-2 text-xs"
            role="alert"
          >
            {error}
          </p>
        ) : null}

        <div className="flex justify-end gap-2">
          <Button
            variant="ghost"
            onClick={() => {
              // Revert a previewed theme the user did not save.
              if (original) applyTheme(original.theme);
              onClose();
            }}
          >
            Cancel
          </Button>
          <Button
            variant="primary"
            onClick={() => void save()}
            disabled={draft === null || isSaving}
          >
            {isSaving ? "Saving…" : "Save"}
          </Button>
        </div>
      </div>
    </div>
  );
}

/** Props for {@link Toggle}. */
interface ToggleProps {
  label: string;
  hint: string;
  checked: boolean;
  onChange: (next: boolean) => void;
}

/** A labelled checkbox row with explanatory text. */
function Toggle({ label, hint, checked, onChange }: ToggleProps) {
  return (
    <label className="border-line hover:border-fg-2/50 flex cursor-pointer items-start gap-3 rounded-md border p-3">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="accent-acc mt-0.5 h-4 w-4 shrink-0"
      />
      <span className="text-sm">
        <span className="text-fg-0 block">{label}</span>
        <span className="text-fg-3 mt-0.5 block text-xs">{hint}</span>
      </span>
    </label>
  );
}
