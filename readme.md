# Modular Rack Power

This is a "side-quest" project on the way to building a small "mini rack" dedicated to hosting GH/A runners.

I'll do the CAD and standalone development here before integrating it into the larger project.
The goal is to have a modular system of a backplane and multiple carrier modules that can be swapped in and out to provide different power configurations as needed.
As long as the power supply can handle is, the backplane modules should be daisy-chained to provide more power to the carriers.

KiCad hardware project for a modular mini-rack power system.

## Boards

- `controller`: Hosts the WT32-ETH01, fan switch/tach circuit, and upstream I2C/LED interfaces.
- `backplane`: Hosts the carrier slots, distributes DC power, carries slot LEDs, and fans I2C out through a mux so multiple fixed-address carrier modules can coexist.
- `carrier`: Mates with one backplane slot and hosts a DC/USB-C PD module plus any local carrier-side support circuitry.

## Layout

```text
hardware/
  boards/
    backplane/
    carrier/
    controller/
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

Also super useful reference: https://www.tinkervault.com/usb-power-sources/dc-pd30-adapters
It's how I 'caught' that the `sw3538` based module I am using is non-standard 140W, later confirmed:

- https://en.eeworld.com.cn/Reference_Designs/detail/81702
- https://github.com/happyme531/h1_SW35xx/issues/13#issuecomment-4361605621


https://github.com/Shrike-Lab/HomeLab-PDU-V1/blob/main/ASSEMBLY/README.md#pcb-tray
