"use client";

import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  checkEgress,
  getSettings,
  listEgressInterfaces,
  updateSettings,
} from "@/lib/ipc/client";
import { isCommandError, type Hop, type Settings } from "@/lib/ipc/types";
import {
  SECTIONS,
  SETTING_DEFS,
  searchSettings,
  type AnySettingDef,
  type SectionId,
} from "@/lib/settings/defs";
import { applyDensity, applyRail, applyTheme } from "@/lib/theme";

import { Chip } from "./Chip";
import { Icon } from "./Icon";
import { DiagnosticsCard } from "./DiagnosticsCard";
import { SettingControl } from "./SettingControl";
import { Skeleton } from "./Skeleton";

/** One applied change, kept so it can be undone individually. */
interface Change {
  /** Which setting changed. */
  id: keyof Settings;
  /** Its label, for the footer. */
  label: string;
  /** What it was before. */
  from: unknown;
  /** What it is now. */
  to: unknown;
}

/** Renders a value the way the footer's change list shows it. */
function describeValue(value: unknown): string {
  if (value === null || value === "") return "not set";
  if (typeof value === "boolean") return value ? "on" : "off";
  return String(value);
}

/** Props for {@link SettingsDialog}. */
export interface SettingsDialogProps {
  /** Called when the screen should close. */
  onClose: () => void;
}

/**
 * Settings, generated entirely from `SETTING_DEFS`.
 *
 * There is no OK, Cancel or Apply. Changes take effect as they are made and
 * stack in the footer, each individually undoable — a settings screen with an
 * Apply button asks the user to predict what a setting will do; one that
 * applies immediately lets them see it and change their mind.
 *
 * Search covers labels, config keys, section names, synonyms and the
 * consequence sentences as they currently read, so someone who remembers only
 * the wording they saw can still find the setting that said it.
 *
 * @param props - See {@link SettingsDialogProps}.
 * @returns The rendered screen.
 */
