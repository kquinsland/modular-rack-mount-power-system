# TODOs

- Fix production issues
  - [ ] Backplane:
    - [X] R10 pulls an STM32 input to 12 V. Fixed on schematic, but PCB needs to be updated.
    - [ ] U5 needs better local VCC bypassing. C20 serves the VIO side; another nearby 100 nF at VCC/
    GND is advisable.
    - [ ] Fix/re-adjust the layout after new schematic changes for ripple injection network.
  - [ ] Carrier:
    - [X] C17 is approximately 12 mm from LED1. Move it beside the LED’s supply pins.
  - [ ] BOTH:
    - [ ] INA current sens traces need to be isolated from other pours!
    - [ ] Import the PCBWay logo and `waywayway` placeholder into the silkscreen layer for the backplane and carrier, get working with KiBot export pipe

- [ ] Finish migrating to konnect
  - [ ] Add the binary/plugin to the mise toolchain
  - [ ] Verify that it works in a new `codex` session / install the skills/MCP config local to this repo
  - [ ] Add the plugin to KiCad 10's plugin manager / both projects
    - do I have to add the zip file or is there a repo/url I can add?
      - If manual, `mise`?

- Set up Hugo/Docs
  - And wire up release generation (heat/thermal/iBom)
- Figure out panelization / validation tooling

- Docs cleanup
  - Consolidate various audit/handoff docs
    - Some things can be ADRs, some things are no longer needed

- Firmware
  - rebuild all of it now that PCBs are consolidated and there's a single STM32 in the mix