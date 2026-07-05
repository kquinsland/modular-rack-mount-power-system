# Power Budget

Track the system and per-slot power assumptions here.

| Item | Value | Notes |
| --- | --- | --- |
| Backplane input voltage | TBD | Define nominal and max. |
| Backplane input current | TBD | Include connector, copper, and fuse limits. |
| Carrier slot voltage | TBD | Usually same as input unless converted on the backplane. |
| Carrier slot current | TBD | Must match connector, copper, and protection strategy. |
| Number of carrier slots | TBD | Determines total backplane current. |
| USB-C PD module max output | TBD | Per carrier. |

## Protection Checklist

- Input fuse or breaker.
- Per-slot fuse, eFuse, current limit, or zero-ohm bring-up option.
- Reverse polarity protection, if input connector allows it.
- TVS or surge strategy for the DC input.
- I2C pull-up placement and voltage domain.
