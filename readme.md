# Modular Rack Power

This is a "side-quest" project on the way to building a small "mini rack" dedicated to hosting GH/A runners.

I'll do the CAD and standalone development here before integrating it into the larger project.
The goal is to have a modular system of a backplane and multiple carrier modules that can be swapped in and out to provide different power configurations as needed.
As long as the power supply can handle it, backplanes may share/daisy-chain the
input distribution needed by their carrier modules.

Hardware, firmware, and host tooling for a modular mini-rack power system.

## Boards

- `backplane-backpack`: Active STM32C092 controller. It connects to CAN-FD,
  manages as many as eight logical I2C/PD ports, and controls the fan/status LED.
  Rev A physically exposes ports 0 through 5.
- `backplane`: Hosts carrier slots and distributes the nominal 24 V power bus.
- `carrier`: Mates with one backplane slot and hosts an SW3538 USB-C PD module.

The former WT32 `controller` design is superseded. Its design files remain as
project history but it is not part of the active firmware architecture.

## Layout

```text
crates/
firmware/
hardware/
  boards/
    backplane-backpack/
    backplane/
    carrier/
  libraries/
    symbols/
    footprints/
    3dmodels/
  mechanical/
docs/
tools/
scripts/
build/
releases/
```

`build/` is for generated local outputs. `releases/` is for fabrication packages that should be preserved exactly as sent to a board house.

See [`firmware/plan.md`](firmware/plan.md) for the reviewed implementation and
repository-integration plan.

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