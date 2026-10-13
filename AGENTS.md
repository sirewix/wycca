# AGENTS.md

Circular calendar rendered as flat-coloured SVG rings: a wallpaper for linux, and
eventually a browser page and an Android widget.

## Ground rules

- **`wycca-core` is pure.** No I/O, no clock, no filesystem, no platform. One
  function matters: `render(&Config, t_ms) -> String`, where `t_ms` is a **wall
  clock** timestamp, not an instant. Same config plus same timestamp gives the same
  bytes; there is a test for exactly that. Everything else follows from this rule:
  the CLI, the browser and the widget all call the same code.
- **`now_ms()` is the only clock read**, and on wasm32 it is an extern `Date.now`.
  Nothing else may ask the system what time it is.
- **The timezone lives in the config, not the core.** `Config::utc_offset_ms` is the
  seam. The CLI fills it from `Local::now().offset().local_minus_utc()` — *not* the
  difference of two timestamps, which is always zero because they are the same
  instant. The browser will read `Intl.DateTimeFormat().resolvedOptions().timeZone`
  and Android `TimeZone.getDefault()`, and both will hand over one integer.
  Do not add `chrono-tz`.
- **Latitude is config only.** Nothing can discover it. `Config::location`.
- **std, not `no_std`.** `f64::sin` wants `libm`, `format!` wants alloc, and
  wasm32-unknown-unknown has a perfectly good std. The only thing it cannot do is
  read the clock, which we already inject. Revisit only for a microcontroller
  target.
- **SVG is the intermediate format**, not an internal draw list. The browser renders
  SVG natively, the CLI rasterises it with `resvg`, and Android hands the same string
  to `android.graphics.SvgPicture` (public API 21+, no library needed). Adding a
  second renderer means somebody wrote the wrong thing.
- **`Svg::line` exists because a fill-only `M..L` renders as nothing.** Ticks, hands
  and markers are strokes. Use the right one.

## Layout

```
crates/wycca-core/   config, geometry, SVG, solar maths, .ics — all pure
  config.rs   the whole schema; every ring type is a struct with a Default
  rings.rs    one fn per ring type, plus the year/month angle maths
  geom.rs     polar -> SVG path data, arcs split at <=180 degrees
  svg.rs      the writer
  sun.rs      equinoxes/solstices (Meeus) and sunrise/sunset (NOAA)
  events.rs   whole-day events out of an .ics feed
  ffi.rs      C ABI for the browser; compiles natively so it can be tested
crates/wycca-cli/   flags, resvg PNG, YAML, `feh`
```

## Rings

`hour24`, `week`, `month`, `year`, `interval` (the generic "n x something" ring:
sprints, release trains). A new ring is a `RingSpec` variant plus a `match` arm in
`rings::draw`. There is no plugin hook; add one when a second consumer needs to add
rings without touching `rings.rs`.

Radii come from `layout()`: `r = outer_radius * t^(1 + 2*spacing)` with
`t = (i+1)/n`. `spacing` is the single slider; `0` is even, `1` squeezes the inner
rings together. Setting a ring's `radius` pins it and skips the layout.

## Conventions

- Angles are degrees, `0` at 12 o'clock, positive clockwise, except the year and
  month rings, which run counter-clockwise. Convert with `geom::rad`.
- Sizes are fractions of half the canvas, never pixels.
- Colour meanings are fixed: `accent` means **now**, `solar` means **the sun**
  (sunrise, sunset, equinoxes, solstices), `weekend` means **not working**. Nothing
  else may borrow those.
- `deny_unknown_fields` on every config struct: a typo must be a parse error.
- Everything time-of-day on a ring sweeps continuously. `hours_into_day` is the
  helper. Do not reintroduce integer clipping.
- Write comments for *why*, never for what.

## Running it

```sh
nix develop                 # cargo, clippy, rustfmt, feh, a font for resvg
nix develop -c cargo test
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo fmt
nix run . -- -c wycca.yaml --events holidays-kz.ics --png wall.png
wycca -a 2026-10-13T14:30:00+02:00   # deterministic, for eyeballing
wycca -f                              # render a PNG and open it in feh
```

Verify by **probing the rendered PNG**, not by looking at the SVG: sample the pixel
at a given radius and angle and compare with the colour you expect. ASCII-art
downsampling is fine for a quick shape check.

## Local files, gitignored

`wycca.yaml` is the user's config and `holidays-kz.ics` a Google holiday calendar.
Refresh the calendar with:

```sh
curl -o holidays-kz.ics "https://calendar.google.com/calendar/ical/en-gb.kz%23holiday%40group.v.calendar.google.com/public/basic.ics"
```

Config is JSON in the core, YAML in the CLI (`.yaml`/`.yml` by extension). YAML is
CLI-only on purpose: it is the only place a person hand-writes a config, and only
there do comments earn their keep.

## Decisions worth remembering

- **Holidays come from `.ics`, never from a hardcoded country table.** Google's
  per-country calendars (`.../calendar/ical/<cc>#holiday@group.v.calendar.google.com/public/basic.ics`)
  are free, need no key, run to 2031, and model *transferred days off*
  (`Day off for Nauryz`), which the usual APIs do not. Nager.Date was checked and
  rejected for KZ: it returns a byte-identical list for every year, missing
  Orthodox Christmas and Children's Day, with Nauryz and May 1 wrong before 2025.
- **Whole-day events only.** `DTSTART;VALUE=DATE` is the test for "whole day";
  anything with a time is skipped because a ring has no room for one.
  `TRANSP:TRANSPARENT` is the test for "a day off", and a free event draws a full
  crossing line while a busy one draws a short notch.
- **No `RRULE` expansion yet.** The country feeds enumerate every year explicitly.
  Swap the hand-written parser for the `icalendar` crate if someone wants a birthday
  calendar; there is a `ponytail:` note saying so in `events.rs`.
- **No gradients.** SVG has no conic gradient, and support is patchy in `resvg` and
  `android.graphics.SvgPicture`. The day-length-brightness idea on the year ring
  should be ~365 flat day-wedges instead.

## Not built yet

Browser app (wasm build; the target needs `rust-overlay`, nixpkgs has no
`wasm32-unknown-unknown` std), Android widget (`cargo-ndk`, geometry in Rust, draw
in your own process and hand the launcher a `Bitmap`), lunar months (1900–2100
new-moon table; no maintained crate exists), day-length wedges, subnational
holidays, `RRULE`.

## Commits

`--no-gpg-sign`. Never commit `wycca.yaml` or the downloaded calendars.