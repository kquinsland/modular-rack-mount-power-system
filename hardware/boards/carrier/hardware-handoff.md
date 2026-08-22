# USB-C PD Carrier PCB Hardware Handoff

**Status:** High-level electrical design handoff for KiCad implementation

**Revision:** Rev A architectural handoff

**Design intent:** Per-module USB-C PD carrier with protected 24–48 V input power switching, current/power telemetry, dual-I²C control, and local 3.3 V housekeeping supply. Rev A is product-limited to 100 W while using a 140 W-capable carrier architecture.

---

## 1. Scope

This carrier PCB is intended to sit between a shared high-current DC distribution bus and one USB-C Power Delivery module.

The carrier shall:

- Accept approximately **24–36 VDC nominally**, with future support for **48 VDC continuous input**.
- Supply a local **3.3 V housekeeping rail** for:
  - STM32 MCU
  - INA237 power monitor
  - WS2812B-2020-V6 RGB status LED
  - I²C pull-ups and miscellaneous logic
- Switch the high-current input rail to the USB-C PD module under MCU control.
- Protect the PD branch against:
  - excessive startup/inrush current
  - downstream short circuits
  - overloads
  - excessive pass-MOSFET dissipation during startup/faults
  - undervoltage / overvoltage
- Measure PD-module input:
  - bus voltage
  - current
  - power
- Provide:
  - one backplane CAN-FD interface
  - one local/downstream I²C bus, with the STM32 acting as controller/master
- Control a single WS2812-compatible RGB status LED.
- Default the PD-module power path to **OFF** whenever the MCU is unpowered, resetting, or not explicitly enabling the branch.

This is a **high-level electrical handoff**, not a production-release schematic. The KiCad engineer should validate component footprints, pin numbering, manufacturer land patterns, thermal design, switching-regulator layout, and protection margins before release.

The present work scope is hardware, schematic, PCB, and physical validation only. Firmware references below define the hardware interface and expected future ownership; firmware implementation is deferred.

The only electrical interface between the backplane and carrier is:

- unswitched DC input
- ground
- backplane CANL
- backplane CANH

The backplane does not supply a carrier logic rail. Each carrier generates its own 3.3 V housekeeping rail from the unswitched DC input.

---

## 2. Measured PD-module characteristics

The following measurements were made on the actual PD module and should be treated as design inputs:

| Parameter | Measured value |
|---|---:|
| Effective input capacitance while powered off | **~300 µF** |
| SDA to GND resistance while powered off | **~15 MΩ** |
| SCL to GND resistance while powered off | **~15 MΩ** |

The approximately 15 MΩ powered-off I²C resistance is encouraging and suggests that the PD module presents a high-impedance load to the I²C bus when unpowered.

No I²C bus-isolation IC is included in this design.

During bring-up, verify the powered-off behavior explicitly:

1. Leave the PD module unpowered.
2. Pull SDA and SCL to 3.3 V through approximately 10 kΩ.
3. Verify both lines rise close to 3.3 V.
4. Verify the pull-ups do not measurably back-power the PD module.
5. Verify the module does not clamp or distort either I²C line.

---

## 3. System assumptions

### 3.1 Product-generation power targets

Keep these three ratings distinct:

| Generation / constraint | Power target | Hardware interpretation |
|---|---:|---|
| Current product generation | **100 W maximum** | Enforced by the selected PD module/configuration and system policy; normal worst-case input current is below the 140 W sizing case. |
| This carrier architecture | **140 W capable** | Size the carrier power path, current limit, connector contacts, copper, and thermal design for this case. |
| Future hardware generation | **240 W capable** | Requires a new hardware review/revision; it is not a rating of this Rev-A carrier. |

The 140 W architecture case is the electrical sizing corner for this carrier.

At the low-voltage operating corner, assuming approximately 90% converter efficiency:

\[
I_{PD-IN} \approx \frac{140W}{24V \times 0.90} \approx 6.48A
\]

The carrier high-current path shall therefore be designed for at least:

- **6.5 A continuous**
- approximately **11 A short-duration fault/current-limit current**
- appropriate margin in connector contacts, copper, vias, shunt, and MOSFET

### 3.2 Sequential startup

An external controller will enable modules **sequentially**, not simultaneously.

This substantially reduces aggregate bus inrush, but each carrier still retains a dedicated hot-swap controller because it also provides:

- per-branch hardware current limiting
- short-circuit protection
- controlled startup
- MOSFET SOA/power limiting
- UVLO/OVLO
- power-good indication
- autonomous hardware fault handling

### 3.3 48 V interpretation

The design target is **up to 48 V continuous input**, not a telecom-style “48 V nominal” rail that normally operates in the high-50-V range.

If the eventual source can exceed 48 V continuously, revisit:

- TVS selection
- INA237 85 V bus/common-mode limit
- UVLO/OVLO values
- MOSFET voltage rating
- all high-voltage capacitors

---

# 4. Functional architecture

```text
                                      24–48 VDC
                                         │
                                  J1 DC + I²C
                                         │
                               ┌─────────┴─────────┐
                               │      DTVS1        │
                               │     SMCJ48A       │
                               └─────────┬─────────┘
                                         │
                                       VCC
                                         │
                    ┌────────────────────┴─────────────────────┐
                    │                                          │
                    │                                          │
             HOUSEKEEPING POWER                         PD POWER BRANCH
                    │                                          │
             ┌──────▼──────┐                                   │
             │ U2 LM5163   │                                   │
             │ 100 V buck  │                                   │
             └──────┬──────┘                                   │
                    │ +3V3                                      │
       ┌────────────┼───────────────┐                           │
       │            │               │                           │
       ▼            ▼               ▼                           ▼
   U1 STM32      U4 INA237       LED1 WS2812                RSH1 6mΩ
       │                            │                           │
       │                            │                       HS_SENSE
       │                            │                           │
       │                     ┌──────┴─────┐               ┌─────▼──────┐
       │                     │            │               │ U3 LMX5069│
       │                     │            │               │ hot-swap  │
       │                     │            │               └─────┬──────┘
       │                     │            │                     │ GATE
       │                     │            │               ┌─────▼──────┐
       │                     │            │               │ Q1 100 V  │
       │                     │            │               │ Linear FET│
       │                     │            │               └─────┬──────┘
       │                     │            │                     │
       │                     │            │                 PD_VIN_SW
       │                     │            │                     │
       │                     │            │                     ▼
       │                     │            └──────────────► J3 PD MODULE
       │                     │                                  │
       │                     └──── I2C1 ────────────────────────┤
       │                                                        │
       └──── FDCAN ──► U5 TCAN3413 ──► J1 BACKPLANE CAN-FD     │
```

The housekeeping branch connects to `VCC` **before** the PD current shunt. Therefore the INA237 reports the PD-module branch power rather than including the STM32/LED housekeeping power.

---

# 5. Major component BOM

## 5.1 Design-critical active components

