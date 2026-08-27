"use client";

/** Props for {@link FragmentStrip}. */
export interface FragmentStripProps {
  /** Downsampled completion per bucket, `0..=255`. */
  buckets: number[];
  /** Accessible description of what the strip represents. */
  label: string;
}

/**
 * A compact strip showing which parts of a file are on disk.
 *
 * Deliberately not a canvas, unlike the whole-torrent heatmap. That one draws
 * up to 1600 buckets where sub-pixel gaps make a complete region look striped;
 * a file strip is capped at 400 and sits in a list row, so a CSS gradient is
 * cheaper, scales with the row, and needs no resize observer.
 *
 * The gradient uses hard stops so each bucket is a flat band rather than
 * blending into its neighbour — a blend would imply partial pieces that do not
 * exist.
 *
 * @param props - See {@link FragmentStripProps}.
 * @returns The rendered strip, or `null` when there is nothing to show.
 */
export function FragmentStrip({ buckets, label }: FragmentStripProps) {
  if (buckets.length === 0) return null;

  const step = 100 / buckets.length;
  const stops = buckets
    .map((level, index) => {
      // `currentColor` inherits the accent from the parent, so the strip
      // themes itself without duplicating palette values here.
      const alpha = (level / 255).toFixed(2);
      const from = (index * step).toFixed(3);
      const to = ((index + 1) * step).toFixed(3);
      return `rgb(from currentColor r g b / ${alpha}) ${from}%, rgb(from currentColor r g b / ${alpha}) ${to}%`;
    })
    .join(", ");

  const complete = buckets.filter((b) => b === 255).length;
  const percent = Math.round((complete / buckets.length) * 100);

  return (
    <div
      className="bg-bg-2 text-acc h-1.5 w-full overflow-hidden rounded-full"
      role="img"
      aria-label={`${label}: roughly ${percent}% of this file's pieces downloaded`}
      style={{ backgroundImage: `linear-gradient(to right, ${stops})` }}
    />
  );
}
