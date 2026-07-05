# Mini Rack Power

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
