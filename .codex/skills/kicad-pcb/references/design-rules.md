# Project Design-Rule Workflow

Design rules are project evidence, not generic prose defaults. Derive them from
the exact design requirements, component datasheets, the selected fabricator's current contract,
ordered stackup, and accepted electrical calculations.

## 1. Capture rule provenance

Record the source and retrieval date for:

- trace, space, annular-ring, drill, slot, and copper-to-edge limits;
- mask, paste, silkscreen, and courtyard constraints;
- layer count, copper thickness, dielectric stackup, and impedance service;
- voltage-clearance, creepage, current, thermal, and mechanical requirements;
- assembly, test, panelization, and enclosure constraints.

Use the strictest applicable requirement. A capability advertised for another
service tier or stackup does not authorize the selected order.

Completion criterion: every configured rule has a current source or calculation,
and every applicable requirement has a project rule or explicit review check.

## 2. Encode the accepted values

Use `set_design_rules` for board-wide minima and `get_design_rules` to read back
what was stored. Use `create_netclass` for electrical groups and
`assign_net_to_class` for exact net membership. Use `set_predefined_sizes` for
the accepted trace/via palette.

- `set_design_rules` writes the board-wide minimums into the project file,
  where KiCad's DRC reads them. `min_via_drill` is KiCad's minimum hole size:
  it applies to every drilled hole, pad holes included, not only to vias. A
  value of 0 means no minimum; a negative value is refused, because KiCad would
  silently stop checking. It refuses while KiCad holds the board open, since
  KiCad's next save rewrites the project file.
- `set_layer_constraints` sets a minimum track width or clearance for one
  copper layer as a rule in the board's custom-rules file (`.kicad_dru`),
  named `konnect:LAYER:track_width` or `konnect:LAYER:clearance`.
- In a custom-rules file, a later rule that sets the same constraint for the
  same items wins. The tool reports such rules in `later_rules`; review them
  instead of assuming Konnect's rule applies.
- KiCad ignores a custom-rules file it cannot parse entirely, and silently:
  one missing parenthesis, a missing `(version 1)` first line, or a `#`
  comment after an item on the same line drops every rule in it, and DRC
  still passes. The tool refuses to write into such a file; fix it first.

The project netclasses are the source of truth for routing widths, clearances, and
via geometry. Name classes by purpose—ordinary signal, current-carrying rail,
controlled-impedance interface, high-voltage isolation—rather than copying an
undated vendor table.

Completion criterion: readback matches the accepted rule record, and every
special net is assigned to the intended class.

## 3. Verify the effective design

After encoding the values, re-run DRC after rule changes and after placement, routing, zone, or outline
changes. Resolve every error or record a deliberate waiver tied to the governing
requirement. For controlled impedance and current-carrying nets, reconcile DRC
with the calculation record; DRC only proves compliance with the values it was
given.

Before manufacturing, compare project rules with the final selected order and
stackup again. Any missing source, mismatched readback, unreviewed DRC result, or
contract drift makes the rule set `INCOMPLETE`.
