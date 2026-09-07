# Carrier Datasheet Archive Audit

**Audit date:** 2026-09-06

**Scope:** Current carrier schematic and generated production BOM compared with
`docs/data-sheets/`. The PCB and production outputs use the common LM5164 and
100 µH inductor. `J1` and `MOD1` are intentionally user-installed and are
listed separately from assembly-populated parts.

## Remaining release-critical exact-part gap

| Ref | Populated part | Local status | Action |
|---|---|---|---|
| C8 | Samsung `CL21A226MAQNNNE` | Near-match only. The local PDF is `CL21A226MAYNNNE`. | Add the exact `...MAQ...` PDF or formally document suffix equivalence. |

## Other carrier gaps or near-matches

| Refs | Populated part | Local status / action |
|---|---|---|
| C1 | Samsung `CL31B105KCHNNNE` | Exact PDF missing. |
| C2 | Fenghua `0603B684K160NT` | Exact PDF missing; this part sets the hot-swap TIMER interval. |
| C3, C4 | Samsung `CL32B225KCJSNNE` | Exact PDF missing. `IHHEC/C1206X225K101T.pdf` is a different part used elsewhere and is not a carrier substitute. |
| C5, C6 | Fenghua `0603B222K500NT` | Exact PDF missing. |
| C7 | Yageo `CC0603JRNPO9BN221` | Exact C0G/NP0 PDF missing; the local Yageo 100 nF X7R PDF is unrelated. |
| C9-C12, C14-C18 | Yageo `CC0603KRX7R9BB104` | Local `CC0603KRX7R0BB104.pdf` is a different exact MPN; treat it as family-level only. |
| C13 | Samsung `CL10A475KO8NNNC` | Exact PDF missing. |
| Q2, Q3 | `2N7002`, LCSC `C8545` | Nexperia family PDF is useful, but the BOM does not identify Nexperia as the populated manufacturer. |
| R2-R19, R24-R26, R28, R30-R32 | Uni-Royal `0603WAF...` series | Exact series PDF missing. Local `YAGEO/RC0603FR.pdf` is a different manufacturer/family. |

## Adequate local coverage

- D1: Hongjiacheng `SMCJ48A.pdf` is the exact manufacturer family datasheet
  covering the populated `SMCJ48A`, LCSC `C19077611`.
- D2: Nexperia `PESD2CANFD27V.pdf`; the populated `-TR` suffix appears to be
  packaging.
- L1: PROD Tech `PSPMAA0805-101M-ANP.pdf` covers the exact 100 µH value and
  its PSPMAA0805 family land pattern.
- L3: TDK `ACT1210D.pdf`, covering the populated
  `ACT1210D-101-2P-TL00` family/value.
- LED1: Worldsemi `WS2812B-2020-V6.pdf`.
- U1: retained `stm32c092fc.pdf` covers the STM32C091/092 xB/xC family,
  including the populated `STM32C092GCU6`; the byte-identical
  `stm32c092fb.pdf` duplicate was removed.
- U2: TI `LM5164.pdf` is the exact family datasheet for planned
  `LM5164DDAR`.
- U3: Wuxi Maxinmicro `LMX5069MS.pdf`.
- U4: TI `INA237.pdf` covers populated `INA237AIDGSR`.
- U5: TI `TCAN3413.pdf`, covering TCAN3413/TCAN3414.
- Q1: Infineon `IPB020N10N5LF.pdf` is the exact MOSFET/SOA datasheet.
- R1: Milliohm `C2985709.pdf` covers the populated
  `HoLLR2512-3W-6mR-1%`, including TCR, power derating, overload tests, and
  the recommended Kelvin-sense land pattern.
- J1: Amass `XT30PW(2+2)-M.G.B.pdf`.
- SW1: ALPS Alpine `SKSCLCE010.pdf`.

## User-installed and DNP items

- `J1` has exact local coverage and is intentionally soldered by the user.
- `MOD1` has SW3538 controller-chip documentation, but no exact module-vendor
  mechanical/electrical datasheet. Add the module drawing if one is available.
- `J2` (`TC2030-IDC-NL`) has no exact local Tag-Connect datasheet; add it if the
  DNP footprint is used or mechanically relied upon.

## Files that are not carrier-specific

Do not delete these merely because the carrier does not use them. Repository
searches show that most serve another board or historical design:

- `JUXING/SMCJ48A.pdf` documents the previously selected D1 alternate. The
  populated carrier part is now Hongjiacheng `C19077611`, whose exact family
  datasheet is archived separately.
- `TI/LMR516xx.pdf` must remain for the backpack's populated
  `LMR51610YDBVR` regulators. LM5164 is not a substitute for LMR51610.
- `BOURNS/SRN6045TA.pdf`, `SAMSUNG/CL21A226MAYNNNE.pdf`,
  `IHHEC/C1206X225K101T.pdf`, and `WS/WS2812B-MINI-V6_V1.2_EN.pdf` are also
  used by backpack designs.
- `Alpha and Omega/AO3407A.pdf` is used by the backplane-backpack.
- `Amass/XT30U(2+2)-F.G.B.pdf`, `DEGSON/ DG135T-C708738.pdf`,
  `Nexperia/PESD5V0S2BT.pdf`, `TI/PCA9554.pdf`, and `TI/TCA9548A.pdf` are
  used by backplane/prototype designs or their documentation.
- STM32G031/reference-manual files remain historical or non-carrier material;
  they must not be cited as STM32C092 carrier references.
- `Changzhou Amass Elec/XT30-family.pdf` has no direct current-design reference
  and is the clearest optional archive candidate, but it may still be useful as
  family-level connector documentation.

The obsolete carrier-only `TI/LM5163.pdf` and
`Sunlord/SWPA8040S680MT.pdf` files were removed with the BOM consolidation.
The safest remaining cleanup is therefore to label board scope and
exact-versus-family coverage and add the exact C8 PDF or document its suffix
equivalence. Broad deletion is not recommended.
