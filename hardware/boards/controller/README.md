# Controller PCB

> **Status:** Superseded by the STM32/CAN-FD `backplane-backpack` architecture.
> These KiCad sources remain as design history and are not an active firmware or
> manufacturing target.

The historical design was intended to:

- Host the WT32-ETH01 ESP32/Ethernet module.
- Drive and monitor one 3-wire 12 V fan.
- Accept regulated 5 V and 12 V controller rails.
- Provide upstream I2C/3.3 V and addressable-LED/5 V links to the first backplane.

See `../../../docs/decisions/0003-backplane-backpack-architecture.md` for the
superseding decision.