| Ref | Qty | Manufacturer / Part | LCSC | Package | Function |
|---|---:|---|---|---|---|
| U1 | 1 | **ST STM32C092GCU7** | — | UFQFPN-28, 4 × 4 mm | MCU with FDCAN |
| U2 | 1 | **TI LM5163DDAR** | **C2873264** | SO PowerPAD-8 | 6–100 V synchronous buck |
| U3 | 1 | **Wuxi Maxinmicro LMX5069MS** | **C47967145** | MSOP-10 | Hot-swap / inrush controller |
| U4 | 1 | **TI INA237AIDGSR** | **C2864837** | VSSOP-10 | I²C voltage/current/power monitor |
| U5 | 1 | **TI TCAN3413DR** | — | SOIC-8 | 3.3 V CAN-FD transceiver |
| Q1 | 1 | **Infineon IPB020N10N5LF** | **C536484** | TO-263 / D²PAK | 100 V linear-mode high-side pass MOSFET |
| Q2 | 1 | **2N7002** | **C8545** | SOT-23 | Fail-safe UVLO clamp |
| Q3 | 1 | **2N7002** | **C8545** | SOT-23 | MCU enable inversion / fail-safe control |
| RSH1 | 1 | **Milliohm HoLLR2512-3W-6mR-1%** | **C2985709** | 2512 | 6 mΩ shared current shunt |
| DTVS1 | 1 | **SMCJ48A** | **C5861043** | SMC | Input TVS |
| D2 | 1 | **Nexperia PESD2CANFD27V-TR** | — | SOT-23 | CAN-FD bus ESD protection |
| L1 | 1 | **YJYCOIN YNR6045-680M** | **C341069** | ~6 × 6 mm | 68 µH buck inductor |
| L3 | 1 | **TDK ACT1210D-101-2P-TL00** | **C3039743** | ACT1210 | CAN common-mode choke |
| LED1 | 1 | **Worldsemi WS2812B-2020-V6** | **C52917434** | 2020 | RGB status LED |

### Q1 sourcing note

`IPB020N10N5LF` is selected because the pass transistor in a hot-swap circuit must tolerate significant **linear-mode/SOA stress** during startup and fault limiting.

Do **not** replace Q1 based only on:

- lower RDS(on)
- higher pulsed current
- cheaper unit price
- same VDS rating

Any substitute must have a manufacturer-specified DC/pulsed linear-mode SOA that survives the programmed LMX5069 power-limit behavior at the 48 V corner.

Q1 is currently expected to be one of the more expensive and sourcing-sensitive parts in the design, so it should be treated as a procurement-review item before production.

## 5.2 Comparison with the backplane-prototype switch

The `backplane-prototype` uses the following discrete per-slot switch:

- Vishay `SQD50P06-15L_GE3`, 60 V P-channel MOSFET in TO-252/DPAK
- BSS123 N-channel MOSFET to pull the P-MOS gate down
- 1 kΩ BSS123 gate-series resistor
- 10 kΩ BSS123 gate pull-down
- 10 kΩ P-MOS gate pull-down/current-limiting resistor
- 100 kΩ P-MOS source-to-gate pull-up for default OFF
- BZT52C12 source-to-gate zener clamp

That circuit is a reasonable compact on/off switch for the prototype voltage/current scope, but it shall **not** be copied onto this carrier.

| Attribute | Backplane prototype | Carrier Rev A | Decision |
|---|---|---|---|
| Pass device | SQD50P06 P-channel, 60 V, 15.5 mΩ max at 25 °C | IPB020N10N5LF N-channel Linear FET, 100 V, 2.0 mΩ max at 25 °C | Use the carrier selection. The prototype FET has inadequate transient margin for a 48 V bus and SMCJ48A clamp. |
| Gate drive | Discrete BSS123 pull-down with passive default-off network and 12 V zener clamp | LMX5069 high-side charge pump plus Q2/Q3 fail-safe UVLO control | Use LMX5069; a high-side N-FET requires gate drive above its source. |
| Inrush | Determined primarily by wiring, MOSFET, and load | Controlled current and MOSFET power limiting | Carrier requires the controlled solution for the measured ~300 µF module input. |
| Fault protection | No per-slot current measurement, timer, power limit, or power-good output | Shared 6 mΩ current shunt, programmable current/power limits, TIMER, PGD, UVLO/OVLO | Carrier protection is materially more complete. |
| Telemetry | None in the switch circuit | INA237 bus/current/power measurement using Kelvin shunt traces | Required on carrier. |
| Approx. cold conduction loss at 6.5 A | `6.5² × 15.5 mΩ ≈ 0.65 W` maximum | `6.5² × 2.0 mΩ ≈ 0.085 W` maximum | The N-FET also provides substantially lower normal loss. |

The useful concepts retained from the prototype are active-high control, passive default-off behavior, and explicit gate protection. The MOSFET, gate driver, current protection, and fault/status implementation are replaced by the LMX5069 architecture.

---

# 6. Housekeeping 3.3 V buck

## 6.1 Regulator choice

Use **LM5163DDAR** rather than the previously considered 60 V-class converter.

Reasons:

- 6–100 V operating range
- appropriate margin for a future 48 V bus
- synchronous architecture: no external freewheel diode
- 500 mA output capability, far above expected housekeeping consumption
- suitable for direct conversion from 24–48 V to 3.3 V

Expected housekeeping current is dominated by:

- STM32
- WS2812 at full-white brightness
- INA237
- I²C pull-ups

It is expected to remain well below 100 mA.

## 6.2 Proposed buck values

Target switching frequency: approximately **300 kHz**

| Ref | Value | Notes |
|---|---:|---|
| RRON | **27.4 kΩ, 1%** | ~300 kHz target |
| RFB_TOP | **110 kΩ, 1%** | Feedback |
| RFB_BOT | **62 kΩ, 1%** | Feedback |
| L1 | **68 µH** | 1 A nominal / ~1.2 A saturation |
| CIN1 | **2.2 µF / 100 V X7R** | Input ceramic |
| CIN2 | **2.2 µF / 100 V X7R** | Input ceramic |
| COUT | **22 µF / 25 V** | Output ceramic |
| COUT_HF | **100 nF** | Local output bypass |
| CBST | **2.2 nF / 50 V** | Bootstrap |
| RA | **110 kΩ, 1%** | Type-3 ripple injection |
| CA | **2.2 nF / 50 V** | Type-3 ripple injection |
| CB | **220 pF C0G / 50 V** | Type-3 ripple injection |
| R_BUCK_PG | **10 kΩ** | PGOOD pull-up to 3V3 |

Feedback target:

\[
V_{OUT}=1.2 \times \left(1+\frac{110k}{62k}\right)\approx3.33V
\]

### Optional input bulk

Provide a DNP footprint for approximately:

- **10–47 µF**
- **100 V**
- aluminum electrolytic or polymer appropriate for the final source

Populate if cable/source inductance or bench measurements indicate useful input damping.

## 6.3 Buck schematic

```text
                              VCC
                                 │
                 ┌───────────────┼────────────────┐
                 │               │                │
             CIN1 2.2u       CIN2 2.2u        optional bulk
              100 V            100 V           10–47u/100V
                 │               │                │
                GND             GND              GND
                                 │
                           ┌─────▼────────────┐
                           │ U2 LM5163        │
VCC ──────────────────►│ VIN             │
VCC ──────────────────►│ EN/UVLO         │
                           │                 │
BUCK_RON ─────────────────►│ RON             │
BUCK_FB  ─────────────────►│ FB              │
BUCK_PGOOD ◄───────────────│ PGOOD           │
BUCK_BST ─────────────────►│ BST             │
BUCK_SW  ◄─────────────────│ SW              │
GND ───────────────────────│ GND + EP        │
                           └─────────────────┘

BUCK_BST ── CBST 2.2n ── BUCK_SW

BUCK_SW ── L1 68uH ───────────── +3V3
                                      │
                                 COUT 22uF
                                      │
                                COUT_HF 100n
                                      │
                                     GND

+3V3 ── RFB_TOP 110k ──┐
                        ├── BUCK_FB
GND  ── RFB_BOT 62k ───┘

BUCK_SW ── RA 110k ── BUCK_RIPPLE
                           │
                           ├── CA 2.2n ── +3V3
                           │
                           └── CB 220p ── BUCK_FB

+3V3 ── 10k ── BUCK_PGOOD ── U1 PA4
```

---

# 7. Hot-swap, inrush, and PD branch protection

## 7.1 Power path

