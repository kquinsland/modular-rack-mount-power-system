# UUIDv8 as the PDCAN SMBus-ARP Identity

**Audience:** Firmware engineer  
**Status:** Proposed protocol decision / implementation handoff  
**Scope:** Upstream discovery and dynamic I²C address assignment for STM32G031 carrier modules

## 1. Executive summary

The proposed discovery mechanism is:

1. Use the SMBus ARP **Device Default Address `0x61`** (7-bit).
2. Keep the normal SMBus ARP discovery model:
   - Prepare to ARP
   - Get UDID
   - simultaneous target response/arbitration
   - Assign Address
3. Replace the standard SMBus **structured 128-bit UDID** with a **128-bit UUIDv8** derived losslessly from the STM32 factory 96-bit UID.
4. Preserve the rest of the ARP framing and behavior, including PEC where applicable.
5. Treat this protocol as **PDCAN ARP**, not as generic standards-compliant SMBus ARP.

The key hardware conclusion is:

> The STM32G0 SMBus/I²C peripheral does not appear to interpret the semantic fields inside the 128-bit UDID. It provides SMBus default-address acknowledgment, target transmission, PEC support, and generic arbitration-loss detection. The response bytes themselves are software-provided data.

Therefore there is strong evidence that a UUIDv8 can be transmitted in the 16-byte identity position and participate in the same wired-AND arbitration as a standard SMBus UDID.

The one behavior that must be validated before freezing the protocol is:

> **Two STM32G031 targets simultaneously responding to the discovery Get-UDID transaction must cause the losing target to detect arbitration loss and immediately stop transmitting, while the winning target completes its UUID response.**

Treat this as a required hardware proof-of-concept.

---

## 2. Identity versus bus address

Each STM32G031 contains a factory-programmed **96-bit unique device identifier**.

Do not hash that UID into the normal 7-bit I²C address. Instead separate permanent identity from routing:

```text
immutable identity     = UUIDv8 derived from STM32 UID96
bus routing address    = dynamically assigned 7-bit I²C address
```

Example:

```text
UUID                                    I²C address
------------------------------------    -----------
7f4ab236-4198-8e32-a572-19a18f6c345a    0x20
17ac9314-9901-8b12-b28f-29180c3ba122    0x21
996a72d0-8714-8fb3-8041-92bcfae22139    0x22
```

The UUID is permanent. The I²C address is a controller-assigned lease and may change.

---

## 3. Why UUIDv8 fits

RFC 9562 defines UUIDv8 as an application-defined UUID format.

A UUID is 128 bits, with:

- 4 bits reserved for `version = 8`
- 2 bits reserved for the RFC UUID variant
- **122 application-defined bits**

Therefore:

```text
122 custom UUIDv8 bits
-96 STM32 UID bits
-------------------
 26 remaining custom bits
```

All 96 STM32 UID bits can be preserved without hashing or truncation.

The mapping should be injective with respect to the STM32 UID:

```text
UID_A != UID_B  =>  UUID_A != UUID_B
```

for a fixed project metadata value.

Reference:

- RFC 9562: https://www.rfc-editor.org/rfc/rfc9562.html

---

## 4. Recommended UUIDv8 construction

Define a 122-bit custom payload:

```text
CUSTOM122 = METADATA26 || STM32_UID96
```

where:

```text
METADATA26
    stable project identity/schema information

STM32_UID96
    complete factory-programmed STM32 UID
```

Do **not** include mutable state in `METADATA26`, including:

- current I²C address
- physical slot
- firmware version
- runtime state
- commissioned configuration

A reasonable format is:

```text
METADATA26
┌──────────────────────┬────────────────────────────┐
│ identity schema      │ project/product namespace │
│ 8 bits               │ 18 bits                    │
└──────────────────────┴────────────────────────────┘
```

The exact numeric namespace should be frozen in a protocol ADR.

RFC 9562 divides UUIDv8 custom data as:

```text
custom_a = 48 bits
version  = 4 bits = 0b1000
custom_b = 12 bits
variant  = 2 bits = 0b10
custom_c = 62 bits
```

Map the 122-bit payload as:

