# `borrowser-external-capture-id-v1` vectors

`representative.bin` is an independently authored 26-field canonical capture-ID
preimage. It records engine `engine` version `1`, OS `os`, architecture `arch`,
no viewport/device-scale/font applicability, an empty resource and invocation
set, fixture revision `revision`, DOM artifact length 17, and the frozen static
HTML parser context. It does not contain or verify an artifact body.

The 33-byte domain occupies offsets 0..33. Fields 1 through 26 begin at offsets
33, 83, 99, 110, 121, 133, 144, 158, 185, 212, 247, 264, 278, 295, 313, 355,
369, 380, 416, 458, 500, 514, 550, 568, 610, and 663. Each field has a 2-byte
tag and 8-byte payload length. The file ends at offset 696.

The bytes were fixed from the published TLV grammar in a standalone Python
encoder, independently of the Rust producer. Reviewed SHA-256:

`426af2e1da5e99c718e4792661dbc253a09fc69656bffb039b0f0a67b7ac0bb5`.
