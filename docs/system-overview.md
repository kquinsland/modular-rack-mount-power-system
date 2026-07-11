# System Overview

Mini Rack Power uses one central controller PCB, one or more backplane PCBs, and
one carrier PCB per populated slot.

```text
                         +--> 12 V fan
                         |
Controller -- I2C/3V3 --+--> Backplane -- slot DC/I2C --> Carrier(s)
           `- LED/5V -------'       `-- daisy-chain --> more backplanes
```

## Controller

The single controller PCB provides:

- A WT32-ETH01 ESP32/Ethernet module.
- High-side supply PWM and tachometer input for one 3-wire 12 V fan.
- The upstream I2C bus and 3.3 V logic rail for the backplane mux.
- Addressable-LED data and 5 V for the backplane LED chain.
- A regulated 5 V/12 V power-input header.

## Backplane

The backplane provides the shared slot infrastructure:

- XT30-style or hybrid mating connectors for carrier boards.
- DC distribution to each carrier slot.
- I2C fanout through a mux, currently expected to be a TI TCA9548A-family part.
- One isolated I2C channel per carrier slot so fixed-address DC/USB-C PD modules can be reused.
- Slot-status LEDs and daisy-chain headers for I2C and LED data.

## Carrier

Each carrier PCB plugs into one backplane slot and hosts one fixed-address DC/USB-C PD module. The carrier should expose only the slot-local DC and I2C interface to the backplane.

## Open Items

- Exact XT30-style connector part number and contact allocation.
- Slot count.
- DC voltage and current rating per slot.
- Whether carrier detection, interrupt, enable, or fault pins are needed in addition to DC and I2C.
- Final keyed connector families for controller power, I2C, LED, and fan wiring.
- Verified 3.3 V current budget for the WT32 output and all daisy-chained backplane muxes.
- Board outlines, mounting, placement refinement, and routing.