```text
CUSTOM122[121:74] -> custom_a
CUSTOM122[73:62]  -> custom_b
CUSTOM122[61:0]   -> custom_c
```

and then insert the UUID version and variant bits.

```text
0                   47 48  51 52       63 64 65 66                127
┌────────────────────┬──────┬────────────┬─────┬────────────────────┐
│      custom_a      │  8   │  custom_b  │ 10  │      custom_c      │
│      48 bits       │4 bits│   12 bits  │2 bit│      62 bits       │
└────────────────────┴──────┴────────────┴─────┴────────────────────┘
```

This preserves every STM32 UID bit; the mandatory UUID bits are inserted between custom fields rather than overwriting source identity bits.

---

## 5. Canonical STM32 UID representation

Firmware must define one canonical byte representation of the 96-bit STM32 UID.

Recommended rule:

> Read the 12 factory UID bytes in increasing memory-address order and treat those 12 bytes as the canonical `STM32_UID96` byte string.

Do not rely on native integer endianness.

Prefer:

```rust
type Stm32Uid = [u8; 12];
```

The same 12-byte representation must be used by:

- firmware
- manufacturing/provisioning tooling
- backplane controller
- CLI
- tests
- UUID reconstruction code

Add fixed test vectors from physical devices during bring-up.

---

## 6. SMBus ARP model

SMBus ARP solves discovery of multiple targets that do not yet have unique bus addresses.

The Device Default Address is:

```text
7-bit address = 0x61
```

A simplified sequence is:

```text
controller                         targets

Prepare to ARP @ 0x61  --------->  A B C

Get UDID @ 0x61  ----------------> A B C respond simultaneously
                    <------------- A wins arbitration

Assign Address(UUID_A, 0x20) ----> all receive
                                    only A matches identity
                                    A adopts 0x20

Get UDID @ 0x61  ----------------> B C respond
                    <------------- B wins

Assign Address(UUID_B, 0x21) ----> B adopts 0x21

Get UDID @ 0x61  ----------------> C responds

Assign Address(UUID_C, 0x22) ----> C adopts 0x22

Get UDID @ 0x61  ----------------> no unresolved target
```

SMBus ARP uses SDA's open-drain/wired-AND behavior for arbitration.

A target loses when:

```text
target attempts to transmit 1
but observes SDA = 0
```

The losing target stops participating in that response.

This is conceptually similar to CAN dominant/recessive arbitration.

Reference:

- SMBus Specification 3.3.1:
  https://smbus.org/specs/SMBus_3_3_1_20241020.pdf

---

## 7. Standard SMBus UDID versus UUIDv8

The SMBus standard defines the 128-bit UDID as a structured value with semantic fields such as:

- device capabilities
- revision/version information
- vendor ID
- device ID
- interface identifier
- subsystem identifiers
- vendor-specific identifier

A UUIDv8 does not use that layout.

Therefore a UUIDv8 placed in the 16-byte ARP identity position is **not a standards-compliant SMBus UDID**.

A generic third-party SMBus ARP host could interpret UUID bits as SMBus capability/vendor/device fields and draw incorrect conclusions.

For that reason, document this protocol as:

```text
PDCAN ARP
```

Do not claim arbitrary third-party SMBus ARP interoperability.

---

## 8. Evidence that STM32 hardware treats the identity as opaque bytes

### 8.1 User-referenced `stm32-smbus`

Examined source:

https://github.com/selat/stm32-smbus/blob/master/smbus.c

This implementation is:

- old
- STM32F4-based
- host/controller-oriented
- not an ARP target implementation

It does not prove ARP target arbitration.

It does show the general design pattern: the peripheral supplies SMBus/I²C transport functions while protocol payload bytes are explicitly handled by software.

### 8.2 STM32G0 HAL exposes an ARP target mode

ST's STM32G0 HAL defines:

```c
#define SMBUS_PERIPHERAL_MODE_SMBUS_SLAVE_ARP I2C_CR1_SMBDEN
```

The associated low-level documentation describes this as SMBus **Slave Default address acknowledge**.

The hardware configuration contains no registers for:

- vendor ID
- device ID
- subsystem fields
- a 128-bit UDID
- UUID fields

References:

