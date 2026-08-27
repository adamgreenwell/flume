import type { Note, NoteSeverity } from "@/lib/ipc/types";

import { Icon, type IconName } from "./Icon";

/**
 * Glyph and colour per severity.
 *
 * The glyph is not decoration — it is the second channel. Status is never
 * carried by colour alone, and a card whose only difference from the one above
 * it is a hue is a card half the users cannot triage.
 */
const PRESENTATION: Record<
  NoteSeverity,
  { icon: IconName; tone: string; label: string }
> = {
  ok: { icon: "check-circle", tone: "text-ok", label: "Fine" },
  warn: { icon: "alert-triangle", tone: "text-warn", label: "Worth knowing" },
  err: { icon: "alert-circle", tone: "text-err", label: "Needs attention" },
  neutral: { icon: "clock", tone: "text-fg-2", label: "Idle" },
};

/** Props for {@link NoteCard}. */
export interface NoteCardProps {
  /** The note to render. */
  note: Note;
}

/**
 * What a torrent is actually doing, in words.
 *
 * The reason the expanded row exists. Everything else in the row is a number;
 * this is the part that says what the numbers mean and what to do about them.
 *
 * The text is derived in Rust rather than assembled here — the engine is the
 * only thing that knows why a torrent is in the state it is in, and a frontend
 * rebuilding that reasoning from summary fields would drift from it.
 *
 * @param props - See {@link NoteCardProps}.
 * @returns The rendered card.
 */
export function NoteCard({ note }: NoteCardProps) {
  const { icon, tone, label } = PRESENTATION[note.severity];

  return (
    <div className="border-line bg-bg-2 flex max-w-[420px] items-start gap-[9px] rounded-sm border px-[11px] py-[9px]">
      <span className={`mt-0.5 shrink-0 ${tone}`}>
        <Icon name={icon} size={15} />
        <span className="sr-only">{label}: </span>
      </span>
      <span>
        <span className="mb-0.5 block text-xs font-semibold">{note.title}</span>
        <span className="text-fg-1 block text-[11.5px] leading-[1.5]">
          {note.body}
        </span>
      </span>
    </div>
  );
}
