# SW3538 USB-C/PD Module Power-Input and Thermal Budget

> **Active product decision:** The SW3538 carrier profile caps its output at
> 20 V, 5 A, and 100 W. EPR and the SW3538's proprietary 20 V/7 A mode are not
> supported. The carrier input path is intended for a little over 5 A.
> The 140 W/7 A calculations below document the device/module claim and why it is
> rejected; they are not the active firmware or system requirement.

## Bottom line

The SW3538 datasheet defines the advertised maximum output as **140 W at 20 V × 7 A**. This rating applies to the USB output side.

The SW3538 is a synchronous **buck converter** with a recommended 5–36 V input range. With a 24 V DC supply, the input current will therefore be lower than 7 A.

That IC range is not the system input specification: the backplane/system is
designed for **24–48 V nominal**, while this carrier population is limited to
**24 V nominal, 30 V maximum input**. A second-generation carrier and suitably
rated module/protection are required for 48 V use. The lower-voltage IC test
conditions and module observations below do not establish a below-24 V nominal
backplane requirement or guarantee carrier startup at those voltages.

For power-distribution design, a conservative allowance per module is:

- **Input power:** approximately **160 W**
- **Continuous input current at 24 V:** approximately **6.5 A**
- **PCB branch design current:** at least **7 A continuous**, preferably with components and copper rated for **8 A**
- **Thermal dissipation:** design around **15 W per module**

