import type { SwarmHealth } from "@/lib/ipc/types";

/**
 * Word and colour per verdict.
 *
 * Only the verdicts that mean "something is wrong" carry a tint. Giving every
 * chip a fill would turn the column into stripes and cost the tint the one job
 * it has, which is to be noticed.
 *
 * `unknown` is deliberately quiet. It is not a warning — it means there were no
 * peer bitfields to judge from, and dressing that up as either verdict would be
 * a confident wrong answer.
 *
 * `thin` is the one that earns a tint: the swarm holds every piece, but losing
 * a peer could strand the download. `healthy` says the same structural thing
 * with room to spare, so it stays quiet.
 */
const PRESENTATION: Record<
  SwarmHealth,
  { label: string; tone: string; tint: string }
> = {
  seeding: { label: "Seeding", tone: "text-ok", tint: "" },
  none: { label: "No seeds", tone: "text-err", tint: "bg-err/15" },
  idle: { label: "Idle", tone: "text-fg-3", tint: "" },
  unknown: { label: "Connected", tone: "text-fg-2", tint: "" },
  healthy: { label: "Healthy", tone: "text-ok", tint: "" },
  thin: { label: "Thin swarm", tone: "text-warn", tint: "bg-warn/15" },
};

/** Props for {@link HealthChip}. */
export interface HealthChipProps {
  /** The verdict to render. */
  health: SwarmHealth;
  /**
   * Sentence describing what the verdict means for this torrent.
   *
   * Not rendered here — the row's meta line carries it. Taken as a prop so it
   * can become the chip's accessible description, which is the only way a
   * screen reader user gets the same "word plus reason" pairing a sighted user
   * gets from reading across the row.
   */
  detail: string;
}

/**
 * The swarm-health verdict in a torrent row.
 *
 * A dot, a word, never colour alone. The chip is only the adjective; the
 * sentence explaining it belongs to the row.
 *
 * @param props - See {@link HealthChipProps}.
 * @returns The rendered chip.
 */
export function HealthChip({ health, detail }: HealthChipProps) {
  const { label, tone, tint } = PRESENTATION[health];

  return (
    <span
      className={`inline-flex h-[21px] items-center gap-1.5 rounded-sm pr-2 pl-[7px] text-[11px] font-medium whitespace-nowrap ${tone} ${tint}`}
      title={detail}
    >
      <span
        className="h-1.5 w-1.5 shrink-0 rounded-full bg-current"
        aria-hidden="true"
      />
      {label}
    </span>
  );
}