```text
                                 RSH1
                            6 mΩ / 3 W
VCC ═══════════════════════/\/\/\/══════════════ HS_SENSE
                           Kelvin │   │ Kelvin           │
                                  │   │                  │ D
                                  │   │             ┌────┴─────┐
                                  │   │             │ Q1       │
                                  │   │             │100 V NMOS│
                                  │   │             │Linear FET│
                                  │   │             └────┬─────┘
                                  │   │                  │ S
                                  │   │              PD_VIN_SW
                                  │   │                  │
                                  │   │                  ▼
                                  │   │                 J3
```

Q1 orientation:

- **Drain** = `HS_SENSE`
- **Source** = `PD_VIN_SW`
- **Gate** = `Q1_GATE`
- TO-263 tab = drain

## 7.2 LMX5069 pin mapping

```text
U3 LMX5069MS
────────────────────
1  SENSE
2  VIN
3  UVLO
4  OVLO
5  GND
6  TIMER
7  PWR
8  PGD
9  OUT
10 GATE
```

Connections:

```text
U3.VIN   ── Kelvin VCC side of RSH1
U3.SENSE ── Kelvin HS_SENSE side of RSH1
U3.GATE  ── Q1_GATE
U3.OUT   ── R_OUT 100Ω ── PD_VIN_SW
U3.PGD   ── PD_PGOOD
```

## 7.3 Shared current shunt

Use:

```text
RSH1 = 6.0 mΩ
Tolerance = ±1%
Power rating = 3 W
```

LMX5069 current-limit threshold:

- minimum approximately 48 mV
- typical approximately 55 mV
- maximum approximately 65 mV

Including shunt tolerance:

\[
I_{LIMIT,min}\approx\frac{48mV}{6.06m\Omega}\approx7.92A
\]

\[
I_{LIMIT,typ}\approx\frac{55mV}{6m\Omega}\approx9.17A
\]

\[
I_{LIMIT,max}\approx\frac{65mV}{5.94m\Omega}\approx10.94A
\]

This provides useful margin over the expected ~6.5 A worst normal load at 24 V.

Approximate shunt loss at 6.5 A:

\[
P=I^2R=6.5^2\times0.006\approx0.254W
\]

Approximate shunt loss near the maximum current-limit corner:

\[
P\approx10.94^2\times0.006\approx0.72W
\]

The 3 W shunt therefore has substantial thermal margin.

## 7.4 MOSFET power limit

Use:

```text
RPWR = 36.0 kΩ, 1%
```

With a 6 mΩ shunt and 48 V maximum input, the LMX5069 design equation gives approximately 41 W at the conservative low power-limit corner. It is not correct to label this value "typical."

The datasheet specifies the power-limit sense threshold only at `VDS = 48 V` and `RPWR = 150 kΩ`: 19 / 25 / 31 mV minimum / typical / maximum. Applying the controller's approximately 1 mV offset and scaling the remainder to 36 kΩ gives this first-order estimate:

| RPWR power-limit corner at 48 V | Approx. Q1 power |
|---|---:|
| Conservative design-equation minimum | **~41 W** |
| Inferred typical | **~54 W** |
| Inferred maximum | **~66 W** |

The 36 kΩ operating point is near the datasheet's recommended minimum 5 mV sense signal, so treat these as design estimates rather than guaranteed limits at this exact resistor value. Validate the limit on hardware over temperature.

Q1 SOA shall be proven against **~66 W and the maximum TIMER interval**, not 41 W / 35 ms. If that hot-corner check fails, revise `RPWR`, Q1, the timer strategy, or the controller architecture; do not release on typical behavior.

## 7.5 Measured 300 µF module startup

Measured PD input capacitance:

```text
C_PD ≈ 300 µF
```

Energy stored at 48 V:

\[
E=\frac12CV^2
 =0.5\times300\mu F\times48^2
 \approx0.346J
\]

Using the approximately 41 W conservative minimum power-limit estimate and ~9.17 A typical current limit gives the slowest calculated LMX5069 startup interval at the 48 V corner:

```text
~8–9 ms conservative power-limit case
```

This is a very manageable startup event.

Sequential external startup further reduces stress on the shared upstream supply because only one module is expected to enter startup at a time.

## 7.6 TIMER

Use:

```text
CTIMER = 680 nF, ±10%, X7R, >=16 V
LCSC suggested part: C237234
```

Approximate typical fault timeout:

\[
t_{FAULT}\approx
\frac{680nF\times3.6V}{70\mu A}
\approx35ms
\]

Approximate conservative minimum using capacitor, threshold, and timer-current corners:

```text
~20 ms
```

This remains comfortably above the expected ~8–9 ms startup interval.

Approximate conservative maximum using `CTIMER = 680 nF +10%`, the 4.0 V maximum TIMER threshold, and the 45 µA minimum fault-charge current:

\[
t_{FAULT,max}\approx
\frac{748nF\times4.0V}{45\mu A}
\approx66.5ms
\]

Use the **minimum** timeout corner to prove that valid startup completes. Use the **maximum** timeout corner to prove Q1 remains inside its hot, worst-case SOA during a persistent short. The final SOA check shall combine the approximately **66 W inferred maximum power limit** with the **66.5 ms maximum timeout**, input-voltage tolerance, PCB/Q1 starting temperature, and the applicable Infineon pulse curve; 41 W / 35 ms is not a release calculation.

### Important TIMER behavior

The LMX5069 uses the same TIMER capacitor for both:

- initial insertion/startup delay behavior
- current/power-limit fault timeout

With 680 nF, the initial insertion delay after raw VIN is first applied is considerably longer than the ~35 ms fault timer:

```text
Typical insertion delay: approximately 0.6 s
Worst-case order of magnitude: approximately 1.5 s
```

This is acceptable because:

- all carriers are expected to receive raw system power before individual PD branches are commanded on
- modules are subsequently enabled sequentially by the external controller

Firmware and system integration should nevertheless avoid assuming that `PD_PGOOD` can assert immediately after system VIN is first applied.

## 7.7 Hot-swap local decoupling

Use:

```text
C_HS = 1 µF / 100 V X7R
```

Place from `HS_SENSE` / Q1 drain to GND physically close to:

- RSH1
- U3
- Q1

---

# 8. UVLO / OVLO

Use independent UVLO and OVLO dividers.

## 8.1 UVLO

```text
VCC ── 110k ──┬── HS_UVLO
                   │
                  18k
                   │
                  GND
```

Approximate intended thresholds:

```text
UVLO rising:  ~19.8 V
UVLO falling: ~17.8 V
```

This prevents the 140 W branch from trying to operate on a substantially collapsed source.

## 8.2 OVLO

```text
VCC ── 110k ──┬── HS_OVLO
                   │
                  5.1k
                   │
                  GND
```

Approximate intended thresholds:

```text
OVLO rising:  ~56.4 V
OVLO falling: ~54.4 V
```

These values permit normal operation through 48 V while providing a hardware shutdown well below the 100 V pass FET rating.

---

# 9. Fail-safe PD enable logic

The PD branch must remain **OFF** if:

- the STM32 is unpowered
- the STM32 is resetting
- the STM32 GPIO is high impedance
- firmware has not explicitly requested PD power

Do not directly connect the MCU GPIO to the LMX5069 UVLO pin.

Use two 2N7002 MOSFETs:

```text
                          VCC
                             │
                           110k
                             │
                             ├──────────── KILL_GATE
                             │               │
                            36k              │ D
                             │          ┌────┴────┐
                            GND         │ Q3      │
                                        │ 2N7002 │
STM32 PD_ENABLE ──────────────── G       │        │
                                        └────┬───┘
                                             S
                                             │
                                            GND


KILL_GATE ───────────── G Q2 2N7002

HS_UVLO ─────────────── D
                        S
                        │
                       GND

PD_ENABLE ── 100k ── GND
```

Behavior:

### Default / fault-safe state

```text
MCU absent/reset/PD_ENABLE LOW
        ↓
Q3 OFF
        ↓
VIN divider drives KILL_GATE high
        ↓
Q2 ON
        ↓
HS_UVLO forced LOW
        ↓
LMX5069 holds Q1 OFF
```

