import type { UpdateFailure, UpdateStatus } from "@/lib/ipc/types";

import { Button } from "./Button";
import { Icon, type IconName } from "./Icon";

/** Props for {@link UpdateBanner}. */
export interface UpdateBannerProps {
  /** The latest status, or `null` before one arrives. */
  status: UpdateStatus | null;
  /**
   * Whether the egress guard is holding. An announced update stays on screen
   * through a hold -- Rust never replaces it -- so this is what explains why
   * Install is waiting, and keeps the button from offering a download Flume
   * would refuse.
   */
  guardHeld: boolean;
  /**
   * Whether the person asked for the last check. Only an answer to a question
   * someone asked is worth interrupting with: the daily check speaks up when
   * there is something to install and is silent otherwise.
   */
  asked: boolean;
  /** The {@link dismissKey} of a message dismissed this session, if any. */
  dismissed: string | null;
  /** Installs the announced update and restarts. */
  onInstall: () => void;
  /** Opens the announced version's release page. */
  onDownload: () => void;
  /** Hides the notice; passed its {@link dismissKey}. */
  onDismiss: (key: string) => void;
}

/** One sentence per failure, because "it failed" is not something to act on. */
const FAILURE: Record<UpdateFailure, string> = {
  network:
    "No answer from GitHub -- offline, DNS, or a proxy or firewall in the way.",
  manifest: "GitHub answered, but not with a release Flume could read.",
  signature:
    "The download did not match its signature, so nothing was installed.",
  install: "The download or install did not finish. Nothing was changed.",
};

/**
 * What "Not now" or "Dismiss" hides, for the rest of the session.
 *
 * Keyed by meaning rather than by object: every status arrives as a new
 * object, so identity would never match. For an update the meaning is its
 * version, so a newer one still appears after "Not now".
 */
export function dismissKey(status: UpdateStatus): string {
  switch (status.state) {
    case "available":
      return `available:${status.version}`;
    case "failed":
      return `failed:${status.stage}:${status.reason}`;
    default:
      return status.state;
  }
}

interface Message {
  icon: IconName;
  tone: string;
  /** Read by screen readers ahead of the title; status is never colour alone. */
  label: string;
  title: string;
  body: string;
  actions: "install" | "download" | "dismiss" | "none";
  /** Install is offered but cannot run yet, because the guard is holding. */
  waiting: boolean;
}

/**
 * What to say for a status, or `null` for nothing.
 *
 * Exported for tests: the banner's whole job is choosing the right sentence,
 * and that choice is cheaper to pin here than through rendered markup.
 */
export function describeUpdate(
  status: UpdateStatus | null,
  asked: boolean,
  dismissed: string | null,
  guardHeld = false,
): Message | null {
  if (!status) return null;
  if (dismissed !== null && dismissed === dismissKey(status)) return null;

  const quiet = { waiting: false } as const;

  switch (status.state) {
    case "available": {
      const inPlace = status.delivery === "inPlace";
      const waiting = inPlace && guardHeld;
      return {
        icon: "arrow-down",
        tone: "text-acc",
        label: "Update",
        title: `Flume ${status.version} is available`,
        body: waiting
          ? "Flume will not download it while the tunnel guard is holding. Install once traffic is leaving through the tunnel again."
          : inPlace
            ? "Installing restarts Flume. Torrents pick up where they left off, and nothing installs until you choose to."
            : "This install is updated by hand or by your package manager, so Flume opens the release page rather than replacing itself.",
        actions: inPlace ? "install" : "download",
        waiting,
      };
    }

    case "installing":
      return {
        ...quiet,
        icon: "clock",
        tone: "text-fg-2",
        label: "Working",
        title: "Installing the update",
        body: "Flume will restart when it is done.",
        actions: "none",
      };

    // A failed install is always worth saying, asked or not: the person
    // clicked Install, and silence would look like it worked. A failed
    // *check* is said only when someone asked -- the daily one failing on an
    // offline laptop is not news, and saying so at every launch would teach
    // people to ignore this banner.
    case "failed":
      if (!asked && status.stage !== "install") return null;
      return {
        ...quiet,
        icon: "alert-triangle",
        tone: "text-warn",
        label: "Worth knowing",
        title: "Could not update",
        body: FAILURE[status.reason],
        actions: "dismiss",
      };

    case "idle":
      return null;

    // The rest answer a question, so they need someone to have asked it.
    case "checking":
      return asked
        ? {
            ...quiet,
            icon: "clock",
            tone: "text-fg-2",
            label: "Working",
            title: "Checking for updates",
            body: "Asking GitHub for the latest release.",
            actions: "none",
          }
        : null;

    case "upToDate":
      return asked
        ? {
            ...quiet,
            icon: "check-circle",
            tone: "text-ok",
            label: "Fine",
            title: "Flume is up to date",
            body: "There is no newer release.",
            actions: "dismiss",
          }
        : null;

    case "held":
      return asked
        ? {
            ...quiet,
            icon: "shield",
            tone: "text-warn",
            label: "Worth knowing",
            title: "Not checked while the tunnel guard is holding",
            // No promise of a retry: only the daily check waits for the
            // guard to release. A check someone asked for is theirs to repeat.
            body: "Flume makes no connections from the address the guard protects. Choose Check for Updates again once traffic is leaving through the tunnel.",
            actions: "dismiss",
          }
        : null;

    case "unconfigured":
      return asked
        ? {
            ...quiet,
            icon: "alert-circle",
            tone: "text-fg-2",
            label: "Note",
            title: "This build cannot check for updates",
            body: "It was built without an update signing key -- usually a development build or a fork. Releases from GitHub can.",
            actions: "dismiss",
          }
        : null;
  }
}

/**
 * Says a newer Flume exists, and answers a check the person asked for.
 *
 * Sits in the strip under the toolbar where the guard's note goes, built the
 * same way as {@link NoteCard} -- a glyph and a word beside the colour, never
 * colour alone -- with the one or two actions that apply.
 *
 * @param props - See {@link UpdateBannerProps}.
 * @returns The banner, or nothing.
 */
export function UpdateBanner({
  status,
  guardHeld,
  asked,
  dismissed,
  onInstall,
  onDownload,
  onDismiss,
}: UpdateBannerProps) {
  const message = describeUpdate(status, asked, dismissed, guardHeld);
  if (!status || !message) return null;

  return (
    <div
      className="border-line flex flex-wrap items-center justify-between gap-x-4 gap-y-2 border-b px-[18px] py-2.5"
      role="status"
    >
      <div className="flex min-w-0 items-start gap-[9px]">
        <span className={`mt-0.5 shrink-0 ${message.tone}`}>
          <Icon name={message.icon} size={15} />
          <span className="sr-only">{message.label}: </span>
        </span>
        <span className="min-w-0">
          <span className="mb-0.5 block text-xs font-semibold">
            {message.title}
          </span>
          <span className="text-fg-1 block text-[11.5px] leading-[1.5]">
            {message.body}
          </span>
        </span>
      </div>

      {message.actions === "none" ? null : (
        <div className="flex shrink-0 items-center gap-2">
          {message.actions === "install" ? (
            <Button
              variant="primary"
              onClick={onInstall}
              disabled={message.waiting}
            >
              Install and restart
            </Button>
          ) : null}
          {message.actions === "download" ? (
            <Button variant="primary" onClick={onDownload}>
              Open release page
            </Button>
          ) : null}
          <Button variant="ghost" onClick={() => onDismiss(dismissKey(status))}>
            {message.actions === "dismiss" ? "Dismiss" : "Not now"}
          </Button>
        </div>
      )}
    </div>
  );
}
