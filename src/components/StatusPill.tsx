"use client";

import type { EngineHealth } from "@/lib/ipc/types";

/** Visual treatment and copy for each {@link EngineHealth} value. */
const HEALTH_PRESENTATION: Record<
  EngineHealth,
  { label: string; dot: string; text: string }
> = {
  starting: { label: "Starting", dot: "bg-faint", text: "text-muted" },
  connecting: { label: "Connecting", dot: "bg-warn", text: "text-warn" },
  ready: { label: "Ready", dot: "bg-ok", text: "text-ok" },
  degraded: { label: "Degraded", dot: "bg-warn", text: "text-warn" },
};

/** Props for {@link StatusPill}. */
export interface StatusPillProps {
  /** Engine health to display. */
  health: EngineHealth;
  /** Whether to animate the indicator dot, used while work is in progress. */
  pulse?: boolean;
}

/**
 * A compact status indicator showing overall engine readiness.
 *
 * Uses a text label alongside the colour dot rather than colour alone, so the
 * state is legible to colour-blind users.
 *
 * @param props - See {@link StatusPillProps}.
 * @returns The rendered pill.
 */
export function StatusPill({ health, pulse = false }: StatusPillProps) {
  const { label, dot, text } = HEALTH_PRESENTATION[health];

  return (
    <span
      className="border-border-subtle bg-surface-raised inline-flex items-center gap-2 rounded-full border px-3 py-1"
      role="status"
      aria-live="polite"
    >
      <span
        className={`h-2 w-2 shrink-0 rounded-full ${dot} ${pulse ? "animate-pulse" : ""}`}
        aria-hidden="true"
      />
      <span className={`text-xs font-medium tracking-wide ${text}`}>{label}</span>
    </span>
  );
}
