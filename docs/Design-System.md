# Design System

Flume's interface is designed rather than assembled. This page is the rulebook
for anyone adding UI: what the vocabulary is, where it lives, and which parts
are not open to interpretation.

The single source is `src/app/globals.css`. Everything below is defined there.

## The direction

Quiet precision. Warm-neutral surfaces — never blue-grey. One accent colour
doing all the interactive work. Status colours reserved for status. Every number
in tabular monospace. Generous hairlines instead of heavy borders.

New UI extends this vocabulary. It does not introduce a second one.

## Colour

Tailwind utilities are named after the tokens, one for one: `bg-bg-1`,
`text-fg-2`, `border-line`, `bg-acc-deep`. There are deliberately no friendlier
aliases — two vocabularies is how `--flume-line` and "gray-800" end up meaning
the same thing to different people.

| Role                                    | Token                             | Used for                                                 |
| --------------------------------------- | --------------------------------- | -------------------------------------------------------- |
| Surfaces                                | `bg-0` … `bg-3`                   | ground, cards, inputs, hover                             |
| Lines                                   | `line`, `line-2`                  | row hairlines, control borders                           |
| Ink                                     | `fg-0` … `fg-3`, `fg-dis`         | primary through 10px labels, then disabled               |
| Accent                                  | `acc`, `acc-dim`, `acc-deep`      | the one interactive colour                               |
| Accent ink and hover                    | `on-acc`, `acc-hi`                | label on an accent fill; hover step                      |
| Status                                  | `ok`, `ok-deep`, `warn`, `err`    | verdicts only, never a series colour                     |
| Chart series                            | `chart-down`, `chart-up`          | throughput plots                                         |

**Never introduce a colour that is not a token.** If a step is genuinely
missing, derive it in OKLCH at fixed chroma and hue — the palette was built that
way and converted to sRGB, so interpolating between two existing values in sRGB
gives the wrong answer.

### Themes

Dark is the default. Light is a re-step of the same roles, not an inversion,
which is why both palettes are written out in full rather than one computed from
the other.

The theme swaps at runtime by flipping `data-theme` on `<html>`. `"system"`
removes the attribute entirely so the `prefers-color-scheme` media query stays
authoritative, and the app follows the OS live. The light palette is declared
twice on purpose: an explicit choice has to beat the system preference, and one
combined selector cannot express both without one overriding the other wrongly.
`src/app/tokens.test.ts` asserts the two copies stay in sync.

## Type

Two families, both vendored as woff2 under `src/app/fonts/` — no CDN, no
build-time fetch, and the app renders correctly with no network at all.

- **Instrument Sans** for UI text. Variable weight axis, 400–700.
- **IBM Plex Mono** for every number. Weights 400, 500, 600, latin only.

Base is 13px / 1.45. The rest of the ramp is measured against it, so changing it
moves every screen.

**Every number uses `flume-num`** (mono plus tabular figures). This is not
stylistic — without tabular figures, columns jitter on every 1 Hz tick as digit
widths change.

Sizes and rates are **decimal** — GB, MB/s — because that is what disks and ISPs
quote. Piece length is the only binary figure, rendered MiB, because that is what
the wire format uses.

## Controls

| Token         | Value | Used for                        |
| ------------- | ----- | ------------------------------- |
| `h-chip`      | 28px  | chips, icon buttons             |
| `h-control`   | 30px  | chrome buttons, inputs          |
| `h-primary`   | 34px  | a sheet's primary action        |
| `r-sm`        | 4px   | chips, tags, small controls     |
| `r-md`        | 6px   | buttons, inputs, nav items      |
| `r-lg`        | 9px   | cards, panels                   |

Do not round these to a framework scale. The spacing was chosen against the type
ramp, and snapping it to a 4/8 grid visibly degrades the result.

These are **pointer** targets, not touch targets. A remote web UI would have to
re-scale to a 44px minimum rather than ship desktop sizes to a phone.

## Icons

