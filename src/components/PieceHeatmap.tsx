"use client";

import { useEffect, useRef } from "react";

import type { PieceMap } from "@/lib/ipc/types";

/** Rendered height of the strip, in CSS pixels. */
const HEIGHT = 40;

/** Props for {@link PieceHeatmap}. */
export interface PieceHeatmapProps {
  /** Downsampled piece completion from the backend. */
  pieces: PieceMap;
}

/**
 * A strip showing which parts of a torrent are on disk.
 *
 * Drawn on a canvas rather than as one element per bucket. With several
 * hundred buckets in a few hundred CSS pixels, sub-pixel rounding leaves
 * hairline gaps between adjacent elements, so a *fully downloaded* region
 * renders as stripes — actively misleading, since stripes are what a partial
 * download should look like. A canvas gives exact pixel control.
 *
 * Intensity is opacity over the accent colour rather than a hue ramp, so it
 * reads in both themes and does not rely on colour discrimination.
 *
 * @param props - See {@link PieceHeatmapProps}.
 * @returns The rendered strip.
 */
export function PieceHeatmap({ pieces }: PieceHeatmapProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const { buckets, totalPieces, piecesPerBucket } = pieces;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || buckets.length === 0) return;

    const draw = () => {
      const context = canvas.getContext("2d");
      if (!context) return;

      // Match the backing store to the device pixel ratio, or the strip is
      // blurry on a HiDPI display.
      const ratio = window.devicePixelRatio || 1;
      const width = canvas.clientWidth;
      canvas.width = Math.max(1, Math.round(width * ratio));
      canvas.height = Math.round(HEIGHT * ratio);
      context.setTransform(ratio, 0, 0, ratio, 0, 0);
      context.clearRect(0, 0, width, HEIGHT);

      // Read the theme's accent at draw time so a theme switch repaints
      // correctly rather than baking in the palette that was active at mount.
      const accent = getComputedStyle(document.documentElement)
        .getPropertyValue("--flume-acc")
        .trim();
      context.fillStyle = accent || "#5ab8ea";

      const step = width / buckets.length;
      buckets.forEach((level, index) => {
        if (level === 0) return;
        context.globalAlpha = level / 255;
        // Round outward so adjacent buckets overlap by a fraction of a pixel
        // instead of leaving a gap between them.
        const start = Math.floor(index * step);
        const end = Math.ceil((index + 1) * step);
        context.fillRect(start, 0, end - start, HEIGHT);
      });
      context.globalAlpha = 1;
    };

    draw();

    // Redraw on resize; the strip is full-width and the panel is resizable.
    const observer = new ResizeObserver(draw);
    observer.observe(canvas);
    return () => observer.disconnect();
  }, [buckets]);

  if (buckets.length === 0) {
    return (
      <p className="text-fg-3 text-xs">
        No piece information yet — this appears once the torrent is running.
      </p>
    );
  }

  const complete = buckets.filter((level) => level === 255).length;
  const percentComplete = Math.round((complete / buckets.length) * 100);

  return (
    <div className="flex flex-col gap-2">
      <canvas
        ref={canvasRef}
        style={{ height: HEIGHT }}
        className="border-line bg-bg-2 w-full rounded-md border"
        role="img"
        aria-label={`Piece map: roughly ${percentComplete}% of ${totalPieces} pieces downloaded`}
      />
      <p className="text-fg-3 text-xs">
        {totalPieces.toLocaleString()} pieces
        {piecesPerBucket > 1
          ? ` · ${piecesPerBucket} per column (downsampled to fit)`
          : ""}
      </p>
    </div>
  );
}