### Enable state

```text
PD_ENABLE HIGH
        ↓
Q3 ON
        ↓
KILL_GATE pulled LOW
        ↓
Q2 OFF
        ↓
normal UVLO divider controls U3
        ↓
PD branch may start
```

This makes PD power control **active-high and fail-safe-off**.

---

# 10. Power-good

```text
+3V3 ── 10k ── PD_PGOOD ── U3.PGD
                         └─ U1 PA3
```

Firmware should use `PD_PGOOD` to distinguish:

- command accepted / enable asserted
- actual successful branch startup

`PD_PGOOD` is the carrier's hardware fault/status indication to the STM32. The LMX5069 does not provide a separate latched `FAULT` pin. Its open-drain `PGD` output deasserts when Q1's drain-to-source voltage rises above the power-good threshold, including while Q1 is off or a fault/retry cycle is in progress.

The LMX5069 automatically retries indefinitely at its TIMER-programmed low duty cycle after a persistent current- or power-limit timeout. That default behavior is intentional. The STM32 observes `PD_PGOOD` and decides whether to:

- leave `PD_ENABLE` asserted and permit hardware retries, or
- drive `PD_ENABLE` low to stop retries and hold the branch off.

Firmware is not in the current-limit loop and cannot weaken the autonomous hardware protection.

Recommended firmware sequence:

```text
receive POWER_ON
    │
    ▼
PD_ENABLE = 1
    │
    ▼
wait for PD_PGOOD
    │
    ├── asserted → probe PD module over I2C1
    │
    └── timeout  → report branch startup failure
```

---

# 11. INA237 current / voltage / power monitor

## 11.1 Measurement topology

The INA237 uses the same RSH1 shunt as the LMX5069.

Use separate Kelvin sense traces from the shunt pads.

```text
Kelvin VIN side
      │
     10Ω
      │
      ├──────── INA_INP_FILT ───── U4 IN+
      │
    100nF
      │
      ├──────── INA_INM_FILT ───── U4 IN-
      │
     10Ω
      │
Kelvin HS_SENSE side
```

The 100 nF capacitor is differential between filtered IN+ and IN- nodes.

## 11.2 INA237 pin map

```text
U4 INA237AIDGSR
────────────────────
1  A1
2  A0
3  ALERT
4  SDA
5  SCL
6  VS
7  GND
8  VBUS
9  IN-
10 IN+
```

Connections:

```text
U4.IN+   = INA_INP_FILT
U4.IN-   = INA_INM_FILT
U4.VBUS  = PD_VIN_SW
U4.VS    = +3V3
U4.GND   = GND
U4.SDA   = PD_I2C_SDA
U4.SCL   = PD_I2C_SCL
U4.ALERT = INA_ALERT
```

`VBUS` is deliberately measured **after Q1**, while current is measured through the upstream RSH1.

Thus the reported load power is approximately the actual voltage presented to the PD module multiplied by branch current.

## 11.3 INA237 ADC range

Configure:

```text
ADCRANGE = 0
Shunt full scale = ±163.84 mV
```

The smaller ±40.96 mV range is not suitable because:

```text
9.17 A × 6mΩ ≈ 55 mV
10.94 A × 6mΩ ≈ 65.6 mV
```

## 11.4 Address

Fixed configuration:

```text
A0 -> GND
A1 -> GND

I²C address = 0x40
```

Each carrier contains only one INA237. Tie A0 and A1 directly to GND; do not fit address-selection or alternate-address strap resistors.

## 11.5 ALERT

```text
+3V3 ── 10k ── INA_ALERT ── U4.ALERT
                         └─ U1 PA2
```

---

# 12. STM32

## 12.1 MCU

```text
U1 = STM32C092GCU7
Package = UFQFPN-28
```

No external crystal is required for this design.

## 12.2 Proposed pin assignment

| Physical pin | STM32 pin | Net / function |
|---:|---|---|
| 1 | PC14 | spare |
| 2 | PC15 | spare |
| 3 | VDD/VDDA | +3V3 |
| 4 | VSS/VSSA | GND |
| 5 | PF2/NRST | NRST |
| 6 | PA0 | spare |
| 7 | PA1 | PD_ENABLE |
| 8 | PA2 | INA_ALERT |
| 9 | PA3 | PD_PGOOD |
| 10 | PA4 | CAN_STB |
| 11 | PA5 | spare |
| 12 | PA6 | spare |
| 13 | PA7 | BUCK_PGOOD |
| 14 | PB0 | spare |
| 15 | PB1 | spare |
| 16 | PA8 | LED_DATA_RAW / TIM1_CH1 |
| 17 | PC6 | spare |
| 18 | PA11 | CAN_RX / FDCAN_RX |
| 19 | PA12 | CAN_TX / FDCAN_TX |
| 20 | PA13 | SWDIO |
| 21 | PA14 | SWCLK |
| 22 | PA15 | spare |
| 23 | PB3 | spare |
| 24 | PB4 | spare |
| 25 | PB5 | spare |
| 26 | PB6 | PD_I2C_SCL / I2C1 |
| 27 | PB7 | PD_I2C_SDA / I2C1 |
| 28 | PB8 | spare |

## 12.3 MCU support

Recommended:

```text
+3V3 ── 100nF ── GND   near MCU supply pins
+3V3 ── 4.7uF  ── GND  local bulk

+3V3 ── 10k ── NRST
NRST ── 100nF ── GND
```

Expose test/programming pads:

```text
TP_SWDIO
TP_SWCLK
TP_NRST
TP_3V3
TP_GND
```

Use pogo/test pads rather than a permanent connector unless mechanical constraints favor otherwise.

---

# 13. CAN-FD and local I²C architecture

## 13.1 Backplane CAN-FD

The STM32 uses its FDCAN peripheral for backplane communication. A TCAN3413DR translates the 3.3 V logic interface to the differential CAN bus. The connector-side network includes a common-mode choke, a PESD2CANFD27V ESD protector, and optional 120 Ω termination selected by SJ1.

```text
U1 PA12 / FDCAN_TX ───── U5.TXD
U1 PA11 / FDCAN_RX ───── U5.RXD
U1 PA4              ───── U5.STB

U5.CANH ── L3 ── CANH_BUS ───── J1 pin 4
U5.CANL ── L3 ── CANL_BUS ───── J1 pin 3
GND                              J1 pin 2
```

SJ1 and R26 provide endpoint termination. Populate/bridge the termination only when this carrier is physically at a bus end. Place D2 adjacent to the connector on the bus side of L3, and route CANH/CANL as a tightly coupled, symmetric pair.

## 13.2 Local / downstream I²C1

The STM32 is controller/master.

```text
U1 PB6 / I2C1_SCL ───── PD_I2C_SCL
U1 PB7 / I2C1_SDA ───── PD_I2C_SDA

U4 INA237.SCL ─────────── PD_I2C_SCL
U4 INA237.SDA ─────────── PD_I2C_SDA

J3.PD_I2C_SCL ─────────── PD_I2C_SCL
J3.PD_I2C_SDA ─────────── PD_I2C_SDA
```

Populate:

```text
+3V3 ── 2.2k ── PD_I2C_SCL
+3V3 ── 2.2k ── PD_I2C_SDA
```

CAN-FD messages may request downstream I²C transactions, but application logic must terminate and validate the CAN protocol before accessing the local bus. This allows firmware to:

- isolate downstream transaction failures
- report PD module communication failures upstream
- power-cycle the PD branch independently
- recover/reinitialize I2C1 without disturbing backplane CAN-FD communication

---

# 14. NeoPixel status LED

Use:

```text
LED1 = WS2812B-2020-V6
Supply = +3V3
```

Connection:

