# Assembly Part Choices

This document records provisional component choices where JLCPCB assembly-library
classification affects cost or availability. Catalog classifications, stock, and
promotions are temporary observations, not design guarantees. Recheck every part
in JLCPCB's BOM-matching step when an assembly order is prepared.

## I2C ESD Protection

### Requirements

The current PCB still has one legacy SOD-523 TVS position on each slot-local SDA
and SCL line. The interim schematic instead combines each slot's pair into one
dual-line C7519 array. An appropriate device should have:

- A reverse working voltage safely above the 3.3 V I2C signal; 5 V is a suitable
  target.
- Low junction capacitance, preferably no more than a few picofarads.
- IEC 61000-4-2 ESD protection.
- A clamping voltage comparable to the candidate parts below.
- A verified symbol, footprint, pinout, and polarity before release for
  manufacture.

### USBLC6-2SC6 Reference

STMicroelectronics `USBLC6-2SC6` (`C7519`) is a useful performance reference. It
protects two data lines and VBUS, has 3.5 pF maximum line capacitance, and uses a
SOT-23-6L package. ST specifies 12 V maximum clamping at 1 A and 17 V at 5 A.

This is not a direct footprint replacement for a single-line SOD-523 device. As
observed on 2026-07-11, JLCPCB classified `C7519` as Extended rather than Basic.

The backplane schematic currently uses `C7519` as the interim selection:

- `U2` through `U7`, one dual-line array per slot.
- SDA and SCL use the device's flow-through I/O pin pairs.
- `VBUS` is tied to `+3V3` and the ground pin is tied to `GND`.
- The schematic carries `LCSC=C7519`, `MPN=USBLC6-2SC6`, and the
  `Package_TO_SOT_SMD:SOT-23-6` footprint assignment.

The backplane PCB has not yet been updated from this schematic and still has the
former individual SOD-523 footprints. It must be synchronized before layout or
manufacture.

Sources:

- [STMicroelectronics USBLC6-2 product page](https://www.st.com/en/protections-and-emi-filters/usblc6-2.html)
- [JLCPCB C7519 listing](https://jlcpcb.com/parts/componentSearch?isSearch=true&searchTxt=USB)

### Strict Basic-Part Search

On 2026-07-11, the JLCPCB catalog returned only three strict Basic parts in its
ESD and surge-protection category. None is a close replacement for
`USBLC6-2SC6`.

| JLCPCB part | Manufacturer part | Package | Assessment |
| --- | --- | --- | --- |
| `C32677` | `PSM712-LF-T7` | SOT-23 | The nearest Basic option in broad function, with two protected I/O pins and a common ground. It is an asymmetric 7 V/12 V, 75 pF device intended for RS-485. It is neither pin-compatible nor a good low-capacitance, low-voltage substitute. |
| `C78395` | `P6SMB6.8CA/TR13` | SMB (DO-214AA) | A two-pin, 600 W power TVS rather than a low-capacitance signal-protection array. |
| `C7420377` | `SMBJ6.5CA` | SMB (DO-214AA) | Also a two-pin, 600 W power TVS and not a practical substitute for the existing signal-line protection. |

Sources:

- [JLCPCB C32677 listing](https://jlcpcb.com/partdetail/ProTekDevices-PSM712_LFT7/C32677)
- [ProTek PSM712 documentation](https://protekdevices.com/products/series-details/?id=5094)
- [JLCPCB C78395 listing](https://jlcpcb.com/partdetail/Brightking-P6SMB6_8CATR13/C78395)
- [JLCPCB C7420377 listing](https://jlcpcb.com/partdetail/hongjiacheng-SMBJ65CA/C7420377)

### Provisional Preferred Candidate

`C20617921`, Hongjiacheng `H5VUD5U`, remains a possible low-cost alternative if
the design returns to individual protection diodes:

- One device per SDA or SCL line.
- SOD-523 package, matching the package chosen for the current placeholders.
- 5 V reverse working voltage.
- 0.8 pF maximum junction capacitance.
- 10 V clamping at 1 A and 15 V at 5 A.
- IEC 61000-4-2 and IEC 61000-4-5 protection.
- Unidirectional; its cathode must connect to the signal and its anode to ground.

On 2026-07-11, JLCPCB's catalog API marked this part as Preferred/Promotional
Extended. JLCPCB currently exempts that class from the feeder-loading fee for
Economic PCBA, although the individual part page still labels it Extended.
Neither the promotion nor stock should be assumed to remain available.

Sources:

- [JLCPCB C20617921 listing](https://jlcpcb.com/partdetail/hongjiacheng-H5VUD5U/C20617921)
- [H5VUD5U manufacturer datasheet](https://atta.szlcsc.com/upload/public/pdf/source/20250709/A4AFC60C313F49F04BEE80399A581AAE.pdf)
- [JLCPCB Basic and Promotional Extended library](https://jlcpcb.com/parts/basic_parts)

### Ordering-Time Decision

Do not make the PCB design depend on `C20617921` retaining promotional status.
When preparing an order:

1. Recheck `C20617921` stock, price, assembly support, and feeder-fee status in
   the actual BOM-matching workflow.
2. Recheck `C7519` stock, price, and assembly classification as the current
   schematic choice.
3. If changing from `C7519` to `C20617921` or another single-line device, update
   both the schematic and PCB; it cannot be substituted into the SOT-23-6
   footprint.
4. If no promotion remains attractive, prefer the technically suitable device
   and account for the Extended-part fee rather than weakening the protection
   design to preserve an expired deal.
5. Recheck the datasheet, polarity, land pattern, and actual clamping test
   conditions before finalizing the BOM.
