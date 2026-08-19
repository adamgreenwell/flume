import { renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { CoreStatus } from "@/lib/ipc/types";

import { useCoreStatus } from "./useCoreStatus";

/** A representative status payload, matching the Rust serde shape. */
const SAMPLE: CoreStatus = {
  clientVersion: "Flume 0.1.0",
  listenPort: 42221,
  announcePort: 42221,
  dht: { enabled: true, nodesV4: 42, nodesV6: 7, outstandingRequests: 1 },
  downloadDir: "/Users/test/Downloads",
  uptimeSeconds: 12,
  downloadBps: 1024,
  uploadBps: 512,
  livePeers: 3,
  health: "ready",
};

afterEach(() => {
  clearMocks();
  vi.useRealTimers();
});

describe("useCoreStatus", () => {
  it("returns the status once the first poll resolves", async () => {
    mockIPC((cmd) => {
      if (cmd === "get_core_status") return SAMPLE;
      throw new Error(`unexpected command: ${cmd}`);
    });

    const { result } = renderHook(() => useCoreStatus());
    expect(result.current.isLoading).toBe(true);

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.status).toEqual(SAMPLE);
    expect(result.current.error).toBeNull();
  });

  it("surfaces a structured CommandError message", async () => {
    mockIPC(() => {
      throw {
        kind: "engineNotReady",
        message: "The torrent engine is still starting.",
      };
    });

    const { result } = renderHook(() => useCoreStatus());

    await waitFor(() =>
      expect(result.current.error).toBe(
        "The torrent engine is still starting.",
      ),
    );
    expect(result.current.status).toBeNull();
  });

  it("falls back to a generic message for an unrecognised rejection", async () => {
    mockIPC(() => {
      throw "some opaque failure";
    });

    const { result } = renderHook(() => useCoreStatus());

    await waitFor(() =>
      expect(result.current.error).toBe("Could not reach the torrent engine."),
    );
  });

  it("clears a previous error once a poll succeeds again", async () => {
    let shouldFail = true;
    mockIPC(() => {
      if (shouldFail) throw { kind: "engineNotReady", message: "not yet" };
      return SAMPLE;
    });

    const { result } = renderHook(() => useCoreStatus());
    await waitFor(() => expect(result.current.error).toBe("not yet"));

    shouldFail = false;
    await waitFor(() => expect(result.current.error).toBeNull(), {
      timeout: 3000,
    });
    expect(result.current.status).toEqual(SAMPLE);
  });

  it("stops polling after unmount", async () => {
    const handler = vi.fn(() => SAMPLE);
    mockIPC(handler);

    const { result, unmount } = renderHook(() => useCoreStatus());
    await waitFor(() => expect(result.current.status).not.toBeNull());

    const callsAtUnmount = handler.mock.calls.length;
    unmount();

    await new Promise((resolve) => setTimeout(resolve, 1500));
    expect(handler.mock.calls.length).toBe(callsAtUnmount);
  });
});