```text
U1 PA0 ── 100Ω ── LED1.DIN

+3V3 ───────────── LED1.VDD
  │
  └── 100nF ── GND

LED1.GND ───────── GND
LED1.DOUT ──────── NC
```

The V6 device is intended to operate directly from the 3.3 V rail, avoiding a logic-level translator.

---

# 15. Input TVS

Use:

```text
DTVS1 = SMCJ48A
LCSC C5861043
```

Connection:

```text
VCC ─────┬──────── rest of carrier
             │
           DTVS1
             │
            GND
```

Place immediately behind the input connector with very short high-current and ground return paths.

## TVS validation concern

The proposed SMCJ48A is an appropriate starting point for a true 48 V maximum-continuous system, but transient validation remains mandatory.

The important limiting device is the INA237:

```text
INA237 maximum bus/common-mode range: 85 V
```

The selected TVS has a nominal clamp in the high-70-V range under its specified pulse conditions, leaving finite but not enormous margin to 85 V.

Validate on the actual:

- supply
- cable/harness
- connector
- branch current
- board layout

under:

- module enable
- module disable
- downstream short
- upstream hot-plug
- hot-unplug / interruption

If measured transients approach the INA237 absolute maximum, revise the TVS/protection architecture.

---

# 16. Complete logical netlist

The following is a signal-level connectivity specification for schematic implementation.

Exact manufacturer pin numbers and footprint pad numbering must be checked against current datasheets and verified in ERC/PCB review.

```text
# ============================================================
# RAW INPUT
# ============================================================

NET VCC
    J1.VIN+
    DTVS1.K
    U2.VIN
    U2.EN_UVLO
    CIN1.1
    CIN2.1
    C_BULK_OPTIONAL.1
    RSH1.POWER_IN
    U3.VIN                # Kelvin from RSH1 upstream pad
    RUV_TOP.1
    ROV_TOP.1
    R_KILL_TOP.1
    R_INA_P.1             # Kelvin from RSH1 upstream pad

NET GND
    J1.GND[*]
    DTVS1.A

    U2.GND
    U2.EXPOSED_PAD
    CIN1.2
    CIN2.2
    C_BULK_OPTIONAL.2
    COUT.2
    COUT_HF.2
    RRON.2
    RFB_BOT.2

    U3.GND
    C_HS.2
    CTIMER.2
    RPWR.2
    RUV_BOT.2
    ROV_BOT.2

    Q2.S
    Q3.S
    R_KILL_BOT.2
    R_PD_EN_PD.2

    U4.GND
    U4.A0
    U4.A1
    C_INA_SUPPLY.2

    U1.VSS_VSSA
    C_MCU.2
    C_MCU_BULK.2
    C_NRST.2

    LED1.GND
    C_LED.2

    J3.GND[*]

    TP_GND


# ============================================================
# LM5163 BUCK
# ============================================================

NET BUCK_SW
    U2.SW
    L1.1
    CBST.2
    RA.1

NET BUCK_BST
    U2.BST
    CBST.1

NET BUCK_RON
    U2.RON
    RRON.1

NET BUCK_RIPPLE
    RA.2
    CA.1
    CB.1

NET BUCK_FB
    U2.FB
    RFB_TOP.2
    RFB_BOT.1
    CB.2

NET +3V3
    L1.2
    COUT.1
    COUT_HF.1

    CA.2
    RFB_TOP.1

    U1.VDD_VDDA
    C_MCU.1
    C_MCU_BULK.1
    R_NRST.1

    U4.VS
    C_INA_SUPPLY.1
    R_ALERT.1

    LED1.VDD
    C_LED.1

    R_PD_SCL.1
    R_PD_SDA.1

    R_PGD.1
    R_BUCK_PG.1

    TP_3V3

NET BUCK_PGOOD
    U2.PGOOD
    R_BUCK_PG.2
    U1.PA7


# ============================================================
# HIGH-CURRENT PD BRANCH
# ============================================================

NET HS_SENSE
    RSH1.POWER_OUT
    Q1.DRAIN
    C_HS.1

    U3.SENSE              # Kelvin from RSH1 downstream pad
    R_INA_N.1             # Kelvin from RSH1 downstream pad

NET Q1_GATE
    U3.GATE
    Q1.GATE

NET PD_VIN_SW
    Q1.SOURCE
    J3.PD_VIN[*]
    U4.VBUS
    R_OUT.2

NET LMX_OUT
    U3.OUT
    R_OUT.1


# ============================================================
# LMX5069 PROGRAMMING
# ============================================================

NET HS_UVLO
    U3.UVLO
    RUV_TOP.2
    RUV_BOT.1
    Q2.D

NET HS_OVLO
    U3.OVLO
    ROV_TOP.2
    ROV_BOT.1

NET HS_TIMER
    U3.TIMER
    CTIMER.1

NET HS_PWR
    U3.PWR
    RPWR.1

NET PD_PGOOD
    U3.PGD
    R_PGD.2
    U1.PA3


# ============================================================
# FAIL-SAFE PD ENABLE
# ============================================================

NET KILL_GATE
    R_KILL_TOP.2
    R_KILL_BOT.1
    Q2.G
    Q3.D

NET PD_ENABLE
    U1.PA1
    Q3.G
    R_PD_EN_PD.1


# ============================================================
# INA237
# ============================================================

NET INA_INP_FILT
    R_INA_P.2
    U4.IN+
    C_INA_DIFF.1

NET INA_INM_FILT
    R_INA_N.2
    U4.IN-
    C_INA_DIFF.2

NET INA_ALERT
    U4.ALERT
    R_ALERT.2
    U1.PA2

# ============================================================
# LOCAL / DOWNSTREAM I2C1
# ============================================================

NET PD_I2C_SCL
    U1.PB6
    U4.SCL
    R_PD_SCL.2
    J3.PD_I2C_SCL

NET PD_I2C_SDA
    U1.PB7
    U4.SDA
    R_PD_SDA.2
    J3.PD_I2C_SDA


# ============================================================
# BACKPLANE CAN-FD
# ============================================================

NET CAN_TX
    U1.PA12
    U5.TXD

NET CAN_RX
    U1.PA11
    U5.RXD

NET CAN_STB
    U1.PA4
    U5.STB

NET CANH_BUS
    L3.CANH_BUS
    D2.IO1
    SJ1.1
    J1.4

NET CANL_BUS
    L3.CANL_BUS
    D2.IO2
    R26.2
    J1.3


# ============================================================
# LED
# ============================================================

NET LED_DATA_RAW
    U1.PA8
    R_LED.1

NET LED_DATA
    R_LED.2
    LED1.DIN


# ============================================================
# RESET / DEBUG
# ============================================================

NET NRST
    U1.NRST
    R_NRST.2
    C_NRST.1
    TP_NRST

NET SWDIO
    U1.PA13
    TP_SWDIO

NET SWCLK
    U1.PA14
    TP_SWCLK
```

---

# 17. Passive BOM

Commodity resistors/capacitors may use the assembler's preferred stocked LCSC basic part where the electrical requirements are met.

For high-voltage ceramics, shunt components, and timing/programming values, preserve the specified electrical characteristics.

## 17.1 Resistors

