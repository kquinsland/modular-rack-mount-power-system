# System Overview

Mini Rack Power is split into a backplane PCB and one or more carrier PCBs.

## Backplane

The backplane provides the shared slot infrastructure:

- XT30-style or hybrid mating connectors for carrier boards.
- DC distribution to each carrier slot.
- I2C fanout through a mux, currently expected to be a TI TCA9548A-family part.
- One isolated I2C channel per carrier slot so fixed-address DC/USB-C PD modules can be reused.

## Carrier

Each carrier PCB plugs into one backplane slot and hosts one fixed-address DC/USB-C PD module. The carrier should expose only the slot-local DC and I2C interface to the backplane.

## Open Items

- Exact XT30-style connector part number and contact allocation.
- Slot count.
- DC voltage and current rating per slot.
- Upstream controller location, if any.
- Whether carrier detection, interrupt, enable, or fault pins are needed in addition to DC and I2C.
