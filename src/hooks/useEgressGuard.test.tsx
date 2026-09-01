import { renderHook, waitFor } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";

import type { GuardStatus } from "@/lib/ipc/types";

import { EGRESS_EVENT, useEgressGuard } from "./useEgressGuard";

const status = (over: Partial<GuardStatus> = {}): GuardStatus => ({
  guard: "hold",
  report: {
    path: { v4: { interface: "en7", kind: "ordinary" }, v6: null },
    verdict: { verdict: "direct", interface: "en7" },
  },
  held: true,
  resumesInSeconds: null,
  ...over,
});

/**
 * Installs an IPC mock with Tauri's built-in event support.
 *
 * @param onCheck - What `check_egress` should do.
 */
function setupIPC(onCheck: () => unknown) {
  mockIPC(
    (cmd) => {
      if (cmd === "check_egress") return onCheck();
      throw new Error(`unexpected command: ${cmd}`);
    },
    { shouldMockEvents: true },
  );
}

afterEach(() => {
  clearMocks();
});

describe("useEgressGuard", () => {
  it("shows the initial fetch before any event arrives", async () => {
    // The backend publishes a status during startup, so the first paint has
    // something real rather than a guess.
    setupIPC(() => status({ held: false, guard: "off" }));

    const { result } = renderHook(() => useEgressGuard());

    await waitFor(() => expect(result.current.status).not.toBeNull());
    expect(result.current.held).toBe(false);
    expect(result.current.status?.guard).toBe("off");
  });

  it("replaces the initial fetch with pushed updates", async () => {
    setupIPC(() => status({ held: false }));

    const { result } = renderHook(() => useEgressGuard());
    await waitFor(() => expect(result.current.status).not.toBeNull());

    await emit(EGRESS_EVENT, status({ held: true, resumesInSeconds: null }));

    await waitFor(() => expect(result.current.held).toBe(true));
  });

  it("does not let a slow initial fetch clobber a newer event", async () => {
    // The fetch and the subscription race on every mount. Losing that race
    // must not roll the UI back to a status from before the event.
    let release: (value: GuardStatus) => void = () => {};
    const pending = new Promise<GuardStatus>((resolve) => {
      release = resolve;
    });
    setupIPC(() => pending);

    const { result } = renderHook(() => useEgressGuard());

    await emit(EGRESS_EVENT, status({ held: true }));
    await waitFor(() => expect(result.current.held).toBe(true));

    // The stale fetch lands afterwards and must be discarded.
    release(status({ held: false, guard: "off" }));
    await new Promise((resolve) => setTimeout(resolve, 10));

    expect(result.current.held).toBe(true);
    expect(result.current.status?.guard).toBe("hold");
  });

  it("reports not-held while the status is still unknown", async () => {
    // Announcing a hold that is not happening would be worse than being a
    // second late to announce one that is.
    setupIPC(() => {
      throw { kind: "engineNotReady", message: "starting" };
    });

    const { result } = renderHook(() => useEgressGuard());

    expect(result.current.held).toBe(false);
    expect(result.current.status).toBeNull();
  });

  it("survives a failed initial fetch and still takes events", async () => {
    setupIPC(() => {
      throw new Error("no");
    });

    const { result } = renderHook(() => useEgressGuard());

    await emit(EGRESS_EVENT, status({ held: true }));
    await waitFor(() => expect(result.current.held).toBe(true));
  });

  it("carries the settle countdown so the wait can be explained", async () => {
    setupIPC(() => status());

    const { result } = renderHook(() => useEgressGuard());
    await emit(EGRESS_EVENT, status({ held: true, resumesInSeconds: 6 }));

    await waitFor(() =>
      expect(result.current.status?.resumesInSeconds).toBe(6),
    );
  });
});
