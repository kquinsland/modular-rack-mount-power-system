# Node Commissioning and Carrier Binding

Every STM32 supplies an immutable 96-bit UID. A commissioned node additionally
stores one mutable Node ID from 1 through 254. Node 0 is reserved for broadcast;
255 is invalid.

## Discovery

`pdcan scan` broadcasts a nonce. Every node replies directly with its UID,
optional Node ID, commissioning state, protocol version, role, carrier profile,
hardware revision, capabilities, update impact, firmware version, bootloader
version, and partition-layout version. Discovery works before commissioning.

```sh
pdcan scan --interface can0
pdcan identify 00112233445566778899aabb --seconds 10
pdcan assign 00112233445566778899aabb 17
pdcan clear-node 00112233445566778899aabb
```

Assignments and clears are addressed by immutable UID and carry a request ID.
Nodes periodically claim their configured address. If two distinct UIDs claim
the same Node ID, each enters `AddressConflict`, suppresses ordinary operational
traffic, remains safe, and continues commissioning traffic so the conflict can
be corrected.

## Optional carrier binding

A carrier may persist `{backplane_uid, slot_index}` as descriptive metadata:

```sh
pdcan bind 17 aabbccddeeff001122334455 0
pdcan clear-binding 17
```

Slot indexes are zero-based. The backplane UID remains stable when its Node ID
changes. Binding does not route messages, grant authority to the backplane, or
prove physical placement. Host software may resolve a human-facing
`backplane.slot` alias to the carrier's current Node ID after discovery.