| Ref | Qty | Value | Tolerance / notes |
|---|---:|---:|---|
| RSH1 | 1 | **6 mΩ** | 1%, 3 W, low TCR |
| RRON | 1 | **27.4 kΩ** | 1% |
| RFB_TOP | 1 | **110 kΩ** | 1% |
| RFB_BOT | 1 | **62 kΩ** | 1% |
| RA | 1 | **110 kΩ** | 1% |
| R_BUCK_PG | 1 | **10 kΩ** | 1% |
| R_OUT | 1 | **100 Ω** | 1–5% |
| RUV_TOP | 1 | **110 kΩ** | 1% |
| RUV_BOT | 1 | **18 kΩ** | 1% |
| ROV_TOP | 1 | **110 kΩ** | 1% |
| ROV_BOT | 1 | **5.1 kΩ** | 1% |
| RPWR | 1 | **36.0 kΩ** | 1% |
| R_PGD | 1 | **10 kΩ** | 1% |
| R_KILL_TOP | 1 | **110 kΩ** | 1% |
| R_KILL_BOT | 1 | **36.0 kΩ** | 1% |
| R_PD_EN_PD | 1 | **100 kΩ** | 1–5% |
| R_INA_P | 1 | **10 Ω** | 1% |
| R_INA_N | 1 | **10 Ω** | 1% |
| R_ALERT | 1 | **10 kΩ** | 1% |
| R_NRST | 1 | **10 kΩ** | 1% |
| R_PD_SCL | 1 | **2.2 kΩ** | local I²C pull-up |
| R_PD_SDA | 1 | **2.2 kΩ** | local I²C pull-up |
| R26 | 1 | **120 Ω** | CAN termination, DNP unless enabled by SJ1 |
| R_LED | 1 | **100 Ω** | LED data series |

## 17.2 Capacitors

| Ref | Qty | Value | Rating / notes |
|---|---:|---:|---|
| CIN1 | 1 | **2.2 µF** | 100 V X7R |
| CIN2 | 1 | **2.2 µF** | 100 V X7R |
| C_BULK_OPTIONAL | 1 | **10–47 µF** | 100 V, DNP initially |
| COUT | 1 | **22 µF** | 25 V |
| COUT_HF | 1 | **100 nF** | >=10 V |
| CBST | 1 | **2.2 nF** | 50 V |
| CA | 1 | **2.2 nF** | 50 V |
| CB | 1 | **220 pF** | C0G, 50 V |
| C_HS | 1 | **1 µF** | 100 V X7R |
| CTIMER | 1 | **680 nF** | X7R, >=16 V, ~10% |
| C_INA_DIFF | 1 | **100 nF** | differential shunt-input filter |
| C_INA_SUPPLY | 1 | **100 nF** | local INA237 bypass |
| C_MCU | 1 | **100 nF** | MCU bypass |
| C_MCU_BULK | 1 | **4.7 µF** | MCU/local 3V3 bulk |
| C_NRST | 1 | **100 nF** | reset |
| C_LED | 1 | **100 nF** | LED local bypass |

---

# 18. Suggested LCSC parts for selected passives

These are convenience selections, not architectural requirements unless noted.

| Function | Suggested part | LCSC |
|---|---|---|
| 6 mΩ / 3 W shunt | HoLLR2512-3W-6mR-1% | **C2985709** |
| 68 µH inductor | YNR6045-680M | **C341069** |
| 2.2 µF / 100 V X7R | Yageo CC1206KKX7R0BB225 or equivalent | **C577211** |
| 22 µF / 25 V output MLCC | Samsung CL21A226MAYNNNE | **C602037** |
| 1 µF / 100 V X7R | Samsung CL31B105KCHNNNE | **C13832** |
| 680 nF timer capacitor | Walsin 0603B684K160CT | **C237234** |
| 2N7002 | commodity stocked 2N7002 | **C8545** |

For low-voltage generic 0603 resistors and decoupling capacitors, prefer existing BOM values/basic parts already used elsewhere in the product when electrically equivalent.

---

# 19. Connector interfaces

The existing carrier/backplane connector is a combined two-power-contact plus two-signal-contact interface. There is no separate upstream control connector and no backplane-provided 3.3 V rail.

## J1 — combined backplane interface

```text
VCC
GND
CANL_BUS
CANH_BUS
```

The power contacts and PCB copper shall support the 140 W architecture case and the short-duration current-limit current. Route CANL/CANH as a controlled, tightly coupled differential pair appropriate for the selected CAN-FD data rate and physical length.

The current carrier schematic assumes `pin 1 = VCC`, `pin 2 = GND`, `pin 3 = CANL`, and `pin 4 = CANH`, while the backplane-prototype schematic presently shows the two power nets in the opposite order. This may be a male/female footprint-numbering mirror. Before fabrication, verify the real mated contacts from manufacturer drawings and continuity, then make symbol pins, footprint pads, net assignments, and connector notes consistent across every carrier and backplane project. Do not infer polarity from an unlabeled PCB-side view.

## J3 — PD-module interface

```text
PD_VIN_SW       multiple contacts as required
GND             multiple contacts as required
PD_I2C_SCL
PD_I2C_SDA
+3V3_AUX        optional if useful
SPARE_GPIO0     optional
SPARE_GPIO1     optional
```

---

# 20. Expected startup sequence

```text
System VIN applied
    │
    ├── DTVS1 clamps transient events
    │
    └── LM5163 starts +3V3
             │
             ▼
        STM32 starts
             │
             │
             └── hardware kill logic keeps LMX5069 UVLO low
                  regardless of MCU reset/high-Z state
             │
             ▼
external controller eventually selects this module
             │
             ▼
upstream POWER_ON command
             │
             ▼
PD_ENABLE = HIGH
             │
             ▼
Q3 grounds KILL_GATE
             │
             ▼
Q2 releases HS_UVLO
             │
             ▼
LMX5069 validates VIN/UVLO/OVLO
             │
             ▼
Q1 turns on under current/power control
             │
             ▼
~300 µF PD input capacitance charges
             │
             ▼
PD_PGOOD asserts
             │
             ▼
STM32 probes PD module over I2C1
             │
       ┌─────┴─────┐
       │           │
    success      failure
       │           │
      ready     report / controlled retry
```

---

# 21. Fault behavior

## 21.1 Downstream short

Expected hardware sequence:

```text
downstream short
    │
    ▼
RSH1 differential voltage rises
    │
    ▼
LMX5069 enters current/power limiting
    │
    ▼
TIMER runs
    │
    ▼
fault persists beyond timer
    │
    ▼
LMX5069 shuts Q1 off
    │
    ├── PD_PGOOD deasserts to STM32
    │
    ▼
LMX5069 waits at its programmed low retry duty cycle
    │
    ├── PD_ENABLE remains HIGH → automatic retries continue
    │
    └── STM32 drives PD_ENABLE LOW → branch held OFF
```

Automatic retry is the accepted default. Firmware is not part of the primary protection loop; it only decides whether continued hardware retries are appropriate after observing `PD_PGOOD` and other telemetry.

## 21.2 MCU crash/reset

Hardware default:

```text
MCU PD_ENABLE LOW/high-Z
    │
    ▼
Q3 OFF
    │
    ▼
Q2 ON
    │
    ▼
UVLO forced low
    │
    ▼
PD branch OFF
```

## 21.3 Brownout

The PD branch should naturally disable if:

- VIN falls below programmed LMX5069 UVLO
- MCU housekeeping collapses and PD_ENABLE is lost

The branch should therefore fail toward OFF rather than partially commanded ON.

## 21.4 PD module communication failure

Firmware should be able to:

1. report an I2C1 failure upstream over CAN-FD
2. clear/reinitialize I2C1
3. disable `PD_ENABLE`
4. allow the PD module's onboard housekeeping supply to discharge `PD_VIN_SW`
5. re-enable the branch
6. wait for `PD_PGOOD`
7. reprobe the module

Keep the backplane CAN-FD interface operational while recovering the downstream I²C bus.

No dedicated bleeder is required on `PD_VIN_SW`. The PD module's onboard low-power supply remains connected to its input capacitors and discharges them to zero after Q1 turns off. Confirm the discharge time on the assembled module during bring-up; add no carrier bleeder unless that measurement disproves the assumption.

---

# 22. PCB layout requirements

These requirements are important enough to include as schematic notes.

## 22.1 High-current path

Keep the branch geometrically direct:

```text
J1.VIN
   ═════════ RSH1 ═════════ Q1 ═════════ J3.PD_VIN
```

