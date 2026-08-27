"use client";

import type { Control } from "@/lib/settings/defs";
import { formatSpeed } from "@/lib/format";

import { Chip } from "./Chip";
import { Icon } from "./Icon";

/**
 * Rate limits the slider offers, in bytes per second.
 *
 * Not a linear range: the difference between 1 and 2 MB/s matters far more
 * than the difference between 40 and 50, so the steps are coarse where nobody
 * is being careful. `null` is the top of the scale rather than a checkbox
 * beside it, because "no limit" is the same decision as "a high limit".
 */
const RATE_STEPS: ReadonlyArray<number | null> = [
  250_000,
  500_000,
  1_000_000,
  2_000_000,
  3_000_000,
  5_000_000,
  8_000_000,
  12_000_000,
  20_000_000,
  35_000_000,
  50_000_000,
  null,
];

/** A toggle switch. */
function Toggle({
  on,
  label,
  onChange,
}: {
  on: boolean;
  label: string;
  onChange: (next: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      onClick={() => onChange(!on)}
      className={`border-line-2 flex h-[21px] w-9 shrink-0 items-center rounded-full border p-0.5 transition-colors ${
        on ? "bg-acc justify-end" : "bg-bg-3 justify-start"
      }`}
    >
      <span
        className={`block h-[17px] w-[17px] rounded-full ${on ? "bg-on-acc" : "bg-fg-2"}`}
      />
    </button>
  );
}

/** Props for {@link SettingControl}. */
export interface SettingControlProps {
  /** Which control to draw. */
  control: Control;
  /** The current value of the setting. */
  value: unknown;
  /** Accessible name — the setting's label. */
  label: string;
  /** Called with the next value. */
  onChange: (next: unknown) => void;
  /** Opens a folder picker, for `path` controls. */
  onBrowse?: () => void;
}

/**
 * The editing control for one setting.
 *
 * Switched on the definition's `control` rather than on the setting's id, so a
 * new setting of an existing kind needs no code here at all — which is the
 * point of generating the screen from a table.
 *
 * @param props - See {@link SettingControlProps}.
 * @returns The rendered control.
 */
export function SettingControl({
  control,
  value,
  label,
  onChange,
  onBrowse,
}: SettingControlProps) {
  switch (control.kind) {
    case "toggle":
      return (
        <Toggle
          on={value === true}
          label={label}
          onChange={(next) => onChange(next)}
        />
      );

    case "segment":
      return (
        <div
          className="border-line flex overflow-hidden rounded-md border"
          role="group"
          aria-label={label}
        >
          {control.options.map((option) => {
            const active = option.value === value;
            return (
              <button
                key={option.value}
                type="button"
                aria-pressed={active}
                onClick={() => onChange(option.value)}
                className={`border-line h-7 border-r px-2.5 text-xs last:border-r-0 ${
                  active
                    ? "bg-acc-deep text-fg-0 font-medium"
                    : "text-fg-2 hover:bg-bg-2 hover:text-fg-0"
                }`}
              >
                {option.label}
              </button>
            );
          })}
        </div>
      );

    case "rate": {
      const current = (value as number | null) ?? null;

      // Nearest step, so a value set from outside this slider still lands
      // somewhere sensible on it rather than snapping to the start.
      let index = RATE_STEPS.length - 1;
      if (current !== null) {
        let best = Number.POSITIVE_INFINITY;
        RATE_STEPS.forEach((step, i) => {
          if (step === null) return;
          const distance = Math.abs(step - current);
          if (distance < best) {
            best = distance;
            index = i;
          }
        });
      }

      const label_ = current === null ? "No limit" : formatSpeed(current);

      return (
        <div className="flex items-center gap-2.5">
          <input
            type="range"
            min={0}
            max={RATE_STEPS.length - 1}
            step={1}
            value={index}
            aria-label={label}
            aria-valuetext={label_}
            onChange={(event) =>
              onChange(RATE_STEPS[Number(event.target.value)] ?? null)
            }
            className="accent-acc w-[150px]"
          />
          <span className="flume-num text-fg-0 w-[76px] shrink-0 text-right text-[11.5px]">
            {label_}
          </span>
        </div>
      );
    }

    case "port": {
      const port = Number(value);
      return (
        <div className="border-line bg-bg-2 flex h-[var(--flume-h-control)] items-center rounded-md border">
          <button
            type="button"
            aria-label={`Decrease ${label}`}
            onClick={() => onChange(Math.max(1024, port - 1))}
            className="text-fg-2 hover:text-fg-0 h-full w-7"
          >
            −
          </button>
          <span className="flume-num text-fg-0 w-[52px] text-center text-[12.5px]">
            {port}
          </span>
          <button
            type="button"
            aria-label={`Increase ${label}`}
            onClick={() => onChange(Math.min(65_535, port + 1))}
            className="text-fg-2 hover:text-fg-0 h-full w-7"
          >
            +
          </button>
        </div>
      );
    }

    case "path":
      return (
        <div className="flex items-center gap-2">
          <span
            className="border-line bg-bg-2 text-fg-1 flex h-[var(--flume-h-control)] max-w-[280px] items-center gap-2 truncate rounded-md border px-2.5 text-[12.5px]"
            title={String(value)}
          >
            <Icon name="folder" size={15} />
            <span className="truncate">{String(value) || "Not set"}</span>
          </span>
          <Chip onClick={onBrowse}>Browse…</Chip>
        </div>
      );

    case "text":
      return (
        <input
          type="text"
          value={(value as string | null) ?? ""}
          aria-label={label}
          placeholder={control.placeholder}
          spellCheck={false}
          onChange={(event) =>
            // Empty means "not set", which is `null` in the settings, not "".
            onChange(
              event.target.value.trim() === "" ? null : event.target.value,
            )
          }
          className="border-line bg-bg-2 text-fg-0 placeholder:text-fg-3 selectable h-[var(--flume-h-control)] w-[280px] rounded-md border px-2.5 font-mono text-[12.5px]"
        />
      );
  }
}
