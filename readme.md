# Modular Rack Power

This is a "side-quest" project on the way to building a small "mini rack" dedicated to hosting GH/A runners.

I'll do the CAD and standalone development here before integrating it into the larger project.
The goal is to have a modular system of a backplane and multiple carrier modules that can be swapped in and out to provide different power configurations as needed.
As long as the power supply can handle it, backplanes may share/daisy-chain the
input distribution needed by their carrier modules.

Hardware, firmware, and host tooling for a modular mini-rack power system.

## Boards

- `backplane`: Consolidated STM32C092/CAN-FD backplane, formerly named
  `backplane-prototype`; hosts six carrier slots, power distribution, current
  monitoring, fan control, and status LED.
- `carrier`: Mates with one backplane slot and hosts an SW3538 USB-C PD module.

The former split backplane/backpack and WT32 controller projects are retired;
their sources remain in Git history. Shared symbols are kept in the common
library directory. The existing backpack firmware is retained as a legacy
target; moving/renaming the hardware does not port its pinout or peripheral model
to the consolidated board.

## Layout

```text
firmware/
  Cargo.toml
  backplane-backpack/
  crates/
  tools/
  xtask/
  docs/pdcan/
hardware/
  boards/
    backplane/
    carrier/
  libraries/
    symbols/
    footprints/
    3dmodels/
  mechanical/
docs/
scripts/
build/
releases/
```

`build/` is for generated local outputs. `releases/` is for fabrication packages that should be preserved exactly as sent to a board house.

See [`firmware/README.md`](firmware/README.md) for workspace commands and the
legacy firmware's hardware limitations. Run `mise run firmware:check` from this
directory, or run Cargo commands from `firmware/`. The reviewed implementation
plan is kept in [`firmware/plan.md`](firmware/plan.md).

## Documentation site

The public-facing project documentation is maintained in [`site/`](site/) and
is intended for <https://mrp.karlquinsland.com/>. Run `mise run site:serve` for
local preview or `mise run site:build` for a production build. Create journal
entries with `mise run worklog:new -- "Concise Summary"` so their filenames and
front matter follow the project convention.

## Misc

I can't believe that it's STILL such a pain in the ass to use LCSC to search for parts that are stocked by the SMT service.
https://yaqwsx.github.io/jlcparts/#/

Also super useful reference: https://www.tinkervault.com/usb-power-sources/dc-pd30-adapters
It's how I 'caught' that the `sw3538` based module I am using is non-standard 140W, later confirmed:

- https://en.eeworld.com.cn/Reference_Designs/detail/81702
- https://github.com/happyme531/h1_SW35xx/issues/13#issuecomment-4361605621


https://github.com/Shrike-Lab/HomeLab-PDU-V1/blob/main/ASSEMBLY/README.md#pcb-tray


Plan was to use APK43070 but they do not support MCU controlling it via i2c

https://meron33.hatenablog.com/entry/2026/02/07/112759
