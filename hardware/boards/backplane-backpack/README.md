# Backplane Backpack

This board mates with the backplane—hence the "backpack" name. It shares the
backplane's mechanical footprint and contains the control electronics that would
otherwise displace high-current routing.

Rev A is a CAN-FD-to-I2C/PD controller built around an STM32C092GCU7. It includes:

- a TCAN3413 CAN-FD transceiver;
- a TCA9548A mux with six populated downstream I2C connectors and two test-pad
  channels;
- support for a configurable 3-wire or 4-wire 12 V PC fan; and
- one WS2812-compatible status LED.

Firmware and protocol capacity is eight zero-based logical ports. Rev A supports
ports 0 through 5; ports 6 and 7 are explicitly unsupported rather than absent.

Rev A has no MCU-controlled high-side switch per PD module. Firmware policy is
therefore a soft limit that applies after module discovery/configuration, and the
emergency-disable operation is best effort through I2C. See
[`firmware/plan.md`](../../../firmware/plan.md) for the accepted limitations and
future-hardware recommendation.
