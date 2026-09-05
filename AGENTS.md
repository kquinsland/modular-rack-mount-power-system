# Repository guidance

## Component datasheets

Before searching the web for a component datasheet, check `docs/data-sheets/`. The repository copy is usually the quickest source for pinouts, electrical limits, package dimensions, and recommended land patterns.

## KiCad power symbols

Use KiCad's official power symbols, such as `power:+3V3` and `power:GND`, for standard power rails. Do not substitute generic or global labels for rails that have an official power symbol. Reserve generic and global labels for signals and nonstandard named rails.
