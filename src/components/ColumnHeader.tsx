/**
 * The 28px header above the list.
 *
 * Column widths are repeated from {@link TorrentRow} rather than shared through
 * a constant. They are layout facts of two different elements that happen to
 * agree, and threading them through a shared object would make the row harder
 * to read for a coupling that a single test catches.
 *
 * Marked up as a real `row` of `columnheader`s so the grid a screen reader sees
 * has headers to announce as it moves across a torrent.
 *
 * @returns The rendered header.
 */
export function ColumnHeader() {
  return (
    <div
      role="row"
      className="border-line bg-bg-0 text-fg-3 flex h-7 shrink-0 items-center gap-4 border-b px-[18px] text-[10px] font-semibold tracking-[0.09em] uppercase"
    >
      <span role="columnheader" className="w-[18px]">
        <span className="sr-only">State</span>
      </span>
      <span role="columnheader" className="min-w-0 grow">
        Name
      </span>
      <span role="columnheader" className="w-[180px]">
        Progress
      </span>
      <span role="columnheader" className="w-[86px] text-right">
        Down
      </span>
      <span role="columnheader" className="w-[86px] text-right">
        Up
      </span>
      <span role="columnheader" className="w-[78px] text-right">
        Peers
      </span>
      <span role="columnheader" className="w-[124px] pl-[14px]">
        Swarm health
      </span>
    </div>
  );
}
