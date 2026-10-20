import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { UpdateStatus } from "@/lib/ipc/types";

import { UpdateBanner, describeUpdate } from "./UpdateBanner";

const available = (delivery: "inPlace" | "download"): UpdateStatus => ({
  state: "available",
  version: "1.5.1",
  delivery,
});

function renderBanner(
  status: UpdateStatus | null,
  { asked = false, dismissed = null as string | null } = {},
) {
  const handlers = {
    onInstall: vi.fn(),
    onDownload: vi.fn(),
    onDismiss: vi.fn(),
  };
  const view = render(
    <UpdateBanner
      status={status}
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
    expect(describeUpdate(available("inPlace"), false, "1.5.1")).toBeNull();
    expect(describeUpdate(available("inPlace"), false, "1.5.0")).not.toBeNull();
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

  it("always reports a failure, asked or not", () => {
    const failed: UpdateStatus = { state: "failed", reason: "signature" };
    const message = describeUpdate(failed, false, null);
    expect(message?.body).toMatch(/nothing was installed/);
  });

  it("explains a held guard instead of failing silently", () => {
    expect(describeUpdate({ state: "held" }, true, null)?.body).toMatch(
      /no connections from the address the guard protects/,
    );
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

  it("opens the release page where it cannot install, and never offers to", () => {
    const { onDownload } = renderBanner(available("download"));
    expect(
      screen.queryByRole("button", { name: "Install and restart" }),
    ).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Open release page" }));
    expect(onDownload).toHaveBeenCalledTimes(1);
  });

  it("passes the version to Not now, so only that version is hidden", () => {
    const { onDismiss } = renderBanner(available("inPlace"));
    fireEvent.click(screen.getByRole("button", { name: "Not now" }));
    expect(onDismiss).toHaveBeenCalledWith("1.5.1");
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
