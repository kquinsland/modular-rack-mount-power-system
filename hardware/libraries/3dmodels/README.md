# Project 3D Models

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

## `Worldsemi_WS2812B-2020-V6.step`

Project-local mechanical model for the Worldsemi WS2812B-2020-V6 addressable
LED. The model uses the nominal dimensions from the Worldsemi V6 datasheet in
[`docs/data-sheets/WorldSemi/WS2812B-2020-V6.pdf`](../../../docs/data-sheets/WorldSemi/WS2812B-2020-V6.pdf):

- 2.20 mm x 2.00 mm overall package
- 0.84 mm overall height
- 0.28 mm substrate height
- 1.13 mm center body width
- four side contacts aligned with the footprint pads

This model is kept in the repository because KiCad's
`LED_WS2812B-2020_PLCC4_2.0x2.0mm` footprint references a matching model name,
but that model is not present in the KiCad 3D model library.
