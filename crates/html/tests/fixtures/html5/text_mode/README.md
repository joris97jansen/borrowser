# HTML5 Text-Mode Golden Corpus

This folder catalogs the dedicated Core-v0 RAWTEXT / RCDATA / script golden
fixtures that live in the existing tokenizer, DOM, and patch harness roots.

Purpose:

- keep tricky text-mode regression coverage easy to find,
- ensure the same scenario family is exercised at token, DOM, and patch levels,
- document the intended corpus without changing the flat fixture-loader
  contract used by the current harnesses.

Layout note:

- The active harness loaders remain flat under `tokenizer/`, `tree_builder/`,
  and `tree_builder_patches/`.
- This folder is therefore a corpus catalog rather than a second fixture root.
- The catalog exists so text-mode scenarios are discoverable as one logical
  suite until the harness layout itself is widened.
- Tokenizer fixtures keep the repo-wide `tok-*` naming convention, while DOM
  and patch fixtures use `tm-*` as the shared text-mode scenario prefix inside
  their existing harness namespaces.

Chunking note:

- These fixtures are intentionally written with a real text-mode closing tag as
  the actual terminator, surrounding nested tags after that terminator, and
  near-miss `</...` lookalikes that MUST remain literal text before it.
- CI coverage for split-close handling comes from the existing deterministic
  semantic chunk plans and seeded fuzz chunking in the tokenizer, DOM, and
  patch golden harnesses.

Escaping note:

- Golden files reuse the escape rules of their owning harness formats.
- In quoted token/DOM/patch text, one literal backslash is rendered as `\\`.
- That means an input fragment like `<\\/script>` is expected to appear in
  golden text as `<\\\\/script>`.

Trailing-newline note:

- Tokenizer `input.html` fixtures continue to use the existing tokenizer-suite
  convention where the formatting newline at end-of-file is semantically
  preserved and therefore appears as a final `CHAR text="\\n"` in goldens.
- DOM and patch harness loaders strip exactly one terminal line ending from
  `input.html`, matching the contracts documented in their own fixture READMEs.

Tokenizer fixtures:

- `tok-rawtext-style`
- `tok-rawtext-style-end-tag-literal-attrs`
- `tok-textmode-style-lookalike-close-nested`
- `tok-rcdata-title`
- `tok-rcdata-title-close-tag-whitespace`
- `tok-rcdata-title-end-tag-literal-attrs`
- `tok-rcdata-textarea`
- `tok-rcdata-textarea-end-tag-literal-slash`
- `tok-textmode-rcdata-entities-nested`
- `tok-script-data-basic`
- `tok-script-data-close-tag-whitespace`
- `tok-script-data-end-tag-literal-attrs`
- `tok-script-data-string-close`
- `tok-textmode-script-lookalike-close-nested`

DOM fixtures:

- `tm-style-rawtext-lookalike-close-nested`
- `tm-rcdata-title-textarea-entities-nested`
- `tm-script-lookalike-close-nested`

Patch fixtures:

- `tm-style-rawtext-lookalike-close-nested`
- `tm-rcdata-title-textarea-entities-nested`
- `tm-script-lookalike-close-nested`