- https://github.com/STMicroelectronics/stm32g0xx-hal-driver/blob/master/Inc/stm32g0xx_hal_smbus.h
- https://dev.st.com/stm32cube-docs/

### 8.3 HAL initialization configures transport features, not UDID semantics

The STM32G0 HAL SMBus initialization configures properties including:

- timing
- own address
- dual address
- general call
- clock stretching
- PEC
- SMBus peripheral mode
- SMBus timeout

ARP target mode is enabled through the peripheral-mode bit.

There is no initialization field containing semantic UDID data.

Reference:

https://github.com/STMicroelectronics/stm32g0xx-hal-driver/blob/master/Src/stm32g0xx_hal_smbus.c

### 8.4 Target transmit data is software supplied

The STM32G0 HAL target-transmit path ultimately loads bytes from a software buffer into the transmit register.

Conceptually:

```c
TXDR = *buffer++;
```

There is no indication that hardware parses those bytes as SMBus UDID subfields.

From the transmission engine's perspective these should therefore be equivalent:

```text
standard 16-byte SMBus UDID
```

and:

```text
16-byte UUIDv8
```

### 8.5 Arbitration loss is generic

The STM32G0 HAL exposes hardware arbitration loss as:

```text
ARLO
```

and maps it to:

```text
HAL_SMBUS_ERROR_ARLO
```

The generic SMBus error handler detects and clears this condition.

There is no UDID-specific semantic logic associated with the arbitration error path.

Reference:

https://github.com/STMicroelectronics/stm32g0xx-hal-driver/blob/master/Src/stm32g0xx_hal_smbus.c

---

## 9. Engineering conclusion

The current conclusion is:

> **The STM32G031 peripheral is expected to allow arbitrary 16-byte identity data to be transmitted during an ARP-style Get-UDID response. Arbitration should operate on those bits without caring about the standard SMBus UDID field meanings.**

Conceptually:

```text
UUIDv8
    │
    ▼
16 canonical UUID bytes
    │
    ▼
SMBus target transmit path
    │
    ▼
open-drain SDA arbitration
```

This conclusion is supported by:

- ST register definitions
- ST HAL implementation behavior
- SMBus architecture
- absence of semantic UDID parsing in the peripheral API

It must still be validated on physical hardware.

---

## 10. Proposed PDCAN ARP wire behavior

Keep SMBus ARP mechanics as close to standard as practical.

### Preserve

```text
Device Default Address       0x61

Prepare-to-ARP semantics
Get-identity semantics
Assign-address semantics

simultaneous identity response
wired-AND arbitration
assigned-address transition
PEC behavior
```

### Change

Instead of:

```text
16-byte standards-compliant SMBus UDID
```

use:

```text
16-byte canonical UUIDv8
```

The discovery identity is therefore the same UUID used everywhere else in the system.

---

## 11. Canonical UUID wire format

Transmit the UUID in canonical RFC/network byte order:

```text
byte 0
byte 1
...
byte 15
```

Do not transmit the native in-memory representation of a Rust/C UUID structure without explicit serialization.

Example UUID:

```text
7f4ab236-4198-8e32-a572-19a18f6c345a
```

Wire bytes:

```text
7f 4a b2 36 41 98 8e 32 a5 72 19 a1 8f 6c 34 5a
```

Use this exact byte sequence for:

- ARP arbitration
- address-assignment identity matching
- logs
- CLI
- persistence
- protocol responses

---

## 12. Arbitration implications

All carrier modules should share the same `METADATA26`.

During discovery:

```text
common metadata bits
        │
        ▼
identical arbitration prefix
        │
        ▼
arbitration reaches STM32 UID bits
        │
        ▼
one unique UID wins
```

All UUID version/variant bits are also common across devices.

No hash collision is introduced because all 96 STM32 UID bits remain present.

The numerically lowest unresolved UUID bitstream wins a given arbitration round because `0` is dominant.

No semantic meaning should be assigned to enumeration order.

---

## 13. Dynamic I²C address allocation

Do not derive the normal I²C address from the UUID.

The backplane owns an address pool.

Example:

