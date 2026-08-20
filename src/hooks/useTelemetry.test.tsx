import { renderHook, waitFor } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";

import type { CoreStatus, TelemetrySnapshot } from "@/lib/ipc/types";

import { TELEMETRY_EVENT, useTelemetry } from "./useTelemetry";

const CORE: CoreStatus = {
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

const snapshot = (uptimeSeconds: number): TelemetrySnapshot => ({
  core: { ...CORE, uptimeSeconds },
  torrents: [],
});

/**
 * Installs an IPC mock with Tauri's built-in event support.
 *
 * `shouldMockEvents` makes the mock handle `plugin:event|listen`/`emit`
 * internally, so tests can deliver events with the real `emit()` rather than
 * reaching into Tauri's callback registry.
 *
 * @param onTelemetryCall - What `get_telemetry` should do.
 */
function setupIPC(onTelemetryCall: () => unknown) {
  mockIPC(
    (cmd) => {
      if (cmd === "get_telemetry") return onTelemetryCall();
      throw new Error(`unexpected command: ${cmd}`);
    },
    { shouldMockEvents: true },
  );
}

/** `get_telemetry` behaviour matching a still-starting engine. */
const engineNotReady = () => {
  throw { kind: "engineNotReady", message: "starting" };
};

afterEach(clearMocks);

describe("useTelemetry", () => {
  it("renders a pushed snapshot", async () => {
    setupIPC(engineNotReady);
    const { result } = renderHook(() => useTelemetry());

    await waitFor(() => expect(result.current.error).toBe("starting"));
    await emit(TELEMETRY_EVENT, snapshot(5));

    await waitFor(() =>
      expect(result.current.telemetry?.core.uptimeSeconds).toBe(5),
    );
    expect(result.current.isLoading).toBe(false);
  });

  it("clears a startup error once telemetry starts flowing", async () => {
    setupIPC(engineNotReady);
    const { result } = renderHook(() => useTelemetry());

    await waitFor(() => expect(result.current.error).toBe("starting"));

    await emit(TELEMETRY_EVENT, snapshot(1));
    await waitFor(() => expect(result.current.error).toBeNull());
  });

  it("primes the first paint from get_telemetry before any event arrives", async () => {
    setupIPC(() => snapshot(7));
    const { result } = renderHook(() => useTelemetry());

    await waitFor(() =>
      expect(result.current.telemetry?.core.uptimeSeconds).toBe(7),
    );
    expect(result.current.error).toBeNull();
  });

  it("falls back to a generic message for an unrecognised rejection", async () => {
    setupIPC(() => {
      throw "opaque";
    });
    const { result } = renderHook(() => useTelemetry());

    await waitFor(() =>
      expect(result.current.error).toBe("Could not reach the torrent engine."),
    );
  });

  it("does not let a slow initial fetch overwrite a newer pushed event", async () => {
    let resolveInitial: (v: TelemetrySnapshot) => void = () => {};
    const pending = new Promise<TelemetrySnapshot>((resolve) => {
      resolveInitial = resolve;
    });
    setupIPC(() => pending);

    const { result } = renderHook(() => useTelemetry());

    // A live event lands while the initial fetch is still in flight.
    await emit(TELEMETRY_EVENT, snapshot(99));
    await waitFor(() =>
      expect(result.current.telemetry?.core.uptimeSeconds).toBe(99),
    );

    // The stale fetch resolves afterwards and must be discarded.
    resolveInitial(snapshot(1));
    await new Promise((r) => setTimeout(r, 50));

    expect(result.current.telemetry?.core.uptimeSeconds).toBe(99);
  });

  it("stops applying events after unmount", async () => {
    setupIPC(engineNotReady);
    const { result, unmount } = renderHook(() => useTelemetry());

    await emit(TELEMETRY_EVENT, snapshot(3));
    await waitFor(() =>
      expect(result.current.telemetry?.core.uptimeSeconds).toBe(3),
    );

    unmount();
    await emit(TELEMETRY_EVENT, snapshot(4));
    await new Promise((r) => setTimeout(r, 50));

    expect(result.current.telemetry?.core.uptimeSeconds).toBe(3);
  });
});

describe("useTelemetry outside the Tauri webview", () => {
  it("reports unavailability instead of raising an unhandled rejection", async () => {
    // Simulates a plain browser: `listen` has no IPC internals to hook into,
    // so it rejects rather than resolving to an unlisten function.
    mockIPC(() => {
      throw new Error("no IPC");
    });
    const internals = (window as unknown as Record<string, unknown>)
      .__TAURI_INTERNALS__;
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;

    try {
      const { result } = renderHook(() => useTelemetry());
      await waitFor(() => expect(result.current.error).not.toBeNull());
      expect(result.current.isLoading).toBe(false);
    } finally {
      (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ =
        internals;
    }
  });
});
