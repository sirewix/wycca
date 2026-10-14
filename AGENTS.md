# AGENTS.md

Circular calendar rendered as flat-coloured SVG rings: a wallpaper for linux, and
eventually a browser page and an Android widget.

## Ground rules

- **`wycca-core` is pure.** No I/O, no clock, no filesystem, no platform. One
  function matters: `render(&Config, t_ms) -> String`, where `t_ms` is a **wall
  clock** timestamp, not an instant. Same config plus same timestamp gives the same
  bytes; there is a test for exactly that. Everything else follows from this rule:
  the CLI, the browser and the widget all call the same code.
- **`now_ms()` is the only clock read, and it is native-only.** wasm32 has no clock
  we can import without linking an undefined symbol, so every caller passes `t_ms`
  in — the browser sends `Date.now() + utc_offset_ms`, the CLI sends the local wall
  clock. Nothing else may ask the system what time it is.
- **The timezone lives in the config, not the core.** `Config::utc_offset_ms` is the
  seam. The CLI fills it from `Local::now().offset().local_minus_utc()` — *not* the
  difference of two timestamps, which is always zero because they are the same
  instant. The browser will read `Intl.DateTimeFormat().resolvedOptions().timeZone`
  and Android `TimeZone.getDefault()`, and both will hand over one integer.
  Do not add `chrono-tz`.
- **Latitude is config only.** Nothing can discover it. `Config::location`.
- **Longitude 0 means unset, not the Gulf of Guinea.** Sunrise and sunset are
  global-instant maths keyed on longitude, so a config with a latitude but no
  longitude puts the Sun on the prime meridian and the day lands hours off.
  `resolve_location` fills it from `utc_offset_ms × 15°`, the zone's standard
  meridian: nobody keeps a wall clock five hours from their own noon, so a
  non-zero offset with longitude zero is a forgotten setting, not the Atlantic. A
  real longitude always wins; the guess is only good to the zone meridian anyway.
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
- **The config UI is described by the core, never implemented by it.** `ui.rs`
  emits sections, fields and ranges as JSON; the browser and the widget render
  that with their own widgets. Two UIs, one description. The only thing tying them
  together is the dotted key strings, and two tests in `ui.rs` check every one of
  them against `Config`.

## Layout

```
crates/wycca-core/   config, geometry, SVG, solar maths, .ics — all pure
  config.rs   the whole schema; a ring is a scale plus a bag of layer flags
  rings.rs    the layers, plus the one angle helper they all run through
  geom.rs     polar -> SVG path data, arcs split at <=180 degrees
  svg.rs      the writer
  sun.rs      equinoxes/solstices (Meeus) and sunrise/sunset (NOAA)
  events.rs   whole-day events out of an .ics feed
  ui.rs       the config form, as data: sections and fields, no widgets
  ffi.rs      C ABI for the browser; compiles natively so it can be tested
crates/wycca-cli/   flags, resvg PNG, YAML, `feh`
web/          index.html + ui.js, the only hand-written page; build.sh emits
              wycca.wasm.js beside them
```

## Rings

A ring is `Scale` + `Layers`: the scale is the span of time one turn covers
(`hour24`, `week`, `month`, `year` — the calendar year, so the seasons sit
symmetric about the cardinal axes and each season holds its three months), the
layers are flags for what is drawn on it (sectors, ticks, markers, a hand). Every
angle is a fraction of the scale's current period through one helper, so layers
can never disagree about a position and `reversed` is applied once. A new thing to
draw is a new flag in `Layers` plus a block in `rings::draw`; there is no
per-ring-type code. `Ring::new(scale)` is the only place a ring's starting layers
are chosen.

Radii come from `layout()`: `r = outer_radius * t^(1 + 2*spacing)` with
`t = (i+1)/n`. `spacing` is the single slider; `0` is even, `1` squeezes the inner
rings together. Setting a ring's `radius` pins it and skips the layout.

## Conventions

- Angles are degrees, `0` at 12 o'clock, positive clockwise, except the year
  rings, which run counter-clockwise. Convert with `geom::rad`.
- Sizes are fractions of half the canvas, never pixels.
- Colour meanings are fixed: `accent` means **now**, `solar` means **the sun**
  (sunrise, sunset, equinoxes, solstices), `weekend` means **not working**, and
  `day`/`night` are the lit and dark halves of the hour ring — each one is the
  background colour on one theme and the opposite on the other, so the dark half
  of the clock reads as dark on either. Nothing else may borrow those.
  Every event is `weekend` red, free or busy; the mark's length is what says which.
- `deny_unknown_fields` on every config struct: a typo must be a parse error.
- Everything time-of-day on a ring sweeps continuously. Angles are fractions of
  the scale's period, never integer-clipped.
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

The browser page is a file, not a server. Open it directly:

```sh
nix develop -c web/build.sh   # cargo -> base64 -> web/wycca.wasm.js
xdg-open web/index.html       # or file://... in any browser, offline
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
- **The wasm is fetched, with a base64 copy for `file://`.** `build.sh` emits both
  `wycca.wasm` and `wycca.wasm.js`; the page tries `fetch` and falls back to the
  inline one, so there is no build flag and deploying is copying the directory.
  The base64 copy is also the whole reason there is no bundler, no node and no
  wasm-bindgen, and the whole reason the timestamp is a parameter: an imported
  `Date.now` is an undefined symbol `wasm-ld` will not link.
- **`reversed` goes through `turn()`, never into the geometry.** Every angle in a
  ring is worked out the ring's natural way round and passed through one helper,
  so flipping a ring cannot be half-applied. A test renders each reversible ring
  both ways and compares.
- **`<input type="color">`, not a colourpicker from a CDN.** The page has to work
  offline from a file, and the widget has no DOM at all; the core already reads
  `#rrggbb`, so nothing needs to understand a colour space.
- **The timezone picker is platform glue, not part of the form.** Resolving
  `Europe/Almaty` to an offset needs a tz database and `chrono-tz` is banned, so
  `ui.js` reads `Intl.supportedValuesOf("timeZone")` and recomputes the offset each
  frame (which is also how daylight saving follows). Android will do the same with
  `TimeZone`. Fifteen lines per platform beats the dependency.
- **A new ring arrives through `wycca_ring`, not as a half-written object.** The
  form says which controls a ring has, not what they start at, so the UI asks
  the core for a `Ring::new(scale)` and never invents a value. The same goes for
  `.ics`: `wycca_events` runs the parser the CLI runs, so the page and
  `wycca --events` can never disagree about what a holiday is.

## Not built yet

Android widget (`cargo-ndk`, geometry in Rust, draw in your own process and hand
the launcher a `Bitmap`), exporting a config file, lunar months (1900–2100
new-moon table; no maintained crate exists), day-length wedges, subnational
holidays, `RRULE`, interval rings (a fixed-length scale for sprints and release
trains; every layer works on one, only the scale is missing).

## Commits

`--no-gpg-sign`. Never commit `wycca.yaml` or the downloaded calendars.