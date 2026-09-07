# Consolidated backplane

This project is the working second-generation backplane. It consolidates the
former `backplane`, `backplane-prototype`, and `backplane-backpack` projects onto
one PCB.

The existing PCB in this directory is the mechanical authority for the board
outline, airflow cutouts, mounting holes, and six slot-connector footprints and
placement. The carrier is the electrical authority for circuitry shared between
the two boards, including the slot pinout, STM32, CAN transceiver, current
sensor, and slot connector pinout.

## Implemented schematic

The root schematic is divided into four sheets:

- `01_power_input.kicad_sch`: DC input, 1 mOhm high-side current shunt,
  INA237 monitor, and separate LM5164 3.3 V and 12 V converters;
- `02_control.kicad_sch`: STM32C092GCU6, reset/decoupling, SWD, and test points;
- `03_can_slots.kicad_sch`: TCAN3413 CAN-FD interface, external CAN protection,
  optional termination, and the six carrier slots; and
- `04_fan_status.kicad_sch`: two independent 3-wire fan supply-PWM switches and
  tachometer inputs, plus an externally powered DS18B20 interface. Per-slot
  addressable LEDs were removed to keep automated assembly on one side of the
  PCB.

Both regulators use LM5164DDAR, LCSC C477928, with
PSPMAA0805-101M-ANP 100 uH inductors, LCSC C2962892. The output-specific
feedback, on-time, ripple-injection, and capacitor networks remain separate.

The PCB stackup specifies 2 oz outer copper, 1 oz inner copper, and an ENIG
surface finish.

Generate the backplane BOM, placement file, Gerber/drill archive, IPC-D-356
netlist, designator inventory, and validation manifest with
`mise run production:backplane`. A successful export records warnings in the
manifest; it does not by itself supersede the fabrication-readiness status
below.

The local I2C2 bus uses STM32 PA6/PA7 and exists only between the STM32 and INA237. The old TCA9548A,
PCA9554, per-slot I2C protection, and per-slot power-switch sheets are not part
of this design. Carrier communication is CAN-FD.

## Connectors

`J5` is the main DC input:

1. GND
2. VIN_RAW

`J2` is a XINLAIYA XY308-2.54-3P three-position screw terminal, LCSC
`C557686`:

1. GND
2. CANL
3. CANH

`J2` is intentionally marked DNP and excluded from pick-and-place output because
the user installs it by hand after assembly. D1 and L3 protect only this external
CAN boundary. Each carrier has its own connector-side CAN protection, so the
backplane does not duplicate ESD parts at every slot. `SJ1` and `R17` provide
normally-open 120 ohm termination for use only when this backplane is at a
physical bus end.

The six carrier slots retain the authoritative combined XT30 plus two-signal
footprint. Their physical pinout is:

1. VIN_BUS — power contact furthest from the two low-voltage signal contacts
2. GND — power contact between pin 1 and the two low-voltage signal contacts
3. CANH
4. CANL

The slot references are `J1`, `J3`, `J4`, `J6`, `J7`, and `J8`, from slot 1
through slot 6. `J9` and `J11` are independently controlled 3-wire fan
connections: GND, switched 12 V, and tach.

CAN differential nets use KiCad's `_P`/`_N` naming convention on both sides of
the common-mode choke: `CAN_BUS_P`/`CAN_BUS_N` and
`CAN_EXT_P`/`CAN_EXT_N`, with `P = CANH` and `N = CANL`. Custom DRC rules
enforce a 0.20-0.30 mm gap, 5 mm maximum uncoupled length, and 1 mm maximum
within-pair skew.

`J12` is the externally powered DS18B20 header:

1. GND
2. DQ (`DS18B20_DATA`, pulled up to 3.3 V through `R24`)
3. +3V3

The DS18B20 data signal uses STM32 `PA15`. The local status NeoPixel uses `PB8`
through a 100 ohm series resistor and has a dedicated 100 nF bypass capacitor.
Fan 1 uses `PB4`/`TIM3_CH1` for PWM and `PB3`/`TIM3_CH2` for tach capture in
the hardware pin allocation. Fan 2 uses `PA0`/`TIM2_CH1` for PWM and
`PA1`/`TIM17_CH1` for tach capture. Firmware that reserves TIM3 for timekeeping
must instead treat the Fan 1 pins as GPIOs or move the time driver to another
timer.

## Power assumptions

The first population targets a nominal 24 V input and up to 100 W output per
slot. Six fully loaded slots produce a working input budget of approximately
30 A. `RSH1` is 1 mOhm, giving 30 mV and 0.9 W at 30 A; configure the INA237 for
its narrow shunt range and route the sense pair as true Kelvin connections.

The later design goal is approximately 48--50 V nominal and 240 W per slot.
That is a future characterization target, not a released rating, until the
input-protection, current-path, thermal, transient, and first-article tests are
complete.

Both fan channels share the LM5164 12 V rail. Its combined continuous,
startup, and stall load must be tested with the intended pair of fans; adding a
second connector does not increase the regulator's 1 A output rating.

## PCB status

The consolidated schematic is synchronized into the PCB, with functional groups
staged outside the authoritative outline for placement and routing. Do not
fabricate the current board yet. The next PCB stage must:

- retain the authoritative outline, cutouts, holes, and slot placement;
- place the selected external CAN terminal; confirm the two generic 2.54 mm
  fan-header footprints and DS18B20 pin header against the sourced parts;
- import and place the new control, CAN, monitor, fan, and regulator circuitry;
- re-engineer the high-current pours, shunt Kelvin routing, regulator loops,
  CAN-FD trunk/stubs, grounding, thermal paths, and test access; and
- pass PCB DRC, schematic/PCB parity, assembly, and first-article review.

See `../../../docs/interfaces.md`, `../../../docs/system-overview.md`, and
`../../../docs/power-budget.md` for the system-level definitions.
