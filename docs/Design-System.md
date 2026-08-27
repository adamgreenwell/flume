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

| Role                 | Token                          | Used for                                   |
| -------------------- | ------------------------------ | ------------------------------------------ |
| Surfaces             | `bg-0` … `bg-3`                | ground, cards, inputs, hover               |
| Lines                | `line`, `line-2`               | row hairlines, control borders             |
| Ink                  | `fg-0` … `fg-3`, `fg-dis`      | primary through 10px labels, then disabled |
| Accent               | `acc`, `acc-dim`, `acc-deep`   | the one interactive colour                 |
| Accent ink and hover | `on-acc`, `acc-hi`             | label on an accent fill; hover step        |
| Status               | `ok`, `ok-deep`, `warn`, `err` | verdicts only, never a series colour       |
| Chart series         | `chart-down`, `chart-up`       | throughput plots                           |

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

| Token       | Value | Used for                    |
| ----------- | ----- | --------------------------- |
| `h-chip`    | 28px  | chips, icon buttons         |
| `h-control` | 30px  | chrome buttons, inputs      |
| `h-primary` | 34px  | a sheet's primary action    |
| `r-sm`      | 4px   | chips, tags, small controls |
| `r-md`      | 6px   | buttons, inputs, nav items  |
| `r-lg`      | 9px   | cards, panels               |

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
  _below_ 4.5:1 so nobody "fixes" it into looking enabled.
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

| Theme | Pair               | Measured | Stated floor |
| ----- | ------------------ | -------- | ------------ |
| dark  | `fg-3` on `bg-2`   | 4.20:1   | 4.5:1        |
| dark  | `fg-3` on `bg-3`   | 3.73:1   | 4.5:1        |
| dark  | `line-2` on `bg-2` | 2.82:1   | 3:1          |
| dark  | `line-2` on `bg-3` | 2.50:1   | 3:1          |
| light | `line-2` on `bg-3` | 2.88:1   | 3:1          |
| light | `warn` as text     | 3.60:1   | 4.5:1        |
| light | `ok` as text       | 4.34:1   | 4.5:1        |

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

## The library window

The main screen is a two-column, two-row grid: a `248px` rail beside `1fr`,
under a `44px` title bar. The main column stacks a `56px` toolbar, a `28px`
column header, the scrolling list, and a `116px` dock.

Row columns, in order and at these exact widths:

| Column       | Width |
| ------------ | ----- |
| State        | `18`  |
| Name         | `1fr` |
| Progress     | `180` |
| Down         | `86`  |
| Up           | `86`  |
| Peers        | `78`  |
| Swarm health | `124` |

Rows are `58px` comfortable, `40px` compact, `0 18px` padding, `16px` gap.
Density is a `data-density` attribute on `<html>`, so one attribute re-lays the
whole list rather than every row branching on a prop. Compact removes the meta
line rather than shrinking it — at 40px there is no room, and a squeezed
sentence is the first thing to become unreadable.

### The one per-platform difference

The title bar reserves `88px` at the **left** on macOS for the traffic lights,
and `138px` at the **right** on Windows and Linux. That inset is the only thing
in the entire app that differs between platforms; everything inside the window
is identical on all three.

### The expanded row

Clicking a row opens a panel beneath it, indented past the status column so it
reads as belonging to that row rather than floating between two. It holds five
stats, three actions, the piece strip, the top contributors, and the note.

**The note is the point.** Every other thing in a row is a number; the note says
what the numbers mean and what to do about them. It is derived in Rust
(`src-tauri/src/engine/note.rs`) because the engine is the only thing that knows
why a torrent is in the state it is in — a frontend rebuilding that reasoning
from summary fields would drift from it. Severity is `ok` / `warn` / `err` /
`neutral`, and `neutral` is a claim rather than an absence: a paused torrent
needs to say that nothing is wrong, loudly enough that the user does not think
something broke.

Detail is **polled** while a row is open, not pushed. It is per-torrent and
several times the size of a summary; broadcasting it for every torrent every
second so one expanded row can read it would be the wrong trade. At most one
row is open, so this is one extra call per second in total, and none at all
when the list is collapsed.

**The piece strip** answers "which parts do I have", which overall progress
cannot: 60% with a solid head and an empty tail is a torrent downloading in
order, and 60% scattered evenly is one pulling rarest-first — they behave
differently when the swarm thins. The engine downsamples to up to 1600 buckets
for the inspector's full-width map; the row's 96-cell strip averages those down
rather than asking for a second resolution.

