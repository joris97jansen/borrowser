# `borrowser-conformance-baseline-v1` vectors

These binary vectors were authored with a standalone implementation of the
frozen framing grammar. All embed the checked-in aggregate-detail V1 golden
verbatim. Their section-tag offsets are shown as `tag/body/length`:

| Vector | Meaning | Section offsets | SHA-256 |
| --- | --- | --- | --- |
| `empty.bin` | Complete known-empty advisory membership, no evaluation, no notes | `1:36/46/679`, `2:725/735/41360`, `3:42095/42105/4`, `4:42109/42119/4`, `5:42123/42133/4`, `6:42137/42147/37`, `7:42184/42194/4` | `7f0f654304d92d444ba85520e4db49199266d015a3c829bd57004447d4c0926a` |
| `selected-zero.bin` | Completed selected scope with zero matching declarations | section 6 is `42137/42147/148`; other membership sections match `empty.bin` | `f02d459b425e92e91a14e6578f595887ddfbeaa795f6d0f61ae2a33d7c856411` |
| `selected-evidence.bin` | One capture, track, point, independently framed equivalent result slot, and capture-linked note | `1:36/46/679`, `2:725/735/41360`, `3:42095/42105/748`, `4:42853/42863/195`, `5:43058/43068/155`, `6:43223/43233/183`, `7:43416/43426/184` | `8a286050bd01ba41da927219e0d639c654fb12073b5103ecb40d5ad242c3d1a9` |
| `all-declared.bin` | Historical all-declared completed evaluation of one point | section 6 is `43223/43233/76`; notes are empty | `a5d10338383c1e56346e9a1bada9698bbf382f58e44c15a882ed9a5ac8ec1783` |

The envelope prefix ends at offset 36. Each body offset follows its 2-byte tag
and 8-byte length. Every file ends exactly after section 7. The selected
evidence slot bytes were independently fixed as `B(U8(1) || B(S("equivalent")))`;
the outer `B` is the list-item frame required by `L(O(result))`.

Maximum-sized files are not committed. The exact 47,219,283-byte boundary and
one-byte-over behavior are tested with the independently asserted per-section
derivation, bounded writer, and transport-limit tests.
