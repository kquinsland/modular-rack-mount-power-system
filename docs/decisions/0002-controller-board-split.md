# 0002: Separate Controller Board

## Decision

The system uses three physical PCB types:

- `controller`: one central WT32-ETH01 controller and the 3-wire fan circuit.
- `backplane`: shared DC distribution, slot LEDs, carrier connectors, and one
  TCA9548A I2C mux per six-slot backplane.
- `carrier`: one reusable fixed-address DC/USB-C PD module carrier.

The WT32-ETH01 and all fan switching/tachometer parts move off the backplane.
The TCA9548A remains on the backplane because it isolates that backplane's local
carrier slots.

## Rationale

A controller is needed only once per system, while backplanes may be repeated.
Keeping the controller and fan circuit on every backplane would duplicate the
most expensive logic and couple the backplane mechanics to the Ethernet module
and fan connector.

The backplane already has separate upstream I2C and LED headers. The controller
uses matching headers, so additional backplanes can continue to daisy-chain the
two buses without duplicating controllers.

## Consequences

- Controller and backplane power are now explicit interfaces.
- The backplane I2C header carries 3.3 V on pin 4; the LED header separately
  carries 5 V on pin 3.
- WT32 GPIO assignments for I2C, LED data, fan PWM, and tach are part of the
  controller interface contract.
- Mechanical connector selection, board outlines, placement refinement, and
  routing remain open work.
