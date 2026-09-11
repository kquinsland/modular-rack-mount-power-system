# `pdcan` Host Utility

`pdcan` is a Clap-derived CLI. Linux builds use CAN-FD with bit-rate switching
through SocketCAN. Artifact inspection, ID decoding, help, and schema discovery
are portable.

Run `pdcan --help` or a subcommand's `--help` for the human interface. Automated
clients should use the `clap_schema` contract generated from the same command
tree:

```sh
pdcan schema --full
pdcan schema firmware activate
```

The schema includes canonical command paths, positional order, inherited global
options, defaults, possible values, conflicts, required argument groups, and
successful-output contracts where declared. Clap remains authoritative for
parsing.

## Common operations

```sh
pdcan scan
pdcan status 17
pdcan set-policy 17 --enabled \
  --max-voltage-mv 20000 --max-current-ma 5000 --max-power-mw 100000
pdcan set-fan 3 0 --duty 70
pdcan bind 17 aabbccddeeff001122334455 0
pdcan emergency-disable all
pdcan acknowledge-resolved 17
```

Carriers are addressed as nodes, never `NODE.PORT`. Fan IDs are zero-based 0
and 1 on a backplane. Capability discovery determines whether a command applies.

## Firmware updates

```sh
pdcan firmware inspect carrier-1.0.0.pdcan
pdcan firmware status 17
pdcan firmware stage 17 carrier-1.0.0.pdcan
pdcan firmware activate 17 --allow-interruption
pdcan firmware update 17 carrier-1.0.0.pdcan --allow-interruption
pdcan firmware abort 17
```

`stage` transfers one bundle directly from the host to one node using 48-byte
chunks, an eight-frame window, and cumulative acknowledgements. It verifies and
persists staged metadata without resetting. `activate` first reads node status.
If update impact is `interrupt`, the CLI refuses activation unless the operator
passes `--allow-interruption`. `update` is only stage followed by activate; it
does not give the backplane control over a carrier.

## Errors

Clap owns syntax and argument-validation failures and exits with status 2. The
host tool uses a `thiserror` enum for operational failures. Stable categories
are `invalid_input`, `artifact`, `transport`, `timeout`, `codec`,
`node_rejected`, `interruption_required`, and `internal`. With `--json`, errors
are emitted to stderr as a stable `{ok:false,error:{code,message}}` envelope;
callers should branch on `code`, not prose. Exit statuses are respectively 2,
3, 4, 5, 6, 7, 8, and 1.

The checked-in `json-schema.json` describes the currently supported JSON
records. JSON success records are presently available for discovery; commands
without a declared JSON success record retain their human-readable output.
