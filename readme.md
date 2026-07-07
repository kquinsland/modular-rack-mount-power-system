# Modular Rack Power

This is a "side-quest" project on the way to building a small "mini rack" dedicated to hosting GH/A runners.

I'll do the CAD and standalone development here before integrating it into the larger project.
The goal is to have a modular system of a backplane and multiple carrier modules that can be swapped in and out to provide different power configurations as needed.
As long as the power supply can handle is, the backplane modules should be daisy-chained to provide more power to the carriers.

KiCad hardware project for a modular mini-rack power system.

## Boards

- `backplane`: Hosts the carrier slots, distributes DC power, and fans I2C out through a mux so multiple fixed-address carrier modules can coexist.
- `carrier`: Mates with one backplane slot and hosts a DC/USB-C PD module plus any local carrier-side support circuitry.

A third board can be added later under `hardware/boards/` without changing the shared library or release structure.

## Layout

```text
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

## Misc

I can't believe that it's STILL such a pain in the ass to use LCSC to search for parts that are stocked by the SMT service.
https://yaqwsx.github.io/jlcparts/#/