```text
carrier pool:
0x20 .. 0x6F

exclude:
0x61 SMBus Device Default Address
reserved I²C addresses
fixed backplane peripherals
system-reserved addresses
```

After a UUID wins discovery:

```text
UUID_A
   │
   ▼
controller selects free address
   │
   ▼
Assign Address(UUID_A, address)
   │
   ▼
target switches normal hardware target address
```

A lowest-free-address allocator is sufficient.

The dynamic address is not part of the UUID.

---

## 14. Address persistence

For Rev A, use **volatile dynamic addressing**.

```text
power-on
   │
   ▼
unresolved
   │
   ▼
controller discovers UUID
   │
   ▼
controller assigns address
```

Advantages:

- simple target state machine
- no flash wear
- no stale-address conflicts
- controller remains source of truth

If a target resets, it should return to the unresolved state and be rediscovered.

Keep the UUID permanent; keep the bus address ephemeral.

---

## 15. Hot-plug discovery

A target cannot initiate an ordinary I²C transaction upstream, so discovery remains controller-driven.

The backplane should periodically probe for unresolved devices.

```text
periodic Get Identity @ 0x61
       │
       ├── no response -> nothing new
       │
       └── response    -> enumerate device
```

Resolved devices must not participate in unresolved discovery.

A polling interval of roughly **1–10 seconds** is reasonable; exact timing is a product/UX decision.

---

## 16. Physical slot identity

This discovery mechanism determines:

```text
which UUIDs exist
which temporary I²C address belongs to each UUID
```

It does not determine physical slot position.

Current hardware has no spare dedicated slot-ID signal, so physical slot association remains a user-level/manual responsibility.

Do not encode a presumed slot into the UUID or dynamic address.

---

## 17. Embassy integration

Current `embassy-stm32` provides target/slave-side I²C capabilities including:

- listen for address matches
- respond to reads
- respond to writes
- reconfigure target addresses at runtime

Reference:

https://docs.embassy.dev/embassy-stm32/

The high-level API does not currently appear to expose every STM32 SMBus-specific control bit needed for ARP, especially the equivalent of:

```text
I2C_CR1.SMBDEN
```

Likely implementation options:

1. small extension to `embassy-stm32`
2. controlled low-level configuration through `stm32-metapac`
3. an internal wrapper around Embassy's target-mode driver

Avoid implementing a completely separate I²C driver unless necessary.

---

## 18. Required two-target proof-of-concept

Before treating the design as frozen, test with:

```text
1 controller
2 STM32G031 targets
shared SDA/SCL
normal pull-ups
```

Give each target a deterministic UUIDv8 with an early differing bit.

### Test A — default address

Verify both unresolved targets acknowledge/respond to the Device Default Address as intended.

### Test B — simultaneous identity transmission

Trigger discovery so both targets begin transmitting.

Expected:

```text
first differing bit
        │
        ├── target sending 0 continues
        │
        └── target sending 1 observes 0 and loses
```

The controller must receive one complete UUID.

### Test C — loser behavior

Verify the losing target:

- stops transmitting promptly
- releases SDA
- does not wedge SDA/SCL
- returns to unresolved/listening state
- reports or exposes the expected arbitration-loss condition

Inspect `ISR.ARLO` or equivalent low-level status.

### Test D — assignment

Assign a normal address to the winning UUID.

Verify:

- only the exact matching target accepts it
- its target address changes
- it stops participating as unresolved
- the loser remains discoverable

### Test E — second round

Run discovery again.

Only the remaining unresolved device should answer.

### Test F — hot addition

After resolving both targets, add a third unresolved target and verify periodic discovery finds only that target.

---

## 19. Failure cases to test

### Identical UUID

Should be impossible if factory UID uniqueness and mapping are correct, but unit-test it.

Two identical arbitration identities would never diverge and therefore neither could lose.

Never support arbitrary non-unique UUID overrides in production firmware.

### PEC failure

If PEC is used, verify corrupted discovery frames:

- are rejected
- do not cause address assignment
- leave the target unresolved

### Controller reset during enumeration

Test reset:

- before response
- during response
- after UUID receipt
- during assignment

Desired behavior is always recoverable unresolved state.

### Repeated arbitration loss

