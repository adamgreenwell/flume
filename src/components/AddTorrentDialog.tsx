"use client";

import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { selectedBytes as sumSelected } from "@/lib/filetree";
import { formatBytes } from "@/lib/format";
import {
  confirmAdd,
  destinationPreflight,
  discardPreview,
  previewTorrent,
} from "@/lib/ipc/client";
import {
  isCommandError,
  type DestinationPreflight,
  type TorrentPreview,
} from "@/lib/ipc/types";
import { looksLikeMagnet } from "@/lib/magnet";

import { Button } from "./Button";
import { FileTree } from "./FileTree";
import { Icon } from "./Icon";
import { PreflightTiles } from "./PreflightTiles";

/**
 * Whether two paths name the same folder, ignoring a trailing separator.
 *
 * librqbit hands back the folder a list-only add resolved to *with* a
 * trailing separator, which the engine test
 * `a_chosen_folder_overrides_the_session_default` documents. A raw string
 * comparison against the remembered folder is therefore true even when both
 * are the same directory — so every untouched add looked like a custom one,
 * persisted the global folder as the remembered one, and then kept writing
 * there after the global setting changed.
 *
 * Only the trailing separator, deliberately. Case folding would be wrong on a
 * case-sensitive volume, and resolving `.`/`..` or symlinks is not something
 * the webview can do — it never touches the filesystem. This equates the one
 * divergence that is known to happen rather than guessing at path identity.
 *
 * @param a - A path.
 * @param b - Another path.
 * @returns Whether they name the same folder.
 */
function sameFolder(a: string, b: string): boolean {
  const trimmed = (path: string) => path.replace(/[/\\]+$/, "");
  return trimmed(a) === trimmed(b);
}

/** Props for {@link AddTorrentDialog}. */
export interface AddTorrentDialogProps {
  /** Called when the dialog should close, after any cleanup. */
  onClose: () => void;
  /** Optional magnet URI to prefill, e.g. detected from the clipboard. */
  initialMagnet?: string;
  /** A `.torrent` path to resolve immediately, from a drag-and-drop. */
  droppedPath?: string;
  /**
   * Where this add will write, unless the user changes it.
   *
   * The last folder an add used, or the global download folder when there
   * has not been one. Passed in rather than read here so the screen has one
   * source for it.
   */
  defaultFolder?: string;
  /**
   * Called with the folder an add actually used, to be remembered.
   *
   * Only on a successful add: remembering a destination the user abandoned
   * would make the next add default to a choice they backed out of.
   */
  onFolderUsed?: (folder: string) => void;
  /**
   * Recent download rate to estimate the finish time against, or `null`.
   *
   * The design calls for the user's real rolling average. Flume does not
   * persist one across sessions yet, so this is the current session's — an
   * honest measurement of a shorter window, which the tile says out loud.
   */
  rateBps?: number | null;
}

/**
 * Two-step add flow: resolve metadata, then choose files, then download.
 *
 * Nothing is downloaded until the user confirms. That is the whole point for
 * the ISO use case — distro torrents routinely bundle several images plus
 * checksums when the user wants one.
 *
 * Resolving a magnet fetches metadata over the DHT and can take seconds, so
 * the resolving state is a real state with its own affordances, not a spinner
 * bolted onto the input.
 *
 * @param props - See {@link AddTorrentDialogProps}.
 * @returns The rendered dialog.
 */
