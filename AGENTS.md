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
  config.rs   the whole schema; a ring is a scale plus layers, a layer a source
              plus a display
  rings.rs    span generation and the two primitives, through one angle helper
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

A ring is `Scale` + `Vec<Layer>`. The scale is the span of time one turn covers
(`hour24`, `week`, `month`, `year` — the calendar year, so the seasons sit
symmetric about the cardinal axes and each season holds its three months). A
layer is the only other thing there is, and it is two things: **where its spans
come from** and **how to draw them**.

```yaml
- kind: periodic        # solar | ics | now
  unit: day             # year month week day hour minute
  amount: 2             # how long a span is, in units; 0 is an instant
  every: 7              # repeat every n units
  shift: sat            # the phase: a weekday for days, a month for months, a number for hours
  disp:                 # the drawing, the same for every kind
    background: weekend # a palette slot name or #rrggbb; null for no fill
    border: { kind: line, color: accent, width: 0.004, inner: 0.1 }
    label: { at: in, anchor: middle, color: foreground, size: 0.8 }
    offsets: { inner: 0, outer: 0 }   # trims, in per cent of the ring's thickness; null fills it
    gap: 0.6            # degrees trimmed off each end, so neighbours read apart
```

That is the whole vocabulary. A tick, a rule between two months, a weekend, a
season, a holiday and the clock hand are all the same layer with different
numbers, which is why there are no flags: `Layers` and its twenty-two booleans
are gone, and so is the `draw` function that switched on them.

Three things the model leans on, all in `rings.rs`:

- **A span is an instant plus a length, and the arc is `start + length`.** Never
  a start and an end to clip to the period, so a season that began last December
  or a night that runs past midnight is one arc that comes back round the turn.
  For this reason `ring_angle` does not clamp and does not fold into 0..360.
- **Each unit counts from its own epoch**, chosen so index zero is the phase
  zero: midnight, midnight on a Monday, January 1970. A Saturday is a Saturday
  on a month ring and a year ring alike, which is what makes `shift` mean the
  same thing on every ring, and why `shift: sat` and `shift: 5` are the same
  layer — the name is which number a person would have said. Months and years are
  walked with `chrono`'s calendar arithmetic, so February is short and leap years
  are long with no special case.
- **A span at least as long as one turn is not drawn.** A ring is a circle; a
  three-month season on a month ring is not a shape. `periodic` drops it.

A label sits at one end of its span or the other or in the middle (`anchor`), and
across the ring or on the fill or past it (`at`). A border's `offsets` are
**trims**: `inner` off the ring's inner edge, `outer` off the outer one, both as
a percentage of the ring's thickness, so `0, 0` is the whole ring and `-10, -10`
stands out past both edges. A mark that stops at the ring's own edge is a dot on a
thin ring, which is the one thing a mark must never be.

`Ring::new(scale)` and `Layer::new(kind)` are the only places a ring's or a
layer's starting values are chosen, which is why the UIs ask for one rather than
writing a half-filled object.

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
  Every event is `weekend` red, like a weekend: a sector, not a mark.
- `deny_unknown_fields` on every config struct, on the `Layer` enum too: a typo
  must be a parse error. It sits next to the serde field defaults on purpose —
  `{"amount": 1, "amoun": 2}` is a typo, not a request for the default.
  `config.schema.yaml` is what makes an editor agree, and it is generated rather
  than written: the `schema` feature derives it from the same structs, off by
  default because the derive drags the schema generator into a wasm whose bytes
  are counted. One test writes the file and fails if it was stale. It is a JSON
  Schema, serialised as YAML because a person reads it — which is also why the
  editor needs to be *told* about it: the `foo.schema.json` convention that
  auto-associates a schema with `foo.json` does not survive the extension.
- **A colour is a palette slot name or a `#rrggbb`, never a field of its own.**
  `Palette::color` resolves one string, so `background: weekend` follows the
  theme and `background: "#800000"` does not, with no extra config surface. Every
  slot is a *name* — `colors.winter`, not `colors.seasons.0` — because the layer
  that draws with it has to be able to write the name down.
