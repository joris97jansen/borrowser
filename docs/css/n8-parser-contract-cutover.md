# N8: Document The CSS Parser Contract And Retire Prototype Parsing Path

Last updated: 2026-04-08  
Status: implemented

## Implemented Result

Milestone N is now closed out as a completed syntax-layer foundation.

The CSS syntax contract is fully documented in-repo, the structured tokenizer +
parser stack is the defined basis for future CSS work, and the product browser
path no longer treats the compatibility wrappers as the primary stylesheet
parser interface.

The shipped syntax-layer cutover state is:

- `css::syntax` owns the tokenizer, structured parser, recovery behavior,
  invariants, limits, and stable snapshot surface
- `css::syntax::parse_stylesheet_with_options(...)` and
  `css::syntax::parse_declarations_with_options(...)` are the stable
  structured syntax entrypoints
- crate-root whole-stylesheet parsing is now model-first after Milestone O;
  explicit syntax access remains available through `css::syntax::...` and root
  aliases such as `css::parse_syntax_stylesheet_with_options(...)`
- browser/runtime integration uses the structured parser path and only projects
  into compatibility forms at the cascade boundary
- compatibility wrappers remain available only as migration bridges for code
  that still needs `CompatStylesheet` or `Vec<Declaration>`
- there is no prototype split-based stylesheet parser path left in the runtime
  engine path

## Why This Exists

Milestone N started by defining the syntax contract and then incrementally
implemented:

- input/span primitives
- explicit token definitions
- a deterministic tokenizer
- a structured stylesheet parser
- deterministic recovery
- stable snapshots
- hardening limits and invariants

This final issue exists to make the new architecture explicit and durable:

- future CSS milestones must build on the syntax layer rather than reviving
  ad hoc parsing
- contributors need a clear distinction between stable parser APIs and
  temporary compatibility bridges
- the repository should state plainly that the prototype stylesheet parser path
  is retired

## Stable API After Cutover

Preferred structured syntax entrypoints:

- `css::syntax::parse_stylesheet_with_options(input, options) -> StylesheetParse`
- `css::syntax::parse_declarations_with_options(input, options) -> DeclarationListParse`
- `tokenize_str_with_options(input, options) -> CssTokenization`

Structured stylesheet contract:

- `StylesheetParse` owns `CssInput`, `CssStylesheet`, diagnostics, and stats
- stylesheet parsing is token-driven and deterministic
- compatibility projection is explicit through
  `StylesheetParse::to_compat_stylesheet()`

Compatibility bridges that remain intentionally secondary:

- `css::syntax::parse_stylesheet(input) -> CompatStylesheet`
- `css::syntax::parse_declarations(input) -> Vec<Declaration>`
- `CompatSelector`, `CompatRule`, `CompatStylesheet`

These compatibility shapes are not the syntax-layer contract and must not be
treated as the foundation for new parser work.

## What Was Retired

Retired as a parser foundation:

- string-splitting stylesheet parsing (`split('{')`, `split('}')`, `split(';')`)
- ad hoc lexical boundary detection from raw string fragments
- silent malformed-input skipping without typed diagnostics and fixed recovery
  points
- unstable `Debug` output as the test contract

Retired from the runtime browser path:

- direct stylesheet parsing through the compatibility convenience wrapper as the
  default basis for page stylesheet handling

The browser now parses stylesheet text through the structured syntax entrypoint,
converts it into the engine-facing model, and projects to compatibility forms
only where the current cascade layer still requires them.

## Known Deferred Limitations

Milestone N intentionally leaves these for later work:

- selector syntax remains only partially structured; richer selector AST work is
  queued separately
- compatibility selector projection still exists because the cascade layer does
  not yet consume selector AST nodes
- declaration values are preserved as syntax-layer component values, not
  property-specific semantic value trees
- at-rule semantics are still limited to structural parsing rather than full
  rule-specific interpretation
- tokenizer input is still one-shot per stylesheet/declaration-list parse, not
  streaming across chunk boundaries

These are known, intentional deferrals rather than gaps in the Milestone N
parser contract.

## Verification And Contributor Guidance

Useful verification commands:

```sh
cargo test -p css
cargo check -p browser
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Contributor rules after cutover:

- new CSS parser/tokenizer work must build on `css::syntax`
- do not introduce new string-splitting parser paths
- do not treat compatibility wrappers as the syntax-layer contract
- extend stable snapshot coverage when parser-observable behavior changes
- keep tokenizer-to-parser invariant validation centralized

## Exit Criteria

- CSS syntax-layer contract is documented in the repository
- prototype parsing path is retired or clearly isolated from the main path
- future contributors can understand how to build on the new syntax layer
- Milestone N can be considered complete without ambiguity
