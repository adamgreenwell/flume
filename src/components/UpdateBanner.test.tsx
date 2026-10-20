import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { UpdateStatus } from "@/lib/ipc/types";

import { UpdateBanner, describeUpdate, dismissKey } from "./UpdateBanner";

const available = (delivery: "inPlace" | "download"): UpdateStatus => ({
  state: "available",
  version: "1.5.1",
  delivery,
});

const failed = (stage: "check" | "install"): UpdateStatus => ({
  state: "failed",
  reason: "network",
  stage,
});

function renderBanner(
  status: UpdateStatus | null,
  { asked = false, dismissed = null as string | null, guardHeld = false } = {},
) {
  const handlers = {
    onInstall: vi.fn(),
    onDownload: vi.fn(),
    onDismiss: vi.fn(),
  };
  const view = render(
    <UpdateBanner
      status={status}
      guardHeld={guardHeld}
      asked={asked}
      dismissed={dismissed}
      {...handlers}
    />,
  );
  return { ...view, ...handlers };
}

describe("describeUpdate", () => {
  it("says nothing before a status arrives, or when idle", () => {
    expect(describeUpdate(null, true, null)).toBeNull();
    expect(describeUpdate({ state: "idle" }, true, null)).toBeNull();
  });

  it("announces an available version whether or not anyone asked", () => {
    expect(describeUpdate(available("inPlace"), false, null)?.title).toBe(
      "Flume 1.5.1 is available",
    );
  });

  it("offers an install only where the format updates in place", () => {
    expect(describeUpdate(available("inPlace"), false, null)?.actions).toBe(
      "install",
    );
    expect(describeUpdate(available("download"), false, null)?.actions).toBe(
      "download",
    );
  });

  it("stays hidden for a version dismissed this session, and only that one", () => {
    expect(
      describeUpdate(available("inPlace"), false, "available:1.5.1"),
    ).toBeNull();
    expect(
      describeUpdate(available("inPlace"), false, "available:1.5.0"),
    ).not.toBeNull();
  });

  // The daily check speaks up only when there is something to install. These
  // are answers to a question, so they need someone to have asked it.
  it.each<UpdateStatus>([
    { state: "checking" },
    { state: "upToDate", checkedAt: 1 },
    { state: "held" },
    { state: "unconfigured" },
  ])("keeps $state quiet unless the person asked", (status) => {
    expect(describeUpdate(status, false, null)).toBeNull();
    expect(describeUpdate(status, true, null)).not.toBeNull();
  });

  // Codex review of #236: a failed daily check on an offline laptop said
  // "Could not update" at every launch.
  it("keeps a failed daily check quiet, but not one someone asked for", () => {
    expect(describeUpdate(failed("check"), false, null)).toBeNull();
    expect(describeUpdate(failed("check"), true, null)?.title).toBe(
      "Could not update",
    );
  });

  it("always reports a failed install -- someone clicked Install", () => {
    expect(describeUpdate(failed("install"), false, null)?.title).toBe(
      "Could not update",
    );
  });

  // Codex review of #236: an install the guard interrupted lost its banner,
  // leaving the pending update unreachable. Rust now keeps it `available`;
  // this is the banner's half -- say why, and do not offer a doomed click.
  it("keeps an update installable through a hold, and says why it waits", () => {
    const message = describeUpdate(available("inPlace"), false, null, true);
    expect(message?.actions).toBe("install");
    expect(message?.waiting).toBe(true);
    expect(message?.body).toMatch(/tunnel guard is holding/);
  });

  it("still opens the release page during a hold, which is not Flume's traffic", () => {
    expect(
      describeUpdate(available("download"), false, null, true)?.waiting,
    ).toBe(false);
  });

  // Codex review of #236: the held message promised a retry that only the
  // daily check performs.
  it("does not promise to check by itself after a hold someone asked about", () => {
    const body = describeUpdate({ state: "held" }, true, null)?.body ?? "";
    expect(body).not.toMatch(/will check/i);
    expect(body).toMatch(/Check for Updates again/);
  });
});

describe("dismissKey", () => {
  it("keys an update by version and a failure by step and reason", () => {
    expect(dismissKey(available("inPlace"))).toBe("available:1.5.1");
    expect(dismissKey(failed("install"))).toBe("failed:install:network");
    expect(dismissKey({ state: "held" })).toBe("held");
  });

  it("lets an install failure be dismissed, though it shows unasked", () => {
    expect(
      describeUpdate(failed("install"), false, "failed:install:network"),
    ).toBeNull();
  });
});

describe("UpdateBanner", () => {
  it("renders nothing when there is nothing to say", () => {
    const { container } = renderBanner({ state: "upToDate", checkedAt: 1 });
    expect(container.innerHTML).toBe("");
  });

  it("installs on a click, and does nothing without one", () => {
    const { onInstall } = renderBanner(available("inPlace"));
    expect(onInstall).not.toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole("button", { name: "Install and restart" }),
    );
    expect(onInstall).toHaveBeenCalledTimes(1);
    expect(
      screen.queryByRole("button", { name: "Open release page" }),
    ).toBeNull();
  });

  it("disables Install while the guard holds", () => {
    const { onInstall } = renderBanner(available("inPlace"), {
      guardHeld: true,
    });
    const install = screen.getByRole("button", {
      name: "Install and restart",
    });
    expect((install as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(install);
    expect(onInstall).not.toHaveBeenCalled();
  });

  it("opens the release page where it cannot install, and never offers to", () => {
    const { onDownload } = renderBanner(available("download"));
    expect(
      screen.queryByRole("button", { name: "Install and restart" }),
    ).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Open release page" }));
    expect(onDownload).toHaveBeenCalledTimes(1);
  });

  it("passes the message's key to Not now, so only that version is hidden", () => {
    const { onDismiss } = renderBanner(available("inPlace"));
    fireEvent.click(screen.getByRole("button", { name: "Not now" }));
    expect(onDismiss).toHaveBeenCalledWith("available:1.5.1");
  });

  it("names the status in words, not colour alone", () => {
    renderBanner(available("inPlace"));
    expect(screen.getByRole("status").textContent).toMatch(/^Update: /);
  });

  it("shows no buttons while installing", () => {
    renderBanner({ state: "installing" });
    expect(screen.queryByRole("button")).toBeNull();
  });
});