Design for:

- >=6.5 A continuous
- >10 A short-duration limit/fault current
- minimum unnecessary neck-down
- multiple copper layers where practical
- adequate via arrays at layer transitions

## 22.2 Shunt Kelvin routing

RSH1 is physically a two-terminal power resistor, but its measurement connections must be routed as Kelvin senses.

High-current copper:

```text
VIN plane/pour ===== [ RSH1 ] ===== HS_SENSE power copper
```

Sense traces:

```text
inner edge VIN pad ───── U3.VIN
                   └──── R_INA_P

inner edge OUT pad ───── U3.SENSE
                   └──── R_INA_N
```

Do not pick up current-sense traces from arbitrary points in the surrounding pours.

## 22.3 Hot-swap component placement

Keep:

- RSH1
- U3
- Q1
- C_HS

physically close.

The pass-MOSFET gate and controller loops should be short and quiet.

## 22.4 Q1 thermal / SOA layout

Q1's normal conduction dissipation is small because its on-resistance is very low.

The important thermal event is linear-mode operation during:

- controlled startup
- overload limiting
- downstream short until timeout

Give the TO-263 drain/tab substantial copper and thermal vias.

Do not optimize Q1 layout based only on its steady-state conduction loss.

## 22.5 LM5163 layout

Critical converter loop:

```text
CIN → LM5163 switching stage → GND → CIN
```

Requirements:

- CIN immediately adjacent to VIN/GND
- short high-di/dt current loop
- small `BUCK_SW` copper area
- L1 immediately adjacent to SW
- feedback/ripple network on quiet side of L1
- keep `BUCK_FB` away from SW and gate-current paths
- solid ground reference under the converter where permitted by TI guidance

## 22.6 TVS placement

Place DTVS1 immediately behind J1.

Its return to ground must be:

- short
- wide
- direct

Do not route TVS surge current through narrow logic-ground sections.

## 22.7 Grounding

Use a continuous ground plane rather than artificially splitting logic and power grounds.

Route high branch current so its return path does not create narrow shared impedances underneath:

- MCU
- INA237
- feedback networks
- I²C

## 22.8 Existing-outline fit assessment

The present carrier outline is usable for the Rev-A electronics, but the result is constrained rather than spacious.

Relevant geometry from the current PCB:

| Region | Approximate geometry | Consequence |
|---|---:|---|
| Main board | **66.25 × 34.30 mm** (`x=115.50…181.75`, `y=96.35…130.65`) | Overall envelope remains unchanged. |
| PD-module front courtyard | **68.4 × 34.0 mm** (`x≈114.8…183.2`, `y≈95.8…129.8`) | It covers essentially the entire component side; do not plan normal top-side carrier components under the module. |
| Central PCB cutout | **~27.35 × 27.05 mm** (`x≈135.10…162.45`, `y≈98.84…125.89`) | Removes most of the center on both sides. |
| Left bottom-side lobe | **~19.6 × 34.3 mm**, before connector/pad keepouts | Primary candidate for TVS, shunt, LMX5069, Q1, and nearby high-current support parts. |
| Right bottom-side lobe | **~19.3 × 34.3 mm**, locally narrowed to **~12.2 mm** by the PD module's USB-C edge notch | Split into buck and MCU/monitor placement zones; use the neck primarily for routing. |
| Center top/bottom routing rails | approximately **2.5 mm top** and **4.8 mm bottom** around the cutout | Useful for signals and parallel copper assistance, not major component placement. |

Courtyard-level package rectangles fit with the following provisional bottom-side floorplan:

```text
BACKPLANE / XT30 END                                  USB-C END

┌──────── connector tongue ────────┬──────── main board ────────────────┐
│                                  │ LEFT LOBE    CUTOUT    RIGHT LOBE  │
│                                  │ TVS/RSH/LMX  ┌─────┐   BUCK block  │
│                                  │ Q1 D2PAK     │ PD  │   3V3 logic   │
│                                  │ gate support │module│   MCU/INA/LED │
│                                  │              └─────┘               │
└──────────────────────────────────┴─────────────────────────────────────┘
                                     all new parts on B.Cu side
```

Fit conclusion:

- **Yes:** the existing 2D outline is sufficient for a first 140 W-capable carrier layout if the new circuitry is placed predominantly on the bottom side.
- **No:** it is not credible as a top-side-only or two-layer implementation.
- Q1 and the ~6 × 6 × 4.5 mm buck inductor are the limiting package/height items. Before placement is frozen, confirm at least their bottom-side mechanical clearance in the rack assembly.
- Reserve Q1's entire available local lobe width for copper and thermal vias where possible. Passing a courtyard check alone is not sufficient thermal validation.
- Perform a real placement/routing review after the schematic is captured; the present result is a feasibility floorplan, not evidence that an unrouted board is production-ready.

## 22.9 Required stackup and copper

Rev A shall use a 4-layer, 1.6 mm stackup with 2 oz outer copper and 1 oz inner copper. The carrier PCB project has been converted to match the backplane-prototype stackup:

```text
L1 / F.Cu    2 oz    module pads, high-current pours, signals
L2 / In1.Cu  1 oz    solid GND
L3 / In2.Cu  1 oz    VCC / PD_VIN_SW high-current assistance
L4 / B.Cu    2 oz    new components, local pours, signals
```

The central cutout makes the remaining copper rails narrow. Use filled copper on every useful layer, parallel paths where the net permits, and dense through-via arrays at layer transitions. Validate finished copper thickness and the exact dielectric stack with JLCPCB at order time.

## 22.10 JLCPCB fabrication rules

`carrier.kicad_dru` ports the JLCPCB-specific custom rules from `backplane-prototype`:

- through vias only; blind, buried, and microvias disallowed
- 0.255 mm minimum PTH annular ring for the selected 2 oz outer-copper process
- 0.30 mm PTH hole clearance
- 0.45 mm pad-hole-to-pad-hole clearance
- minimum round NPTH and plated/non-plated slot sizes
- 1.0 mm / 0.15 mm minimum legend text size/thickness
- 0.15 mm minimum legend graphic width

The carrier project defaults were also aligned so newly created silk, text, clearances, holes, and vias start at compliant values.

The backplane's separate `13.0 mm` `HIGH_CURRENT_DC` track-width rule was **not** copied: it is a board-specific 30 A distribution rule, not a JLCPCB fabrication limit, and it is geometrically impossible on portions of this carrier. Define the carrier high-current nets explicitly and implement them as calculated multi-layer pours for the 6.5 A continuous / >10 A transient carrier requirement.

---

# 23. BOM consolidation opportunities

The design intentionally reuses common passive values.

### 110 kΩ

Can be shared by:

- RFB_TOP
- RA
- RUV_TOP
- ROV_TOP
- R_KILL_TOP

### 36 kΩ

Can be shared by:

- RPWR
- R_KILL_BOT

### 100 Ω

Can be shared by:

- R_OUT
- R_LED

### 10 kΩ

Can be shared by:

- BUCK_PGOOD pull-up
- PD_PGOOD pull-up
- INA_ALERT pull-up
- NRST pull-up

### 2.2 kΩ

Used for all I²C pull-ups.

### 2.2 nF

Can be shared by:

- LM5163 CBST
- LM5163 CA

### 100 nF

Can be reused for:

- MCU bypass
- INA237 supply bypass
- INA237 differential filter
- LED bypass
- NRST capacitor if using the same dielectric/voltage rating

This should help minimize unique feeder/pick-and-place setup count.

---

# 24. Remaining engineering concerns / validation items

## 24.1 Actual PD-module behavior during controlled ramp

The 300 µF measurement allows a good capacitor-only startup calculation.

However, the real PD module may begin switching before its input is fully charged.

A converter behaving approximately as a constant-power load during startup can make the hot-swap event more demanding than a passive capacitor.

