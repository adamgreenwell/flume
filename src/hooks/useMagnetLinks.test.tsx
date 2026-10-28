import { renderHook, waitFor } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";

import { OPEN_MAGNET_EVENT, useMagnetLinks } from "./useMagnetLinks";

const LAUNCH = "magnet:?xt=urn:btih:launch";
const LATER = "magnet:?xt=urn:btih:later";

/**
 * Mocks IPC with Tauri's event support, plus Rust's inbox: `held` is handed
 * over on the first `take_pending_links` and never again, as `Inbox::open`
 * does.
 */
function setupIPC(held: string[]) {
  let pending = [...held];
  mockIPC(
    (cmd) => {
      if (cmd === "take_pending_links") {
        const out = pending;
        pending = [];
        return out;
      }
      throw new Error(`unexpected command: ${cmd}`);
    },
    { shouldMockEvents: true },
  );
}

afterEach(() => {
  clearMocks();
});

describe("useMagnetLinks", () => {
  // #241: the link that launched Flume arrived before the page existed and
  // was emitted into nothing. Rust now holds it; the hook takes it.
  it("delivers a link that arrived before the page was listening", async () => {
    setupIPC([LAUNCH]);
    const onMagnet = vi.fn();

    renderHook(() => useMagnetLinks(onMagnet));

    await waitFor(() => expect(onMagnet).toHaveBeenCalledWith(LAUNCH));
    expect(onMagnet).toHaveBeenCalledTimes(1);
  });

  // Asking first would open a gap: from the moment Rust is told the window
  // is listening it emits instead of holding, so the listener must already be
  // there. Tested by what that gap would lose: a link emitted the instant the
  // inbox opens reaches the hook only if it subscribed first.
  it("subscribes before it asks for held links", async () => {
    mockIPC(
      async (cmd) => {
        if (cmd !== "take_pending_links") {
          throw new Error(`unexpected command: ${cmd}`);
        }
        await emit(OPEN_MAGNET_EVENT, LATER);
        return [];
      },
      { shouldMockEvents: true },
    );
    const onMagnet = vi.fn();

    renderHook(() => useMagnetLinks(onMagnet));

    await waitFor(() => expect(onMagnet).toHaveBeenCalledWith(LATER));
  });

  // React runs effects twice in development. Rust hands held links over
  // once, so the first effect's take must not be thrown away with it.
  it("neither loses nor repeats a held link when effects run twice", async () => {
    setupIPC([LAUNCH]);
    const onMagnet = vi.fn();

    // RTL's own option, not a <StrictMode> wrapper: under this setup the
    // wrapper runs effects once, so a test built on it passed without ever
    // exercising the double run -- a mutation dropping the held link
    // survived it. This one runs mount, cleanup, mount, as the app does.
    renderHook(() => useMagnetLinks(onMagnet), { reactStrictMode: true });

    await waitFor(() => expect(onMagnet).toHaveBeenCalledWith(LAUNCH));
    // Let any second take settle before counting.
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(onMagnet.mock.calls.filter(([uri]) => uri === LAUNCH)).toHaveLength(
      1,
    );
  });

  it("still delivers links emitted after it is listening", async () => {
    setupIPC([]);
    const onMagnet = vi.fn();

    renderHook(() => useMagnetLinks(onMagnet));
    // Emitting only reaches a listener that exists, so wait for the take,
    // which comes after the subscription.
    await new Promise((resolve) => setTimeout(resolve, 20));
    await emit(OPEN_MAGNET_EVENT, LATER);

    await waitFor(() => expect(onMagnet).toHaveBeenCalledWith(LATER));
  });
});