- **A border is a rule in the colour of the paper.** `Border`'s default is the
  `background` slot, so adding a border without a colour divides whatever is
  under it and disappears where nothing is. Name a colour when you want to see it.
- Everything time-of-day on a ring sweeps continuously. Angles are fractions of
  the scale's period, never integer-clipped.
- Write comments for *why*, never for what.

## Running it

```sh
nix develop                 # cargo, clippy, rustfmt, feh, a font for resvg
nix develop -c cargo test
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo fmt
nix run . -- -c wycca.yaml --events holidays-uk.ics --png wall.png
wycca -a 2026-10-13T14:30:00+02:00   # deterministic, for eyeballing
wycca -f                              # render a PNG and open it in feh

cargo test -p wycca-core --features schema   # rewrites config.schema.yaml
```

The browser page is a file, not a server. `nix build .#web` is the same thing
`web/release.sh` builds, for a user with nix and no toolchain; open either one
directly:

```sh
nix develop -c web/build.sh   # cargo -> base64 -> web/wycca.wasm.js
xdg-open web/index.html       # or file://... in any browser, offline
```

A release is the same page, tuned for bytes and without the `file://` fallback.
Pushing a `v*` tag runs the workflow in `.github/workflows/release.yml`, which
does exactly this and uploads the tarball:

```sh
nix develop -c web/release.sh    # web/dist/ + wycca-web-<version>.tar.gz
WYCCA_VERSION=v0.1.0 nix develop -c web/release.sh
gh release create v0.1.0 wycca-web-v0.1.0.tar.gz
```

Verify by **probing the rendered output**, not by looking at the SVG: sample the
pixel at a given radius and angle and compare with the colour you expect, or just
count the colours — `grep -o 'fill="#[0-9a-f]*"' wall.svg | sort | uniq -c`
should account for every layer, which is how the seasons went missing once
already. ASCII-art downsampling is fine for a quick shape check.

## Local files, gitignored

`wycca.yaml` is the user's config and `holidays-uk.ics` a Google holiday calendar.
Refresh the calendar with:

```sh
curl -o holidays-uk.ics "https://calendar.google.com/calendar/ical/en-gb.uk%23holiday%40group.v.calendar.google.com/public/basic.ics"
```

Config is JSON in the core, YAML in the CLI (`.yaml`/`.yml` by extension). YAML is
CLI-only on purpose: it is the only place a person hand-writes a config, and only
there do comments earn their keep.

## Decisions worth remembering

- **Holidays come from `.ics`, never from a hardcoded country table.** Google's
  per-country calendars (`.../calendar/ical/<cc>#holiday@group.v.calendar.google.com/public/basic.ics`)
  are free, need no key, run to 2031, and model *transferred days off*
  (`Substitute Bank Holiday for Boxing Day`), which the usual APIs do not.
  Nager.Date was checked against one of these feeds and rejected: it returns a
  byte-identical list for every year, dropping observance days and getting some
  dates wrong before 2025.
- **Whole-day events only.** `DTSTART;VALUE=DATE` is the test for "whole day";
  anything with a time is skipped because a ring has no room for one.
  `TRANSP:TRANSPARENT` is the test for "a day off" and `only` is the config's way
  of asking for one. An event is a `weekend`-red sector, like a weekend: one day
  across the ring, no label — the names run into the month numbers on a ring that
  thin. `Config::feeds` is a name to
  days map, so a layer can read one and `--events` has somewhere to put it: it
  lands in `DEFAULT_FEED`, the feed the year ring's holidays layer already names.
