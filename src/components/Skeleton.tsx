/** Props for {@link Skeleton}. */
export interface SkeletonProps {
  /** How many placeholder rows to draw. */
  rows?: number;
  /** Accessible description of what is loading. */
  label: string;
}

/**
 * A placeholder for content that is still loading.
 *
 * Preferred over a spinner or bare "Loading…" text because it preserves the
 * shape of what is arriving, so the panel does not visibly reflow when real
 * content replaces it.
 *
 * Rows vary in width so the block reads as *content* rather than as a loading
 * bar, and the pulse is suppressed under `prefers-reduced-motion` by the
 * global rule in `globals.css`.
 *
 * @param props - See {@link SkeletonProps}.
 * @returns The rendered placeholder.
 */
export function Skeleton({ rows = 3, label }: SkeletonProps) {
  // Deterministic widths: a random pattern would reshuffle on every render.
  const widths = ["82%", "64%", "91%", "73%", "58%"];

  return (
    <div
      className="flex flex-col gap-2.5"
      role="status"
      aria-label={label}
      aria-busy="true"
    >
      {Array.from({ length: rows }, (_, index) => (
        <div
          key={index}
          className="bg-bg-2 h-9 animate-pulse rounded-md"
          style={{ width: widths[index % widths.length] }}
        />
      ))}
      <span className="sr-only">{label}</span>
    </div>
  );
}