Reference: [SW3538 datasheet](https://ismartware.com/upload/goods/20220811/202208111603413353.pdf)

---

## Input-current calculation

Use:

\[
P_\text{in}=\frac{P_\text{out}}{\eta}
\]

\[
I_\text{in}=\frac{P_\text{out}}{V_\text{in}\eta}
\]

\[
P_\text{loss}=P_\text{in}-P_\text{out}
\]

For 140 W output from a 24 V bus:

| Assumed efficiency | Input power | 24 V input current | Module power loss |
|---:|---:|---:|---:|
| 100% theoretical | 140.0 W | 5.83 A | 0 W |
| 95% | 147.4 W | 6.14 A | 7.4 W |
| 94% | 148.9 W | 6.21 A | 8.9 W |
| 92% | 152.2 W | 6.34 A | 12.2 W |
| 90% | 155.6 W | 6.48 A | 15.6 W |

For electrical and thermal planning, **90–92% efficiency** is a reasonable conservative estimate, even though a particularly good module may perform better.

The SW3538 datasheet mentions efficiency above 95%, but at a substantially different operating point: 12 V input, 5 V output, and 5 A output—25 W rather than 140 W. It does not publish an efficiency curve for 24 V input, 20 V output, and 7 A load.

An independently tested large SW3538 module measured above 91% efficiency at a 100 W load and remained stable during a 30-minute test. A smaller module using the same IC measured above 92% but thermally reduced its output from 100 W to about 90 W after 15 minutes. This illustrates why the module’s MOSFETs, inductor, PCB, heatsink, firmware, and airflow matter as much as the IC itself.

Reference: [TinkerVault SW3538 module testing](https://www.tinkervault.com/usb-power-sources/dc-pd30-adapters)

---

## Important USB-PD qualification

The module’s **20 V × 7 A mode is not standards-compliant USB Power Delivery**.

USB-IF states that before USB PD 3.1, standard USB PD was limited to **100 W at 20 V × 5 A**. Standard USB PD 3.1 reaches 140 W using **28 V × 5 A**, not 20 V × 7 A.

Reference: [USB-IF USB Charger and USB Power Delivery information](https://www.usb.org/usb-charger-pd)

Therefore:

- **SW3538 “140 W”** = proprietary or nonstandard **20 V × 7 A**
- **Standard USB PD 3.0 maximum** = **20 V × 5 A = 100 W**
- **Standard USB PD 3.1 140 W** = **28 V × 5 A**

Because the SW3538 is a buck-only converter and the input supply is 24 V, it cannot generate the standard 28 V EPR output.

With ordinary standards-compliant PD devices and cables, the practical maximum is therefore likely to be **100 W**, unless the sink and cable specifically support the module’s proprietary 7 A mode.

At 100 W output:

| Assumed efficiency | Input power | 24 V input current | Module power loss |
|---:|---:|---:|---:|
| 92% | 108.7 W | 4.53 A | 8.7 W |
| 90% | 111.1 W | 4.63 A | 11.1 W |

A practical standard-PD budget would consequently be about **120 W and 5 A per module**, with branch hardware rated closer to 6 A.

---

## The IC marking does not prove the board supports 140 W

Not every SW3538 module actually advertises 20 V at 7 A.

In one module comparison:

- A larger black SW3538 board exposed a 20 V/7 A PDO when supplied above approximately 21 V.
- A smaller blue board using the same IC exposed only 20 V/5 A despite being marketed as 140 W.

Reference: [TinkerVault SW3538 module comparison](https://www.tinkervault.com/usb-power-sources/dc-pd30-adapters)

The reliable way to determine the capability of a particular module is to inspect its advertised source PDOs with a PD analyzer or programmable USB-C load:

- **20 V, 7 A advertised:** design for the 160 W / 6.5 A case.
- **20 V, 5 A maximum:** design for approximately 112–120 W / 4.7–5 A.
- **Only 20 V, 3.25 A:** the module is effectively a 65 W source.

The 140 W rating is also a **single USB-C-port maximum**. The SW3538’s default dual-port behavior does not mean 140 W is available from USB-C while additional full power is simultaneously available from USB-A. When both ports are occupied, the standard control mode reduces the available output modes.

Reference: [SW3538 datasheet](https://ismartware.com/upload/goods/20220811/202208111603413353.pdf)

---

## Recommended PCB and supply sizing

For an unknown “140 W” SW3538 module, use:

\[
I_{\text{branch,max}} =
\frac{140}{0.90 \times V_{\text{bus,min}}}
\]

At exactly 24.0 V:

\[
I_{\text{branch,max}}=
\frac{140}{0.90 \times 24.0}
=6.48\text{ A}
\]

If the nominal 24 V bus can fall by 5% to 22.8 V:

\[
I_{\text{branch,max}}=
\frac{140}{0.90 \times 22.8}
=6.82\text{ A}
\]

Recommended design values:

| Item | Recommended design value per module |
|---|---:|
| Allocated input power | 160 W |
| Expected maximum continuous current | 6.5–6.8 A |
| Minimum branch current rating | 7 A |
| Preferred connector and trace rating | 8 A or greater |
| Thermal-removal capability | Approximately 15 W |
| Input voltage at module under load | Keep comfortably above 21 V |

For \(N\) modules operating simultaneously:

\[
P_\text{load}\approx N \times 160\text{ W}
\]

Then add power-supply operating margin.

For example, four fully loaded modules represent approximately:

\[
4 \times 160=640\text{ W}
\]

A supply around **700–750 W at 24 V** would provide reasonable operating headroom.

---

## Distribution resistance and voltage drop

Keep the power-distribution resistance low.

At 6.5 A, every **10 mΩ** of combined trace, connector, fuse, and wiring resistance produces:

\[
P=I^2R
\]

\[
P=6.5^2 \times 0.010
\approx 0.42\text{ W}
\]

The corresponding voltage drop is:

\[
V=IR
\]

\[
V=6.5 \times 0.010
=0.065\text{ V}
\]

So each 10 mΩ in the power path causes approximately:

- **0.42 W of heat**
- **65 mV of voltage drop**

Several connectors, fuses, and narrow PCB sections can therefore add meaningful heat and reduce the voltage margin available to the buck converter.

---

## Practical design recommendation

For each SW3538 module supplied from a nominal 24 V bus:

1. Allocate **160 W** of supply capacity.
2. Design the branch for at least **7 A continuous**.
3. Prefer traces, connectors, fuses, and switches rated for **8 A or more**.
4. Provide thermal management capable of removing approximately **15 W** from the module.
5. Account for supply tolerance and wiring drop so the module remains comfortably above approximately **21 V** at full load.
6. Verify the actual module PDOs before assuming that it can advertise or sustain 20 V at 7 A.
7. Treat 20 V at 7 A as proprietary rather than standard USB-PD operation.

For conventional, standards-compliant 100 W USB-PD operation, a lower design target of approximately **120 W and 5 A per module** is generally sufficient, while still leaving useful margin.
