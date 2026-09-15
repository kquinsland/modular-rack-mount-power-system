# Project 3D Models

## `TDK_ACT1210D-101-2P-TL00.{wrl,step}`

EasyEDA/LCSC mechanical model pair for the TDK ACT1210D-101-2P-TL00
common-mode choke, imported from LCSC part C3039743 with `easyeda2kicad`
1.0.1. The colored WRL is used by the `L_CommonModeChoke_TDK_ACT1210D`
footprint; the same-basename STEP is available for KiCad's STEP-model
substitution during board export.

`easyeda2kicad` normalizes the OBJ-derived WRL to sit on Z=0 but copies the
catalog STEP unchanged. The catalog STEP placed the component below Z=0, so
the repository STEP has its Z geometry mirrored to match the WRL. The
footprint rotates the pair 180 degrees to match this project's pad numbering.

## `ACT12_Choke.step`

Earlier project-local mechanical model for the TDK ACT1210D common-mode choke.
It is retained for reference but is no longer associated with the maintained
footprint or carrier PCB.

## `SKSCLCE010.step`

Project-local mechanical model for the Alps Alpine SKSCLCE010 side-push
tactile switch. It is used by the `SW_ALPS_SKSCLCE010_SidePush` footprint.

## `ST_UFQFPN-28_4x4mm_P0.5mm.step`

Project-local mechanical model for the STM32G031G6U6 UFQFPN-28 package.
The model uses the nominal package dimensions from STMicroelectronics
[datasheet DS12992 Rev 4](https://www.st.com/resource/en/datasheet/stm32g031f6.pdf),
Table 77:

- 4.0 mm x 4.0 mm body
- 0.55 mm overall height
- 28 terminals on 0.50 mm pitch
- 0.25 mm nominal terminal width
- no exposed center pad

This model is kept in the repository because KiCad 10's stock
`Package_DFN_QFN` footprint references
`QFN-28_4x4mm_P0.5mm.step`, but that model is not present in the KiCad 10
3D model library.

## `Worldsemi_WS2812B-4020_SideView.wrl` / `.step`

Colored EasyEDA housing model for the side-view 4020 LED used by LED1 on the
carrier and backplane. The exact V6 listing, LCSC C52941387, has no EasyEDA 3D
model, so this uses the same-package WS2812B-4020 model from
[LCSC C965557](https://www.lcsc.com/product-detail/C965557.html), exported with
`easyeda2kicad` 1.0.1. This is a mechanical visualization substitute only; the
schematic, BOM, electrical pinout, and land pattern still specify the V6 part.
The V6 manufacturer drawing is stored in
[`WS2812B-4020-V6.pdf`](../../../docs/data-sheets/WorldSemi/WS2812B-4020-V6.pdf).

- EasyEDA model UUID: `12fb8a9d7ac5442fbdb526964b6e9ee0`.
- Side-view mounted envelope: 3.98 mm X × 1.70 mm Y × 2.00 mm Z.
- Emitting face points along model +Y (footprint -Y); no axis rotation or scaling.
- Both formats are centered in XY, with the mounting face at Z = 0.
- WRL is the colored render model; same-basename STEP supports mechanical export.

Retrieve with `mise exec -- easyeda2kicad --lcsc_id C965557 --3d --output /tmp/led`.
EasyEDA's raw STEP origin is translated by (+2.14, +0.85, +1.55) mm. The WRL
exporter separately bakes a footprint-origin offset into its coordinates; recenter
that mesh in XY and put its minimum Z at zero as well. STEP normalization used
CadQuery 2.6.1; its exported geometry is uncolored. Do not rotate the housing to
make the 2 mm dimension the footprint depth: that would turn the emitting face
away from its side-view orientation.

The obsolete 2 × 2 mm LED model was removed. Historical release snapshots that
still refer to it must be viewed from their original Git revision.