export function SettingsDialog({ onClose }: SettingsDialogProps) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [section, setSection] = useState<SectionId>("speed");
  const [query, setQuery] = useState("");
  const [changes, setChanges] = useState<Change[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [interfaces, setInterfaces] = useState<readonly Hop[]>([]);
  const [activeInterface, setActiveInterface] = useState<string | null>(null);

  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void getSettings()
      .then(setSettings)
      .catch((caught: unknown) =>
        setError(
          isCommandError(caught)
            ? caught.message
            : "Could not read your settings.",
        ),
      );
  }, []);

  // Loaded once when the dialog opens rather than on a timer. Enumerating
  // interfaces costs ~3.2 ms, which is fine for a dialog and is exactly what
  // the guard's per-tick path avoids.
  useEffect(() => {
    void listEgressInterfaces()
      .then(setInterfaces)
      .catch(() => {
        // An empty list renders as "Any tunnel interface" alone, which is the
        // default anyway. Failing to enumerate is not worth an error dialog
        // over a setting most people never open.
      });
    void checkEgress()
      .then((status) =>
        setActiveInterface(status.report.path.v4?.interface ?? null),
      )
      .catch(() => {});
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      // A control that has already handled Escape keeps it. A text field
      // uses it to abandon an edit, and closing the dialog out from under
      // that would throw away the rest of the screen to undo one field.
      // Checked as well as the field stopping propagation, so a future
      // control gets this for free by calling `preventDefault` alone.
      if (e.key === "Escape" && !e.defaultPrevented) onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    dialogRef.current?.focus();
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  /**
   * Serialises the writes, so a slow one cannot land on top of a newer one.
   *
   * `update_settings` takes the whole object and returns the whole object,
   * and `change` installs whatever comes back. Two of those in flight at
   * once is a lost update in both directions: a restart-triggering change
   * can take seconds -- a filter URL is fetched while the session is built
   * -- and the dialog stays interactive throughout, so a theme toggle
   * started later can finish first. The slow reply then overwrites the
   * theme in the UI, and the next edit posts that stale snapshot back and
   * loses it on disk too.
   *
   * The same queue `page.tsx` grew for the same reason when the rail and
   * density chips began racing each other. Here it matters more, because
   * the slow case is now seconds rather than milliseconds.
   */
  const writes = useRef<Promise<unknown>>(Promise.resolve());

  /**
   * How many edits have been made, so a reply can tell if it is the latest.
   *
   * The queue fixes the order writes are *sent* in and not what is installed
   * when they come back. Each reply is the whole settings object as the
   * backend had it at that moment, so an earlier reply does not contain a
   * later edit -- that edit has not been sent yet, it is behind this one in
   * the queue. Installing it anyway snaps the later control back to its old
   * value until its own write finishes, which for a filter URL means the
   * length of an engine restart.
   *
   * So a reply is installed only while it is still the newest word. A later
   * edit installs its own reply, which does contain both.
   */
  const edits = useRef(0);

  /**
   * The latest edit ordinal per field, for the effects that are not state.
   *
   * Theme, density and the rail are applied to the DOM as well as stored,
   * and installing a settings object does not re-apply them. So the global
   * ordinal is the wrong test for *undoing* one: with a later edit to some
   * unrelated field, a failed theme write would skip its own rollback and
   * leave the window dark while the control and the backend both read light.
   *
   * Per field, the question is the one that matters -- has anyone touched
   * *this* since -- and the answer governs the effect while the global
   * ordinal still governs the whole-object install.
   */
  const fieldEdits = useRef(new Map<string, number>());

  /**
   * Writes one field and records it as an undoable change.
   *
   * The optimistic update is deliberate: a control that waits for a round trip
   * before moving feels broken, and every one of these is cheap to put back.
   */
  const change = useCallback(
    async (def: AnySettingDef, next: unknown, record = true) => {
      if (!settings) return;

      const previous = settings[def.id];
      if (previous === next) return;

      const mine = ++edits.current;
      fieldEdits.current.set(def.id, mine);
      /** Whether this edit is still the most recent one made anywhere. */
      const newest = () => edits.current === mine;
      /** Whether this edit is still the most recent one to *this* field. */
      const newestForField = () => fieldEdits.current.get(def.id) === mine;

      const updated = { ...settings, [def.id]: next } as Settings;
      setSettings(updated);
      setError(null);

      // Theme and density are frontend-only, so they take effect here rather
      // than waiting for the engine to acknowledge a write it does not act on.
      if (def.id === "theme") applyTheme(updated.theme);
      if (def.id === "density") applyDensity(updated.density);
      // Without this the sidebar setting persists and does nothing until the
      // next launch -- a control that appears to have no effect, which is
      // worse than one that is missing.
      if (def.id === "rail") applyRail(updated.rail);

      if (record) {
        setChanges((current) => [
          ...current,
          { id: def.id, label: def.label, from: previous, to: next },
        ]);
      }

      try {
        // Queued, and the payload is built when the turn comes rather than
        // now. Queueing `updated` would send a snapshot of the whole object
        // taken before the writes ahead of it ran -- so if one of those
        // fails and the backend rolls its field back, this write posts the
        // rolled-back value straight over the top and fails in its turn.
        //
        // One field, applied to whatever the backend actually holds. The
        // same read-then-merge `page.tsx` uses, which is the half of that
        // pattern this screen was missing.
        const write = writes.current
          .catch(() => {})
          .then(() => getSettings())
          .then((current) =>
            updateSettings({ ...current, [def.id]: next } as Settings),
          );
        writes.current = write.catch(() => {});
        const authoritative = await write;
        // Discarded rather than installed if something has been typed since.
        // The newer edit is holding optimistic state this reply predates,
        // and its own reply will carry both.
        if (newest()) setSettings(authoritative);
      } catch (caught: unknown) {
        // Put the control back -- but to what the backend holds, not to the
        // snapshot this render closed over. A failed session change is
        // rolled back on the Rust side, and that rollback is the truth; the
        // local snapshot still carries the value that was rejected.
        // Same restraint on the failure path, and for the same reason: the
        // backend state after this write was rolled back does not contain a
        // later edit either. The error is reported regardless -- the user
        // needs to know this change did not stick, whatever they have typed
        // since.
        const rolled = await getSettings().catch(() => settings);
        if (newest()) setSettings(rolled);
        // The DOM effects answer to this field alone. A later edit to some
        // *other* field must not strand the window dark after a failed
        // theme write, and installing a settings object does not re-apply
        // them -- only these calls do.
        if (newestForField()) {
          if (def.id === "theme") applyTheme(rolled.theme);
          if (def.id === "density") applyDensity(rolled.density);
          if (def.id === "rail") applyRail(rolled.rail);
        }
        setChanges((current) => current.slice(0, -1));
        setError(
          isCommandError(caught)
            ? caught.message
            : "Could not save that change.",
        );
      }
    },
    [settings],
  );

  const undo = useCallback(
    (index: number) => {
      const target = changes[index];
      const def = SETTING_DEFS.find((d) => d.id === target.id);
      if (!def) return;

      setChanges((current) => current.filter((_, i) => i !== index));
      void change(def, target.from, false);
    },
    [changes, change],
  );

  const revertAll = useCallback(() => {
    // Newest first, so a field changed twice ends on its original value
    // rather than on whatever the earliest entry happened to record.
    for (const entry of [...changes].reverse()) {
      const def = SETTING_DEFS.find((d) => d.id === entry.id);
      if (def) void change(def, entry.from, false);
    }
    setChanges([]);
  }, [changes, change]);

  const searching = query.trim() !== "";
  const shown = useMemo(() => {
    if (!settings) return [];
    const matched = searchSettings(query, settings);
    return searching ? matched : matched.filter((d) => d.section === section);
  }, [settings, query, searching, section]);

  const active = SECTIONS.find((s) => s.id === section);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
        tabIndex={-1}
        className="border-line bg-bg-0 flex h-[820px] max-h-[90vh] w-full max-w-[1200px] flex-col overflow-hidden rounded-lg border shadow-2xl outline-none"
      >
        <div className="border-line bg-bg-1 flex shrink-0 items-center gap-3.5 border-b px-6 py-3.5">
          <h2 className="text-[17px] font-semibold tracking-[-0.02em]">
            Settings
          </h2>
          <div className="relative ml-auto flex items-center">
            <span className="text-fg-3 pointer-events-none absolute left-[9px]">
              <Icon name="search" size={14} />
            </span>
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search settings"
              aria-label="Search settings"
              className="border-line bg-bg-2 text-fg-0 placeholder:text-fg-3 focus:border-acc-dim h-[var(--flume-h-control)] w-[260px] rounded-md border pr-3 pl-[30px] text-[12.5px] outline-none"
            />
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close settings"
            className="text-fg-2 hover:text-fg-0"
          >
            <Icon name="plus" size={16} className="rotate-45" />
          </button>
        </div>

        <div className="flex min-h-0 grow">
          <nav
            className="border-line bg-bg-1 flex w-[222px] shrink-0 flex-col gap-0.5 border-r px-2.5 py-3"
            aria-label="Settings sections"
          >
            {SECTIONS.map((s) => {
              const on = !searching && s.id === section;
              return (
                <button
                  key={s.id}
                  type="button"
                  aria-current={on ? "page" : undefined}
                  onClick={() => {
                    setQuery("");
                    setSection(s.id);
                  }}
                  className={`flex h-[34px] items-center gap-2.5 rounded-md px-2.5 text-left text-[12.5px] transition-colors ${
                    on
                      ? "bg-bg-3 text-fg-0 font-medium"
                      : "text-fg-1 hover:bg-bg-2 hover:text-fg-0"
                  }`}
                >
                  <Icon name={s.icon} size={16} />
                  {s.name}
                </button>
              );
            })}
          </nav>

          <div className="min-w-0 grow overflow-y-auto px-6 pt-1.5 pb-6">
            {settings === null && error === null ? (
              <div className="pt-6">
                <Skeleton label="Loading settings" rows={4} />
              </div>
            ) : null}

            {settings !== null && !searching && active ? (
              <div className="pt-5 pb-1.5">
                <div className="text-[15px] font-semibold tracking-[-0.015em]">
                  {active.name}
                </div>
                <div className="text-fg-2 mt-0.5 text-xs">
                  {active.description}
                </div>
              </div>
            ) : null}

            {settings !== null && searching ? (
              <div className="text-fg-3 pt-5 pb-1.5 text-[10px] font-semibold tracking-[0.09em] uppercase">
                {shown.length} {shown.length === 1 ? "result" : "results"}
              </div>
            ) : null}

            {settings !== null && searching && shown.length === 0 ? (
              <p className="text-fg-2 py-6 text-[12.5px]">
                Nothing matches “{query.trim()}”. Search covers the setting
                names, their config keys, and the sentence under each one.
              </p>
            ) : null}

            {settings !== null
              ? shown.map((def) => {
                  const sectionOf = SECTIONS.find((s) => s.id === def.section);
                  const value = settings[def.id];
                  const consequence = (
                    def.consequence as (v: unknown) => string
                  )(value);

                  return (
                    <div
                      key={def.id}
                      className="border-line flex items-start gap-6 border-b py-4"
                    >
                      <div className="min-w-0 grow">
                        {searching && sectionOf ? (
                          <div className="text-acc mb-[3px] text-[10.5px]">
                            {sectionOf.name}
                          </div>
                        ) : null}
                        <div className="flex items-center gap-2.5">
                          <b className="text-[13.5px] font-semibold tracking-[-0.008em]">
                            {def.label}
                          </b>
                          <span className="flume-num border-line bg-bg-2 text-fg-3 rounded-[3px] border px-[5px] py-px text-[10px] whitespace-nowrap">
                            {def.key}
                          </span>
                          {def.restartsSession ? (
                            <span className="text-warn bg-warn/15 rounded-[3px] px-[5px] py-px text-[10px] font-medium">
                              restarts the engine
                            </span>
                          ) : null}
                        </div>
                        <p className="text-fg-1 mt-1 max-w-[620px] text-xs leading-[1.5]">
                          {consequence}
                        </p>
                      </div>

                      <div className="shrink-0 pt-0.5">
                        <SettingControl
                          control={def.control}
                          value={value}
                          label={def.label}
                          onChange={(next) => void change(def, next)}
                          interfaces={interfaces}
                          activeInterface={activeInterface}
                          onBrowse={async () => {
                            const picked = await open({
                              directory: true,
                              multiple: false,
                            });
                            if (typeof picked === "string") {
                              void change(def, picked);
                            }
                          }}
                        />
                      </div>
                    </div>
                  );
                })
              : null}

            {/*
              The one thing on this screen that is not generated from
              SETTING_DEFS. That table binds each row to a `Settings` field,
              and a report you build on demand is an action with no field to
              bind to. Hidden while searching rather than given a fake entry in
              the search index, so the "N results" count stays truthful.
            */}
            {settings !== null && !searching && section === "privacy" ? (
              <DiagnosticsCard />
            ) : null}
          </div>
        </div>

        <div className="border-line bg-bg-1 flex min-h-[62px] shrink-0 items-center gap-3.5 border-t px-6 py-2">
          <span className={`shrink-0 ${error ? "text-err" : "text-ok"}`}>
            <Icon name={error ? "alert-circle" : "check-circle"} size={16} />
          </span>
          <span className="shrink-0 text-[12.5px]">
            {error ? (
              <span className="text-err" role="alert">
                {error}
              </span>
            ) : (
              <span className="text-fg-1">
                {changes.length === 0
                  ? "Changes apply as you make them."
                  : `${changes.length} ${changes.length === 1 ? "change" : "changes"} applied.`}
              </span>
            )}
          </span>

          <div className="flex min-w-0 flex-wrap gap-1.5 overflow-hidden">
            {changes.slice(-3).map((entry, index) => (
              <button
                key={`${entry.id}-${index}`}
                type="button"
                onClick={() =>
                  undo(changes.length - Math.min(3, changes.length) + index)
                }
                title={`Undo: ${entry.label}`}
                className="border-line text-fg-2 hover:border-line-2 hover:text-fg-0 flex items-center gap-1.5 rounded-sm border px-2 py-1 text-[11px]"
              >
                {entry.label}
                <s className="text-fg-3">{describeValue(entry.from)}</s>
                <span className="text-fg-0">{describeValue(entry.to)}</span>
              </button>
            ))}
          </div>

          <span className="grow" />

          {changes.length > 0 ? (
            <>
              <Chip onClick={() => undo(changes.length - 1)}>Undo last</Chip>
              <Chip onClick={revertAll}>Revert all</Chip>
            </>
          ) : null}
        </div>
      </div>
    </div>
  );
}
