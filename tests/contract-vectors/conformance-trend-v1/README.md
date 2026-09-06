# `borrowser-conformance-trend-v1` vectors

These files were fixed with a standalone encoder of the frozen Trend V1
grammar. Fingerprints in `unchanged.bin` were independently recomputed from
the aggregate-detail golden, including `B(VariantKey || H(variant))` logical
list items.

| Vector | Meaning | Section offsets | SHA-256 |
| --- | --- | --- | --- |
| `unchanged.bin` | The empty baseline compared with itself: 25 unchanged logical cases and 25 unchanged variants | `1:33/43/720`, `2:763/773/259`, `3:1032/1042/130`, `4:1172/1182/3948`, `5:5130/5140/5344`, `6:10484/10494/52`, `7:10546/10556/52` | `dfb48cc741f0a00d643df563e3e034efe2546cb2c4235f4f3746794ee8396702` |
| `four-population.bin` | A removed logical case, added variant, changed evaluated advisory point, and unchanged note | `1:33/43/720`, `2:763/773/259`, `3:1032/1042/138`, `4:1180/1190/138`, `5:1328/1338/169`, `6:1507/1517/297`, `7:1814/1824/179` | `6ffdaec2af0ad71190021279d81a92808595cb3aecef4d52f2aae6a2d0c096af` |
| `evaluation-transition.bin` | One surviving advisory point changes from unevaluated to evaluated; only mask bit 1 (`0x02`) is set | `1:33/43/720`, `2:763/773/259`, `3:1032/1042/134`, `4:1176/1186/52`, `5:1238/1248/52`, `6:1300/1310/297`, `7:1607/1617/52` | `7603a20c62d2bcf29f8be9731569623d215fd0773cafa4798b27bb264b5e0f69` |

Section 1 contains the baseline format plus all thirteen baseline component
versions. Section 2 orders the compatibility tuple before the four digests.
Section 3 contains both framed operation descriptors and six U64 evaluation
counts. Population bodies contain only counts and framed records; the section
tag supplies the population identity.

The two advisory vectors were independently re-reviewed after removing the
non-contractual inner variant-key frame. Their advisory population key has one
outer record-level `B(population-key)` whose payload begins directly with
`S(TestId) || S(observation) || S(variant-kind)`, followed by `S(comparable)`
and `S(track-id)`.

The logical fingerprints in `unchanged.bin` were independently recomputed from
the embedded aggregate-detail bytes. Twenty-four cases contain variants and
changed identity when the non-contractual inner `B(VariantKey)` was removed;
the one zero-variant case retained its identity. Framing and section offsets did
not change.

`four-population.bin` has one fixed opaque logical fingerprint used to exercise
the removed-record grammar; it is not presented as an aggregate-derived logical
identity. `evaluation-transition.bin` has no logical records. Inspection of
their actual population records therefore found no logical fingerprint bytes to
recompute under the corrected preimage contract.

Maximum-sized output is not committed. Checked derivation assertions prove the
corrected conservative syntactic maximum of 74,232,509 bytes and its 48,640
bytes of headroom within the frozen 74,281,149-byte accepted V1 ceiling.
Bounded-writer tests cover that frozen transport limit.