**Top contributors** are ranked by bytes that passed verification, not by
connection order. The design shows an instantaneous per-peer rate; librqbit's
per-peer counters are cumulative totals with no rate among them, so the column
shows the total each peer has supplied — arguably the better answer to "who is
contributing", since it does not swing with the tick.

### The add sheet

Adding a torrent is a review you read, not a modal you dismiss. That only works
if the review has something to say, so the sheet answers the questions worth
asking before tens of gigabytes start arriving.

**Four pre-flight tiles.** Three recalculate as files are toggled — bytes
selected, the volume afterwards, and the estimated finish. The fourth reports
how many peers answered while the file list was fetched, and does not change;
re-measuring it on every checkbox click would be both wrong and pointless.

Every tile states its basis in the line underneath. A number whose derivation
is invisible is a number the user has to take on trust, which is the thing a
review sheet exists to avoid.

**Already on disk.** Files present at the right name and length are detected,
tagged, and deselected by default, with the footer saying why. Re-select one
and the footer switches to naming the bytes it would fetch again. Length is
checked, not content — hashing 46 GB to answer a question asked before the
download starts would take longer than the download, and every piece is
verified on arrival anyway.

**Tri-state folders.** `partial` is what makes folder checkboxes usable:
without a third mark, a folder with one file deselected looks identical to one
with everything deselected. It is exposed as `aria-checked="mixed"`, the real
ARIA value. Clicking a partly-selected folder _completes_ it rather than
clearing — the other reading throws away the selection the user just built.

Files sort folders-first then numerically, so "part2" precedes "part10".
Torrents are full of numbered parts, and lexicographic order gets every one of
them wrong.

Two things the design specifies that Flume reports differently, because
librqbit does not expose the underlying data: the swarm tile shows peers seen
rather than a seeds/leechers scrape, and the finish estimate uses this
session's average rather than a persisted seven-day one. Both say which they
are using.

### The dock chart

Sixty samples at 1 Hz, two series on **one shared scale** — a chart that gave
upload its own axis would draw a trickle and a torrent at the same height.

The history lives in `useThroughputHistory`, not in the engine. It is
presentation state: the chart is the only thing that wants it, and pushing
sixty samples across IPC every tick to redraw a chart that already holds
fifty-nine of them is per-tick waste for nothing. Samples are keyed on the
session's uptime, because two consecutive ticks can carry byte-identical rates
and React can legitimately re-run an effect with the same snapshot.

The ceiling is the configured rate limit when there is one — the useful
question then is how close you are to it, which a rescaling axis cannot answer.
Otherwise it is 1, 2 or 5 times a power of ten above the busiest sample, with a
floor of 1 MB/s so an idle session does not magnify background chatter into
dramatic peaks.

Segments are smooth-stepped, flat out of one sample and flat into the next. A
straight line between two readings claims the rate moved evenly between them,
which is a measurement nobody took.

A partial window is right-aligned: ten seconds of history draws ten seconds of
line at the right edge rather than stretching across the full width and
implying a minute it does not have.

### Swarm health

The column reports a verdict, not a peer count. What it can say today:

| Verdict   | Means                                    | Shown as  |
| --------- | ---------------------------------------- | --------- |
| `seeding` | Complete and serving                     | Seeding   |
| `none`    | No reachable peer holds the remainder    | No seeds  |
| `idle`    | Paused, checking or errored — not trying | Idle      |
| `unknown` | Connected, but coverage is unknowable    | Connected |

`unknown` is not a plumbing gap. Telling a thin swarm from a healthy one needs
piece availability, which librqbit 9.0.0 does not expose — see
[issue #79](https://github.com/adamgreenwell/flume/issues/79). Do not derive a
swarm verdict from peer counts and present it as an availability judgement.

## Components

`src/components/` holds the shared primitives. Check whether one already exists,
with its states defined, before building a new one.

| Component     | Notes                                                              |
| ------------- | ------------------------------------------------------------------ |
| `Button`      | primary / secondary / ghost / danger; `control` and `dialog` sizes |
| `IconButton`  | 28px, always has an accessible name                                |
| `Icon`        | 16 grid, stroked, constant optical weight                          |
| `ProgressBar` | 5px, colour by state, always with its percentage                   |
| `StatusPill`  | dot plus word; tint reserved for states wanting attention          |
| `StatCard`    | label above, mono value, caption below; `dock` and `strip` sizes   |

`danger` on `Button` has no design and is built from the system's vocabulary.
Treat it as provisional, like the three undesigned icons.