export function AddTorrentDialog({
  onClose,
  initialMagnet = "",
  droppedPath,
  rateBps = null,
  defaultFolder,
  onFolderUsed,
}: AddTorrentDialogProps) {
  const [magnet, setMagnet] = useState(initialMagnet);
  const [preview, setPreview] = useState<TorrentPreview | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  // A dialog opened by a drop starts already resolving, so the effect below
  // never has to flip this synchronously.
  const [isResolving, setIsResolving] = useState(Boolean(droppedPath));
  const [waitedSeconds, setWaitedSeconds] = useState(0);
  const [isAdding, setIsAdding] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // The chosen destination, and the preflight re-answered for it.
  //
  // `null` in `checked` means nothing has been chosen, so the preview's own
  // figures stand. Once a folder is picked they are replaced rather than
  // adjusted: free space belongs to a volume and "already on disk" to a
  // folder, so neither survives the change.
  const [folder, setFolder] = useState<string | null>(null);
  const [checked, setChecked] = useState<DestinationPreflight | null>(null);
  const [folderError, setFolderError] = useState<string | null>(null);
  // Whether a check is in flight, and which folder the latest one is for.
  //
  // Both are needed and they answer different questions. The flag gates
  // confirmation, because adding against the previous folder's free space is
  // exactly the mistake this sheet exists to prevent. The ref discards a
  // reply that is no longer about the chosen folder, because two picks in
  // quick succession can land out of order and the slower one would win.
  const [checking, setChecking] = useState(false);
  const checkingFor = useRef<string | null>(null);

  const dialogRef = useRef<HTMLDivElement>(null);
  // Kept in a ref so the unmount cleanup does not need `preview` as a dep,
  // which would re-run cleanup on every preview change.
  const pendingHashRef = useRef<string | null>(null);

  const describe = (caught: unknown, fallback: string) =>
    isCommandError(caught) ? caught.message : fallback;

  const checkFolder = useCallback(async (hash: string, dir: string) => {
    checkingFor.current = dir;
    setChecking(true);
    setFolderError(null);
    try {
      const result = await destinationPreflight(hash, dir);
      // Superseded while this was in flight; the newer check owns the state.
      if (checkingFor.current !== dir) return;
      setChecked(result);
    } catch (caught: unknown) {
      if (checkingFor.current !== dir) return;
      // The figures are dropped rather than left describing the old folder.
      // An honest "unknown" is what `PreflightTiles` renders for null, and
      // it beats a number about somewhere else.
      setChecked({ freeBytes: null, alreadyOnDisk: [] });
      setFolderError(describe(caught, "Could not check that folder."));
    } finally {
      if (checkingFor.current === dir) setChecking(false);
    }
  }, []);

  /** Applies a successfully resolved preview. */
  const applyPreview = useCallback(
    (resolved: TorrentPreview) => {
      setPreview(resolved);
      pendingHashRef.current = resolved.infoHash;

      // The remembered folder becomes the real destination, not just the one
      // on display. Leaving `folder` null while showing it meant the sheet
      // named one folder and the add wrote to another -- and the preflight
      // described the one it was not using. Only when it actually differs
      // from where the session would put it: equal values should stay `null`
      // so the engine's own default remains the single source of that path.
      // Not for a torrent already in the list. Adding it again replaces its
      // file selection and leaves its data where it is, so a destination is
      // not a choice there -- and preflighting one would check a folder
      // nothing will be written to.
      if (
        !resolved.alreadyAdded &&
        defaultFolder &&
        !sameFolder(defaultFolder, resolved.savePath)
      ) {
        setFolder(defaultFolder);
        void checkFolder(resolved.infoHash, defaultFolder);
      }

      // Everything selected by default except what is already on disk. The
      // common case is wanting the whole torrent, and starting from nothing
      // selected makes the primary action dead on arrival — but re-fetching
      // a file the user already has is the one thing an add sheet exists to
      // prevent, so those start off and the footer says why.
      //
      // Against the *preview's* answer, which is about the session folder. A
      // remembered folder is checked just above and its reply arrives later;
      // it updates the marks, not this selection, for the reason the picker
      // does not either -- see `chooseFolder`.
      setSelected(
        new Set(
          resolved.files
            .filter((f) => !resolved.alreadyOnDisk[f.index])
            .map((f) => f.index),
        ),
      );
      setError(null);
      setIsResolving(false);
    },
    [defaultFolder, checkFolder],
  );

  /** Records a failed resolution. */
  const failPreview = useCallback((caught: unknown) => {
    setError(describe(caught, "Could not read that torrent."));
    setIsResolving(false);
  }, []);

  // Counts while a resolve is in flight, and resets when it is not. A magnet's
  // wait is bounded in the engine, but a static sentence gives the user no way
  // to tell a slow answer from a dead one before that deadline arrives.
  useEffect(() => {
    if (!isResolving) return;

    const started = Date.now();
    const id = setInterval(() => {
      setWaitedSeconds(Math.floor((Date.now() - started) / 1000));
    }, 1000);
    return () => clearInterval(id);
  }, [isResolving]);

  /** Resolves from a user gesture (button press or file picker). */
  const resolve = useCallback(
    async (source: Parameters<typeof previewTorrent>[0]) => {
      setError(null);
      setWaitedSeconds(0);
      setIsResolving(true);
      try {
        applyPreview(await previewTorrent(source));
      } catch (caught: unknown) {
        failPreview(caught);
      }
    },
    [applyPreview, failPreview],
  );

  // Offer a magnet the user already copied. Read on open rather than on every
  // window focus: opening this dialog is a deliberate act, so reading the
  // clipboard then is expected rather than background snooping.
  useEffect(() => {
    if (initialMagnet) return;
    let active = true;
    readText()
      .then((text) => {
        if (active && text && looksLikeMagnet(text)) setMagnet(text.trim());
      })
      .catch(() => {
        // No clipboard access, or nothing text-shaped in it. Not worth saying.
      });
    return () => {
      active = false;
    };
  }, [initialMagnet]);

  // A file dropped onto the window resolves immediately. State is only ever
  // set from the promise callbacks, never synchronously in the effect body.
  useEffect(() => {
    if (!droppedPath) return;
    let active = true;
    previewTorrent({ kind: "file", path: droppedPath })
      .then((resolved) => {
        if (active) applyPreview(resolved);
      })
      .catch((caught: unknown) => {
        if (active) failPreview(caught);
      });
    return () => {
      active = false;
    };
  }, [droppedPath, applyPreview, failPreview]);

  const pickFile = async () => {
    const path = await open({
      multiple: false,
      filters: [{ name: "Torrent", extensions: ["torrent"] }],
    });
    if (typeof path !== "string") return;

    // The path goes to Rust, which reads the file. The webview never touches
    // the filesystem.
    await resolve({ kind: "file", path });
  };

  /**
   * Chooses a destination and re-answers the preflight for it.
   *
   * The selection is deliberately left alone. It started as "everything you
   * do not already have", and a file that is absent from the new folder is
   * now shown unmarked and unselected rather than being silently ticked --
   * re-selecting files the user may have deselected on purpose would be
   * worse than making them press Select all.
   */
  const chooseFolder = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked !== "string" || !preview) return;

    setFolder(picked);
    await checkFolder(preview.infoHash, picked);
  };

  const confirm = async () => {
    if (!preview) return;
    setIsAdding(true);
    setError(null);
    try {
      const all = selected.size === preview.files.length;
      // `null` when the user did not choose one, so the engine uses the
      // session's own folder rather than a copy of the global setting made
      // here. The two are the same today and would drift the moment the
      // setting changed while a dialog was open.
      await confirmAdd(preview.infoHash, all ? null : [...selected], folder);
      // Remembered only now, with the add through: a destination recorded
      // when it was picked would outlive a cancelled dialog.
      //
      // The `alreadyAdded` half cannot currently fire -- that case shows no
      // Change button and adopts no default, so `folder` is always null
      // there. It is here as the statement of the rule rather than as the
      // thing enforcing it: the engine returns `AlreadyManaged` and leaves
      // the files alone, so a folder chosen for one was never used, and
      // remembering it would default the *next* add to it. If the row is
      // ever shown for an existing torrent, this is what stops that
      // regression.
      if (folder && !preview.alreadyAdded) onFolderUsed?.(folder);
      pendingHashRef.current = null;
      onClose();
    } catch (caught: unknown) {
      setError(describe(caught, "Could not start that torrent."));
      setIsAdding(false);
    }
  };

  // Release any preview the user abandoned, so its resolved metadata is not
  // retained in the engine for the life of the process.
  useEffect(() => {
    return () => {
      const hash = pendingHashRef.current;
      if (hash) void discardPreview(hash).catch(() => {});
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    dialogRef.current?.focus();
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const chosenBytes = preview ? sumSelected(preview.files, selected) : 0;

  // The destination in force, and the figures that describe it.
  //
  // `checked` wins once a folder has been chosen, because the preview's
  // answers were about a different folder. Not merged -- there is nothing
  // to merge, they are answers to the same two questions about somewhere
  // else.
  const destination = folder ?? defaultFolder ?? preview?.savePath ?? "";
  const freeBytes = checked ? checked.freeBytes : (preview?.freeBytes ?? null);
  const alreadyOnDisk = checked
    ? checked.alreadyOnDisk
    : (preview?.alreadyOnDisk ?? []);

  // Indices already present at full length, as a set for the tree to test.
  const onDisk = useMemo(
    () =>
      new Set(
        (preview?.files ?? [])
          .filter((f) => alreadyOnDisk[f.index])
          .map((f) => f.index),
      ),
    [preview, alreadyOnDisk],
  );

  // Files the user has chosen anyway, despite already having them. The footer
  // says so rather than silently re-downloading: this is the exact mistake the
  // sheet exists to catch.
  const reFetching = [...onDisk].filter((index) => selected.has(index));
  const reFetchBytes = preview
    ? preview.files
        .filter((f) => reFetching.includes(f.index))
        .reduce((sum, f) => sum + f.length, 0)
    : 0;

  if (preview === null) {
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
          aria-label="Add a torrent"
          tabIndex={-1}
          className="border-line bg-bg-1 flex w-full max-w-xl flex-col gap-4 rounded-lg border p-5 shadow-2xl outline-none"
        >
          <h2 className="text-fg-0 text-[15px] font-semibold tracking-[-0.015em]">
            Add a torrent
          </h2>

          <div className="flex flex-col gap-2">
            <label
              htmlFor="magnet-input"
              className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase"
            >
              Magnet link
            </label>
            <div className="flex gap-2">
              <input
                id="magnet-input"
                value={magnet}
                onChange={(e) => setMagnet(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && magnet.trim() && !isResolving) {
                    void resolve({ kind: "magnet", uri: magnet.trim() });
                  }
                }}
                placeholder="magnet:?xt=urn:btih:…"
                spellCheck={false}
                autoFocus
                className="border-line bg-bg-2 text-fg-0 placeholder:text-fg-3 selectable h-[var(--flume-h-control)] min-w-0 flex-1 rounded-md border px-3 font-mono text-[12.5px]"
              />
              <Button
                variant="primary"
                disabled={!magnet.trim() || isResolving}
                onClick={() =>
                  void resolve({ kind: "magnet", uri: magnet.trim() })
                }
              >
                {isResolving ? "Resolving…" : "Resolve"}
              </Button>
            </div>
          </div>

          <div className="flex items-center gap-3">
            <span className="bg-line h-px flex-1" />
            <span className="text-fg-3 text-xs">or</span>
            <span className="bg-line h-px flex-1" />
          </div>

          <Button onClick={() => void pickFile()} disabled={isResolving}>
            Choose a .torrent file…
          </Button>

          {isResolving ? (
            <p className="text-fg-2 text-[12.5px]" role="status">
              Fetching the file list from peers over the DHT. Nothing is
              downloaded yet — this is only the list of what the torrent
              contains.
              {/* A magnet carries no file list, so this wait depends on
                  another peer answering. The count is what distinguishes
                  "still working" from "stuck" — without it the sentence reads
                  the same after two seconds and after a minute. */}
              {waitedSeconds > 0 ? (
                <>
                  {" "}
                  <span className="flume-num">{waitedSeconds}s</span> so far.
                </>
              ) : null}
            </p>
          ) : null}

          {error ? (
            <p
              className="border-err/30 bg-err/10 text-err rounded-md border px-3 py-2 text-[12.5px]"
              role="alert"
            >
              {error}
            </p>
          ) : null}

          <div className="flex justify-end">
            <Button variant="ghost" onClick={onClose}>
              Cancel
            </Button>
          </div>
        </div>
      </div>
    );
  }

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
        aria-label={`Review ${preview.name} before downloading`}
        tabIndex={-1}
        className="border-line bg-bg-1 flex max-h-[90vh] w-full max-w-[1040px] flex-col overflow-hidden rounded-lg border shadow-2xl outline-none"
      >
        <div className="border-line flex shrink-0 items-start gap-3.5 border-b px-5 py-4">
          <span className="bg-acc-deep text-acc flex h-[34px] w-[34px] shrink-0 items-center justify-center rounded-[7px]">
            <Icon name="arrow-down" size={19} />
          </span>
          <div className="min-w-0 grow">
            <span className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase">
              {preview.seenPeers > 0
                ? "Magnet link · file list fetched from peers"
                : "Torrent file"}
            </span>
            <div
              className="truncate text-[17px] font-semibold tracking-[-0.02em]"
              title={preview.name}
            >
              {preview.name}
            </div>
            <div className="text-fg-2 mt-0.5 flex flex-wrap items-center gap-2 text-[11.5px]">
              <span className="flume-num selectable">
                btih:{preview.infoHash.slice(0, 8)}…{preview.infoHash.slice(-4)}
              </span>
              <span className="text-fg-3">·</span>
              <span className="text-ok bg-ok/15 inline-flex items-center gap-1.5 rounded-sm px-1.5 py-0.5 font-medium">
                <Icon name="check" size={11} />
                {preview.files.length}{" "}
                {preview.files.length === 1 ? "file" : "files"} verified against
                the info hash
              </span>
            </div>
          </div>
        </div>

        {/*
          Hidden for a torrent already in the list. `savePath` describes where
          a *new* add would go -- a list-only add resolves the session folder,
          not the existing torrent's -- so showing it beside a torrent whose
          files are elsewhere would assert the wrong path. The notice below
          says the destination cannot be changed instead.
        */}
        {preview.alreadyAdded ? null : (
          <div className="border-line flex shrink-0 items-center gap-2 border-b px-3.5 py-2">
            <span className="text-fg-3 shrink-0 text-[10px] font-semibold tracking-[0.09em] uppercase">
              Saving to
            </span>
            {/*
            Truncated from the left, so the end of a long path -- the part
            that says which folder this actually is -- stays visible. `dir`
            is what makes the ellipsis land at the start.
          */}
            <span
              dir="rtl"
              title={destination}
              className="text-fg-1 min-w-0 grow truncate text-left text-[12px]"
            >
              {destination}
            </span>
            <Button onClick={() => void chooseFolder()}>Change</Button>
          </div>
        )}
        {folderError === null ? null : (
          <p className="text-warn shrink-0 px-3.5 py-1.5 text-[11px]">
            {folderError} The figures below are shown as unknown rather than
            describing the previous folder.
          </p>
        )}

        <PreflightTiles
          seenPeers={preview.seenPeers}
          selectedBytes={chosenBytes}
          totalBytes={preview.totalBytes}
          selectedCount={selected.size}
          totalCount={preview.files.length}
          freeBytes={freeBytes}
          rateBps={rateBps}
        />

        <div className="flex min-h-0 grow flex-col">
          <div className="border-line flex shrink-0 items-center gap-1.5 border-b px-3.5 py-2">
            <span className="text-fg-3 mr-auto text-[10px] font-semibold tracking-[0.09em] uppercase">
              Contents
            </span>
            <Button
              onClick={() =>
                setSelected(new Set(preview.files.map((f) => f.index)))
              }
            >
              Select all
            </Button>
            <Button onClick={() => setSelected(new Set())}>Clear</Button>
          </div>

          <FileTree
            files={preview.files}
            selected={selected}
            onChange={setSelected}
            onDisk={onDisk}
          />
        </div>

        {preview.alreadyAdded ? (
          <p
            className="border-line bg-warn/10 text-warn shrink-0 border-t px-5 py-2 text-[12.5px]"
            role="status"
          >
            This torrent is already in your list. Adding it again replaces its
            file selection, and leaves its files where they are — the
            destination cannot be changed here.
          </p>
        ) : null}

        {error ? (
          <p
            className="border-line bg-err/10 text-err shrink-0 border-t px-5 py-2 text-[12.5px]"
            role="alert"
          >
            {error}
          </p>
        ) : null}

        <div className="border-line flex min-h-[70px] shrink-0 items-center gap-2.5 border-t px-5 py-2">
          <span
            className={`shrink-0 ${reFetching.length > 0 ? "text-warn" : "text-ok"}`}
          >
            <Icon
              name={reFetching.length > 0 ? "alert-triangle" : "check-circle"}
              size={16}
            />
          </span>
          <span className="text-fg-1 text-xs">
            {onDisk.size === 0
              ? "Nothing here is on disk yet. Nothing has been downloaded — only the file list was fetched."
              : reFetching.length > 0
                ? `${reFetching.length} of these ${reFetching.length === 1 ? "file is" : "files are"} already on disk. Leaving ${reFetching.length === 1 ? "it" : "them"} selected re-downloads ${formatBytes(reFetchBytes)} you already have.`
                : `${onDisk.size} ${onDisk.size === 1 ? "file is" : "files are"} already on disk — deselected so ${onDisk.size === 1 ? "it is" : "they are"} not fetched twice.`}
          </span>

          <span className="grow" />

          <Button variant="ghost" size="dialog" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="primary"
            size="dialog"
            onClick={() => void confirm()}
            // Also while a folder check is in flight. Confirming then would
            // start the torrent against the *previous* folder's free space
            // and on-disk marks, which is the decision this sheet exists to
            // inform -- and the figures beside the button would be describing
            // somewhere else at the moment it was pressed.
            disabled={selected.size === 0 || isAdding || checking}
          >
            {isAdding
              ? "Starting…"
              : checking
                ? "Checking that folder…"
                : selected.size === 0
                  ? "Nothing selected"
                  : `Add ${selected.size} ${selected.size === 1 ? "file" : "files"} · ${formatBytes(chosenBytes)}`}
          </Button>
        </div>
      </div>
    </div>
  );
}