- **The config UI is described by the core, never implemented by it.** `ui.rs`
  emits sections, fields and ranges as JSON; the browser and the widget render
  that with their own widgets. Two UIs, one description. The only thing tying them
  together is the dotted key strings, and two tests in `ui.rs` check every one of
  them against `Config`.
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
- **The release is a tarball, and its build is a second script.** `release.sh`
  differs from `build.sh` in three measured things, and the numbers are why: a
  wasm-only `[profile.wasm-release]` at `opt-level = "z"`, and a binaryen
  `wasm-opt` **re-encode only** — its encoder is tighter than LLVM's. Together
  they take the file from 394 KB to 335 KB and, gzipped, from 137 KB to 116 KB.
  wasm-opt's *optimizing* passes (`-Oz`) do better raw (302 KB) and worse gzipped
  (120 KB), and gzip is what nginx sends, so they are off; do not "fix" that by
  adding `-Oz` back without re-measuring. The output is `web/dist/`, which has no
  `wycca.wasm.js`: the base64 copy is for a page opened from disk, and a deployed
  page fetches its wasm like any other asset. `nix build .#web` does the same
  work and repeats those `wasm-opt` flags verbatim, so a change to one is a
  change to both.
- **What the wasm's size is made of, now that it has been looked at.** Rust's
  `core::fmt` for `format!("{x:.2}")` in `geom.rs`/`svg.rs`, serde's derive tables
  and error strings, chrono's `format`/`parse` module (pulled in by its `serde`
  feature, not by any date we format), panic scaffolding for a dozen always-on
  `assert!`s in the dependencies, and ~700 monomorphized functions. Neither debug
  symbols (already stripped by the profile, there is not one custom section in the
  file) nor `pub` (not a wasm root) is among them. Hand-rolling a fixed-point
  number writer to dodge `fmt` was tried and reverted: it measured *worse*, so
  `flt2dec` is reachable from something else and the win is not there.
- **`reversed` goes through `turn()`, never into the geometry.** Every angle in a
  ring is worked out the ring's natural way round and passed through one helper,
  so flipping a ring cannot be half-applied. A test renders each reversible ring
  both ways and compares.
- **`<input type="color">`, not a colourpicker from a CDN.** The page has to work
  offline from a file, and the widget has no DOM at all; the core already reads
  `#rrggbb`, so nothing needs to understand a colour space.
- **The timezone picker is platform glue, not part of the form.** Resolving
  `Europe/London` to an offset needs a tz database and `chrono-tz` is banned, so
  `ui.js` reads `Intl.supportedValuesOf("timeZone")` and recomputes the offset each
  frame (which is also how daylight saving follows). Android will do the same with
  `TimeZone`. Fifteen lines per platform beats the dependency.
- **A new ring arrives through `wycca_ring`, and a new layer through
  `wycca_layer`.** The form says which controls a ring has, not what they start
  at, so the UI asks the core for a `Ring::new(scale)` and never invents a value.
  Changing a layer's kind keeps its `disp` and takes a fresh source, because the
  drawing is the person's and the source is the core's. The same goes for `.ics`:
  `wycca_events` runs the parser the CLI runs, so the page and `wycca --events`
  can never disagree about what a holiday is.
- **A layer's title comes from the core too**, via `wycca_title`: `2d from sat`,
  `3mo from dec`, `point every 6h from 0`. Spelling it in the page instead would
  mean two descriptions of the same thing, and a title is the one line a person
  reads before deciding whether to open the card.

## Not built yet

Android widget (`cargo-ndk`, geometry in Rust, draw in your own process and hand
the launcher a `Bitmap`), exporting a config file, lunar months (1900–2100
new-moon table; no maintained crate exists), day-length wedges, subnational
holidays, `RRULE`, interval rings (a fixed-length scale for sprints and release
trains; every layer works on one, only the scale is missing).

Two marks are described and not built, because nothing draws one yet: a triangle
pointing at the ring from outside it, and a circle. `Border` is where they would
go — `Hand` already has `inner`, the start of a hand, which is the same idea. A
tick that needs a dot at the centre is `svg.circle` and one line.

## Commits

`--no-gpg-sign`. Never commit `wycca.yaml` or the downloaded calendars.