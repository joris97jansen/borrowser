# Independent V5 canonical vector

`reservation.json` is a manually specified minimal V5 reservation observation with
an explicitly empty instance collection and source page 1, reservation position 0.
It was encoded independently with sorted JSON keys, compact separators and one LF;
`reservation.sha256` is the SHA-256 of those exact bytes. It was not captured from the
Rust serializer under test.

Reservation requester metadata is explicitly outside this V5 observation contract;
the vector has no requester member and the parser rejects an added requester field.

The V5 contract test checks exact bytes, identity, parsing, bounds and historical V4
rejection. The mixed-carrier regressions additionally reuse unchanged V3/V4 vectors
and a V2 caller record to verify historical occurrence-credit isolation. There is no
V5 inventory/context/publication fixture because those formats were not introduced.
