"use client";

import { useEffect, useState, type ReactNode } from "react";

/** True only when the mock build flag is set. Inlined by Next at build time. */
const MOCK_ENABLED = process.env.NEXT_PUBLIC_FLUME_MOCK === "1";

/** Props for {@link DevMockGate}. */
export interface DevMockGateProps {
  /** The application, rendered once the mock (if any) is installed. */
  children: ReactNode;
}

/**
 * Installs the development IPC mock before the app mounts, when enabled.
 *
 * It genuinely *gates*: children are withheld until the mock is in place.
 * Without that, the app's own effects run first, fail to reach a backend that
 * is not there yet, and settle into an error state that a late-arriving mock
 * never clears.
 *
 * `NEXT_PUBLIC_FLUME_MOCK` is inlined at build time, so in a normal build
 * `MOCK_ENABLED` is a literal `false`, children render immediately, and the
 * mock module — imported dynamically — is never pulled into the bundle.
 *
 * @param props - See {@link DevMockGateProps}.
 * @returns The application, plus a badge in mock mode.
 */
export function DevMockGate({ children }: DevMockGateProps) {
  const [ready, setReady] = useState(!MOCK_ENABLED);

  useEffect(() => {
    if (!MOCK_ENABLED) return;
    void import("@/lib/devmock").then((mock) => {
      mock.install();
      setReady(true);
    });
  }, []);

  if (!ready) return null;

  return (
    <>
      {children}
      {MOCK_ENABLED ? (
        <div className="border-warn/40 bg-warn/15 text-warn pointer-events-none fixed bottom-3 left-3 z-[60] rounded-full border px-3 py-1 text-[11px] font-medium">
          Mock data — no torrent engine
        </div>
      ) : null}
    </>
  );
}