Stroked SVG on a 16×16 grid, held at a constant 1.5px optical weight — `Icon`
scales `stroke-width` by the grid-to-size ratio, so a 20px glyph sits beside a
14px one at the same weight rather than thickening as it shrinks.

No emoji, no icon font, and nothing filled. A solid glyph beside stroked ones
reads as a different weight class, which is why even pause is drawn as two
strokes.

Three glyphs — `play`, `trash`, `settings` — have no design and are marked
`[undesigned]` in `src/components/Icon.tsx`. Treat them as provisional.

## Accessibility

These were designed in and are easy to break by accident. `src/app/tokens.test.ts`
enforces them.

- **`fg-3` is the floor for text.** `fg-dis` is for disabled controls only and
  must never carry text the user needs to read — the test asserts it stays
  *below* 4.5:1 so nobody "fixes" it into looking enabled.
- **`line-2` clears 3:1**, so control borders and unchecked checkboxes are
  actually visible. Do not lighten it.
- **Status is never colour alone.** Every state carries a dot, a word, and a
  sentence. The pill is only the adjective; the sentence explaining what to do
  belongs to whatever the pill labels.
- **Download and upload are always labelled.** The two chart series separate
  cleanly under normal, protan and deutan vision but converge under tritanopia,
  so colour alone can never carry the distinction.
- **Visible focus on every control.** The `:focus-visible` rule is global.
- **A progress bar always ships with its number.** At 5px tall a 3% fill and a
  0% fill are the same two pixels, so `ProgressBar` renders the percentage
  itself rather than trusting call sites to remember.

### Known contrast gaps

Recorded rather than silently corrected, because closing them would mean putting
a colour in the app that is in no palette. Pinned in `tokens.test.ts` so they
cannot get worse:

| Theme | Pair              | Measured | Stated floor |
| ----- | ----------------- | -------- | ------------ |
| dark  | `fg-3` on `bg-2`  | 4.20:1   | 4.5:1        |
| dark  | `fg-3` on `bg-3`  | 3.73:1   | 4.5:1        |
| dark  | `line-2` on `bg-2`| 2.82:1   | 3:1          |
| dark  | `line-2` on `bg-3`| 2.50:1   | 3:1          |
| light | `line-2` on `bg-3`| 2.88:1   | 3:1          |
| light | `warn` as text    | 3.60:1   | 4.5:1        |
| light | `ok` as text      | 4.34:1   | 4.5:1        |

All of these hold on the ground (`bg-0`) and on cards (`bg-1`), which is where
the design actually places small labels and control borders. They fall short
only on the raised steps. Until they are resolved, do not put a 10px `fg-3`
label or a `line-2` border on `bg-2` or `bg-3`, and do not rely on `warn` or
`ok` as text colour in the light theme.

## Storybook

```bash
npm run storybook
```

Every primitive, in every state, in both themes, with axe running beside it. The
theme toolbar flips `data-theme` exactly as the app does, so what you see is the
real mechanism rather than a Storybook-only wrapper.

`npm run storybook:build` produces a static build. It is deliberately not part
of `npm run check` — stories are already typechecked by `tsc`, and a full static
build on every run costs more than the config churn it would catch.

## Components

`src/components/` holds the shared primitives. Check whether one already exists,
with its states defined, before building a new one.

| Component     | Notes                                                             |
| ------------- | ----------------------------------------------------------------- |
| `Button`      | primary / secondary / ghost / danger; `control` and `dialog` sizes |
| `IconButton`  | 28px, always has an accessible name                                |
| `Icon`        | 16 grid, stroked, constant optical weight                          |
| `ProgressBar` | 5px, colour by state, always with its percentage                   |
| `StatusPill`  | dot plus word; tint reserved for states wanting attention          |
| `StatCard`    | label above, mono value, caption below; `dock` and `strip` sizes   |

`danger` on `Button` has no design and is built from the system's vocabulary.
Treat it as provisional, like the three undesigned icons.