Bench-test:

- 24 V startup
- 36 V startup
- 48 V startup
- no USB load
- moderate USB load
- maximum allowed USB load if the module permits it immediately after power-up

Capture:

- VCC
- HS_SENSE
- PD_VIN_SW
- Q1 VGS
- RSH1 differential voltage
- PD_PGOOD

Confirm Q1 remains within SOA and TIMER does not trip during valid startup.

## 24.2 CTIMER

`680 nF` is a good starting point for the measured ~300 µF module.

After capturing real startup waveforms, tune CTIMER if necessary.

The goal is:

- enough time to start normally over component/temperature/source variation
- not so much time that Q1 sits in linear-mode fault limiting unnecessarily long

## 24.3 Q1 procurement

Q1 is electrically conservative but may be one of the highest-cost components.

Before production:

- check LCSC inventory
- check price at production quantity
- evaluate equivalent 100 V hot-swap/linear-mode MOSFETs

Any replacement must be SOA-qualified, not merely a low-RDS(on) switching FET.

## 24.4 TVS transient margin

Validate SMCJ48A clamp behavior on actual hardware.

The INA237 85 V bus/common-mode maximum is one of the tightest upper-voltage constraints.

If board-level transients approach that value, revise:

- TVS
- upstream suppression
- monitor architecture
- or component voltage class

## 24.5 Input fuse / upstream protection

This design does **not** include a local fuse or reverse-polarity protector.

LMX5069 protects only the switched PD branch.

A short in:

- the LM5163 branch
- TVS
- raw VIN PCB copper

must be cleared by the upstream distribution protection.

Confirm the system/harness feeding J1 is appropriately fused.

If it is not, add a local input fuse.

## 24.6 Reverse polarity

No reverse-polarity protection is currently specified.

If the connector/harness makes reversed input physically possible, add appropriate protection.

## 24.7 I²C powered-off verification

The measured ~15 MΩ to GND is encouraging.

Before final release, verify:

- SDA high when PD module is unpowered
- SCL high when PD module is unpowered
- no measurable back-power
- no unexpected clamp diode conduction

If this test passes, the direct I²C connection in this handoff is appropriate.

## 24.8 Regulator design cross-check

The LM5163 values in this document are calculated starting values.

Before release, cross-check the final:

- input range
- expected minimum/maximum load
- switching frequency
- inductor
- Type-3 ripple network
- capacitor derating

against the latest TI design procedure / simulation tool.

## 24.9 Connector current rating

The selected physical power connector and contacts must carry:

- ~6.5 A continuous at the 24 V / 140 W corner
- >10 A briefly during current-limit/fault events

Do not assume a connector is suitable based solely on nominal family rating; account for:

- number of contacts paralleled
- PCB terminal temperature rise
- mating-cycle degradation
- ambient temperature
- enclosure airflow

## 24.10 Connector pin-map release check

Resolve the carrier/backplane power-pin mismatch called out on the carrier schematic before fabrication. This is a release blocker even if it is ultimately only a male/female footprint-view issue. Verify the physical mating contacts, then audit every symbol, footprint, and connector note across the carrier, backplane, and backplane-prototype projects.

## 24.11 Bottom-side mechanical clearance

The existing outline only fits the added electronics by using the bottom side. Check the assembled carrier against the rack, guides, neighboring boards, fasteners, and enclosure for clearance around:

- Q1 D²PAK body and solder fillet
- the approximately 4.5 mm-tall buck inductor
- TVS and shunt bodies
- programming pogo-pin access

If this clearance is unavailable, the carrier outline or mechanical stack must change; moving the circuitry to the module side is not supported by the current PD-module courtyard.

---

# 25. Firmware-visible hardware signals

Recommended firmware abstraction:

```text
PD_ENABLE       output   active high
PD_PGOOD        input    pulled high when branch healthy
INA_ALERT       input    open-drain interrupt/status
BUCK_PGOOD      input    housekeeping regulator status

CAN_TX/RX/STB   FDCAN    backplane CAN-FD interface through U5
PD_I2C_*        I2C1     downstream controller/master

LED_DATA        output   WS2812 status
```

Recommended branch state machine:

```text
OFF
 │
 │ POWER_ON
 ▼
ENABLING
 │
 ├── PGOOD timeout ───────────► FAULT
 │
 └── PGOOD asserted
        │
        ▼
   PROBING_PD
        │
        ├── I2C failure ──────► FAULT
        │
        └── success
             │
             ▼
            ON

FAULT
 │
 ├── report status upstream
 ├── optionally sample INA237
 └── disable/retry according to policy
```

Hardware protection must remain autonomous; firmware should observe and orchestrate but not replace current-limit or short-circuit protection.

---

# 26. References

The KiCad engineer should use the latest manufacturer datasheets for final pin/footprint/layout verification.

Primary references:

- **Wuxi Maxinmicro LMX5069** datasheet
- **Texas Instruments LM5163 / LM5163-Q1** datasheet
- **Texas Instruments INA237** datasheet
- **STMicroelectronics STM32G031x4/x6/x8** datasheet
- **Infineon IPB020N10N5LF** datasheet / OptiMOS Linear FET SOA documentation
- **Worldsemi WS2812B-2020-V6** datasheet
- LCSC catalog entries corresponding to the LCSC codes listed in this document

---

# 27. Summary of frozen Rev-A decisions

Power-generation envelope:

```text
Current product limit       100 W
Rev-A carrier architecture  140 W capable
Future hardware target      240 W capable; new hardware review required
```

Backplane/carrier interface:

```text
J1 = VCC + GND + backplane CANL/CANH
No backplane 3.3 V rail
Carrier generates local 3.3 V with LM5163
```

The current intended architecture is:

```text
24–48 VDC
   │
   ├── SMCJ48A input TVS
   │
   ├── LM5163 100 V buck ──► 3.3 V
   │                         ├── STM32G031
   │                         ├── INA237
   │                         ├── WS2812B-2020-V6
   │                         └── I²C pull-ups
   │
   └── 6 mΩ Kelvin shunt
          │
          ├── INA237 current sense
          │
          └── LMX5069 hot-swap controller
                    │
                    └── 100 V linear-mode N-MOSFET
                              │
                              └── PD_VIN_SW ──► USB-C PD module
```

Key programmed values:

```text
RSHUNT       = 6 mΩ / 3 W / 1%
Current limit:
    min      ≈ 7.9 A
    typ      ≈ 9.2 A
    max      ≈ 10.9 A

RPWR         = 36 kΩ
Q1 power limit estimates at 48 V:
    conservative minimum ≈ 41 W
    inferred typical     ≈ 54 W
    inferred maximum     ≈ 66 W

Measured PD CIN = ~300 µF

CTIMER       = 680 nF
fault timeout ≈ 35 ms typical
fault timeout ≈ 20 ms conservative minimum
fault timeout ≈ 66.5 ms conservative maximum
expected PD startup ≈ 8–9 ms typical at 48 V

UVLO rising  ≈ 19.8 V
UVLO falling ≈ 17.8 V

OVLO rising  ≈ 56.4 V
OVLO falling ≈ 54.4 V

Housekeeping:
    3.33 V
    LM5163
    ~300 kHz
    68 µH

INA237:
    address default 0x40
    ADCRANGE = ±163.84 mV

PD power:
    fail-safe OFF
    active-high MCU enable
    sequentially enabled by external controller
    LMX5069 automatic retry enabled by default
    STM32 observes PD_PGOOD and may hold PD_ENABLE low

PCB:
    existing outline retained
    new circuitry predominantly on B.Cu side
    4 layers, 2 oz outer / 1 oz inner copper
    JLCPCB custom fabrication rules enabled

Discharge:
    no dedicated bleeder
    PD module housekeeping supply discharges PD_VIN_SW capacitors
```