Repeatedly force the same target to lose and verify the peripheral state machine remains healthy.

### Target reset after assignment

For Rev A:

```text
target reset
    │
    ▼
dynamic address forgotten
    │
    ▼
target returns to unresolved discovery
```

---

## 20. Suggested software structure

```text
identity/
    stm32_uid.rs
    uuid_v8.rs

arp/
    protocol.rs
    target.rs
    controller.rs
    pec.rs

i2c/
    upstream_target.rs
```

Suggested core types:

```rust
pub struct DeviceUuid([u8; 16]);

pub enum ArpState {
    Unresolved,
    Assigned { address: u8 },
}
```

Identity API:

```rust
fn read_stm32_uid() -> [u8; 12];

fn uuid_v8_from_stm32_uid(
    uid: [u8; 12],
    metadata: Metadata26,
) -> DeviceUuid;
```

ARP code should operate only on `DeviceUuid`. It should not know how the UUID was derived.

---

## 21. Required invariants

Encode these as automated tests.

### Identity

```text
same STM32 UID + metadata => same UUID
different STM32 UID      => different UUID
UUID version             == 8
UUID variant             == RFC variant 0b10
all 96 UID bits          are recoverable
```

### Addressing

```text
0x61 is never assigned as a normal carrier address
no two resolved targets get the same address
target reset clears volatile address
UUID never depends on dynamic address
```

### ARP

```text
resolved targets do not join unresolved discovery
arbitration loss does not alter UUID
only exact UUID match accepts address assignment
failed PEC never results in assignment
```

---

## 22. Compliance statement

Document the implementation as:

> **PDCAN ARP uses the SMBus ARP transport/arbitration model and Device Default Address, but substitutes a UUIDv8 for the standard SMBus structured UDID. It is therefore not intended to be a generic standards-compliant SMBus ARP device.**

This wording should appear in:

- protocol documentation
- firmware ADR
- relevant source comments
- test plans

The custom identity format is intentional.

---

## 23. Why this tradeoff is useful

Using UUIDv8 allows the system to:

- preserve all 96 factory UID bits
- avoid identity hash collisions
- use normal 128-bit UUID types in Rust and databases
- use familiar UUID strings in CLI/logging
- use the same identity during discovery and normal operation
- retain SMBus wired-AND arbitration
- keep dynamic I²C addresses completely separate from identity

The lost capability is generic third-party SMBus ARP interoperability, which is not currently a project requirement.

---

## 24. Final recommendation

Proceed with this design unless the two-target proof-of-concept disproves the target arbitration assumption:

```text
STM32 UID96
      │
      ▼
lossless UUIDv8 mapping
      │
      ▼
128-bit permanent PDCAN device identity
      │
      ├── logs / CLI / APIs
      │
      └── PDCAN ARP identity
              │
              ▼
        discovery at 0x61
              │
              ▼
       wired-AND arbitration
              │
              ▼
      controller assigns free
         7-bit I²C address
```

For Rev A:

- use **volatile** dynamically assigned I²C addresses
- preserve the full STM32 UID96 in UUIDv8
- do not hash or truncate UID96
- do not derive the I²C address from the UUID
- do not claim generic SMBus ARP compliance
- validate two-device target arbitration on real STM32G031 hardware before freezing the wire protocol

---

## 25. Primary references

### SMBus

System Management Bus (SMBus) Specification, Version 3.3.1:

https://smbus.org/specs/SMBus_3_3_1_20241020.pdf

### UUID

RFC 9562 — Universally Unique IDentifiers (UUIDs):

https://www.rfc-editor.org/rfc/rfc9562.html

### STM32G0 HAL SMBus definitions

https://github.com/STMicroelectronics/stm32g0xx-hal-driver/blob/master/Inc/stm32g0xx_hal_smbus.h

### STM32G0 HAL SMBus implementation

https://github.com/STMicroelectronics/stm32g0xx-hal-driver/blob/master/Src/stm32g0xx_hal_smbus.c

### Referenced STM32 SMBus implementation

https://github.com/selat/stm32-smbus/blob/master/smbus.c

### Embassy STM32 I²C support

https://docs.embassy.dev/embassy-stm32/
