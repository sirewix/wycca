# Circular calendar app

## Platforms
- linux cli app that renders svg or png (then either sets it as a wallpaper on cron/scheduler or just opens a `feh` window)
- web app (svg or canvas)
- android widget (todo in the future)

## Drawing

Some platform-agnostic renderer, basic shapes: lines, circles, triangles, rectangles. Text only for numbers. Flat colors, ideally smooth antialised edges.

## Output

Output contains of several rings that can be configured. Dark and light themes must be supported from the start. Colors should be configurable. Background must be either transparent or color-filled.

### Rings

Rings contain sectors and markers. Sectors are color filled.

Types of possible rings (by data source):
- year
- month
- lunar month (if can be determinisically calculated)
- week
- hour
- minute
- second
- xn — any of the listed above times n. For example "a sprint" is 2 weeks. Start point and important markers/milestones — configurable

### Markers

Dots or short lines. The most inner circle can use lines starting from the center, similar to clock hands. Markers can be annotated by numbers, preferably spaced out

## Configuration

Output must be configurable. Start with simple json config, postpone gui android widget configuration for later. webapp only statically configurable, no gui config.
Everything must be configurable: colors, rings presence and order, rings angular offset.

## Default preset

Rings starting from inner to outer:
- 24h clock, just hours, no minutes or seconds. 360 degrees contain all 24 hours not just 12. Start position and rotation as conventional clocks. Use line/hand for marking current hour (clip to lowest integer hour)
- Week calendar. Week starts from monday, weekend is colored red as sectors, current day is marked.
- Year calendar. Time flows counter-clockwise, seasons are symmetrical: winter at the top, spring at the left, summer at the bottom, fall at the right. Year starts not at a right angle because of this. Seasons are color coded: winter is blueish white, spring is light but pastel green, autumn is sandy-yellow, fall is brownish. Months and weeks are marked.

Whatever is not specified here, feel free to choose arbitrarily, but leave a config option.
