# O8: Document The CSSOM/Rule Model Contract And Retire Obsolete Internal Representations

Last updated: 2026-04-08  
Status: implemented

This document closes Milestone O by making the engine-facing CSS
stylesheet/rule/declaration/value model the explicit repository contract and by
isolating the remaining legacy compatibility surfaces.

Related code:
- `crates/css/src/lib.rs`
- `crates/css/src/model/mod.rs`
- `crates/css/src/model/entry.rs`
- `crates/css/src/model/serialize.rs`
- `crates/css/src/cascade.rs`
- `crates/css/src/syntax/mod.rs`
- `crates/css/src/syntax/results.rs`
- `crates/browser/src/page.rs`

Related documents:
- `docs/css/o1-rule-value-model-architecture.md`
- `docs/css/o6-canonical-model-serialization.md`
- `docs/css/o7-parser-model-cutover.md`
- `docs/css/syntax-parser-contract.md`

## Implemented Result

Milestone O is now complete as a repository contract and implementation state:

- the crate-root stylesheet parse API is model-first
- the browser stores model parse artifacts as stylesheet state
- the engine-facing stylesheet/rule/declaration/value model is documented as
  the default handoff into later selector and cascade work
- ownership, span/debug-span, and snapshot expectations are documented across
  the model docs and Milestone O contract documents
- compatibility-scoped stylesheet and raw-string declaration surfaces are
  isolated as explicit migration APIs rather than looking like the preferred
  contract

## Default Contract After Milestone O

Future engine-facing CSS work should treat these as the normative contract:

- `css::parse_stylesheet(...) -> css::Stylesheet`
- `css::parse_stylesheet_with_options(...) -> css::StylesheetParse`
- `css::Stylesheet`
- `css::Rule`
- `css::Declaration`
- `css::DeclarationValue`
- `css::serialize_stylesheet_for_snapshot(...)`
- `css::serialize_stylesheet_parse_for_snapshot(...)`

This is the stable stylesheet handoff layer for:

- selector AST refinement
- selector matching
- cascade precedence and winner resolution
- specified/computed value work
- diagnostics and future CSS tooling

## Ownership, Span, And Serialization Expectations

Milestone O established and implemented these durable expectations:

- the model owns long-lived stylesheet/rule/declaration/value data
- parser-local scratch data is not borrowed by the model contract
- structural nodes carry direct source spans
- text/helper payloads carry optional debug spans where appropriate
- canonicalization is limited to model-owned identity rules such as
  case-insensitive property and at-rule names
- snapshot/debug output is explicit, versioned, deterministic, and aligned with
  the model rather than authored CSS pretty-printing

The authoritative details live in:

- `docs/css/o1-rule-value-model-architecture.md`
- `docs/css/o6-canonical-model-serialization.md`

## What Remains Intentionally Deferred

Milestone O does not complete later CSS semantics. The following remain
intentional follow-up work:

- selector AST expansion beyond preserved selector/prelude payloads
- selector matching beyond the current compatibility-scoped subset
- at-rule semantic interpretation
- cascade winner resolution that natively consumes model selectors/values
- property-specific specified-value parsing
- computed-style generation on top of the model without the current string-based
  bridge
- full inline `style=""` migration away from compatibility-scoped raw-string
  declarations

Those are not gaps in the model contract. They are later-layer semantics that
must build on the model instead of bypassing it.

## Obsolete And Transitional Representations

The following surfaces remain in-repo only as explicit migration boundaries:

- `css::syntax::CompatSelector`
- `css::syntax::CompatRule`
- `css::syntax::CompatStylesheet`
- `css::syntax::Declaration { name: String, value: String }`
- `css::syntax::parse_stylesheet(...)`
- `css::syntax::parse_declarations(...)`
- `StylesheetParse::to_compat_stylesheet()`

Retirement/isolation policy:

- these compatibility surfaces are not the engine-facing contract
- new engine-facing CSS work must not store them as primary stylesheet state
- new parser integration must not route through them to reach the model
- crate-root compatibility re-exports are now explicitly marked as
  migration-only
- compatibility projection belongs only at the consumer boundary that still
  needs it

Current remaining compatibility boundary:

- `css::cascade` derives its temporary compatibility view from model parse
  artifacts at attachment time

## Contributor Guidance

When working on later CSS milestones:

- start from `css::StylesheetParse` / `css::Stylesheet`
- use `css::syntax::...` only when you are intentionally doing syntax-layer
  work
- treat compat/raw-string surfaces as transitional adapters to be reduced, not
  extended
- update model snapshots and fixtures when model-grammar changes are intended
- do not introduce new raw-string stylesheet/value storage as a normative path

## Exit Criteria

- the CSSOM/rule/value model contract is documented in-repository
- ownership/span/serialization expectations are documented and linked
- obsolete compatibility/raw-string assumptions are isolated from the preferred
  crate-root contract
- future contributors can continue into selector and cascade milestones without
  ambiguity about the model handoff
