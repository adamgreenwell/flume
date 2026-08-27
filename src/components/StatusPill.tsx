"use client";

import type { EngineHealth } from "@/lib/ipc/types";

/**
 * Visual treatment and copy for each {@link EngineHealth} value.
 *
 * Only the state that wants the user to look carries a tinted background. The
 * design reserves the tint for "something here needs attention" — giving every
 * pill a fill would flatten that signal into decoration, and a status bar where
 * everything is highlighted highlights nothing.
 */
const HEALTH_PRESENTATION: Record<
  EngineHealth,
  { label: string; tone: string; tint: string }
> = {
  starting: { label: "Starting", tone: "text-fg-3", tint: "" },
  connecting: { label: "Connecting", tone: "text-warn", tint: "" },
  ready: { label: "Ready", tone: "text-ok", tint: "" },
  degraded: { label: "Degraded", tone: "text-warn", tint: "bg-warn/15" },
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
 * Carries a dot and a word, never colour alone. The pill is only the
 * adjective — the sentence explaining what the state means and what to do
 * about it belongs to whatever the pill is labelling, not to the pill.
 *
 * @param props - See {@link StatusPillProps}.
 * @returns The rendered pill.
 */
export function StatusPill({ health, pulse = false }: StatusPillProps) {
  const { label, tone, tint } = HEALTH_PRESENTATION[health];

  return (
    <span
      className={`inline-flex h-[22px] items-center gap-1.5 rounded-sm px-[9px] text-[11px] font-medium whitespace-nowrap ${tone} ${tint}`}
      role="status"
      aria-live="polite"
    >
      <span
        className={`h-1.5 w-1.5 shrink-0 rounded-full bg-current ${pulse ? "animate-pulse" : ""}`}
        aria-hidden="true"
      />
      {label}
    </span>
  );
}
