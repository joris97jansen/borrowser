# AE9a: Parser-Owned Form Tree Construction Contract

Last updated: 2026-07-13
Scope: `crates/html/src/html5/tree_builder` and browser form-control seeding

## Normative acceptance source

AE9a is accepted against the immutable WHATWG HTML repository revision
`88ae68cb961651f0f92c5d2046049f53ecdfc6cf`, snapshot date **2026-07-11**.
The normative source is that revision's [`source`](https://github.com/whatwg/html/blob/88ae68cb961651f0f92c5d2046049f53ecdfc6cf/source), specifically “The stack of open elements”, “The form element pointer”, “Creating and inserting nodes”, and “The rules for parsing tokens in HTML content”, including the `in body`, `text`, and `in table` insertion modes and self-closing-flag acknowledgment steps.

The live [HTML parsing page](https://html.spec.whatwg.org/multipage/parsing.html)
is convenience-only. Later Living Standard changes do not alter AE9a acceptance.
The pinned revision is an acceptance snapshot; this contract does not claim it
introduced these algorithms.

## Ownership

HTML tree construction owns form-token recovery, `FormElementPointer(PatchKey)`,
the exact stack operations it requires, textarea initial-LF suppression, parser
errors, and parser-created DOM/patch ordering. The pointer is set only after a
parser-created form succeeds and never uses borrowed DOM references, raw
pointers, runtime IDs, name lookup, or DOM ancestry.

Browser/runtime consumes parser-created nodes to seed interactive state. It does
not decide form acceptance or recovery and does not strip textarea source LFs.
Its existing radio ancestor grouping remains runtime behavior, independent of
the parser form pointer.

## Insertion and stack semantics

Dispatch uses only normal and void semantic insertion. Normal `form`,
`textarea`, `button`, and `fieldset` insertions retain their stack entry even
with trailing-solidus syntax; that syntax records a deterministic parser error.
Void `input` and legacy `keygen` perform a bounded real push/pop and acknowledge
the flag when present. Category mismatch is an unconditional internal assertion,
not malformed-input behavior.

Every AE9 start-tag route finalizes the original tokenizer self-closing flag at
the dispatch boundary with the private
`SelfClosingFlagDisposition::{Acknowledge, LeaveUnacknowledged}`. `form`,
`textarea`, `button`, `fieldset`, and the InTable `form` route leave it
unacknowledged; `input`, `keygen`, and the InTable hidden-`input` route
acknowledge it. Finalization records
`non-void-html-element-start-tag-with-trailing-solidus` exactly for a set flag
with `LeaveUnacknowledged`, after any tag-algorithm error. This is also reached
when an AE9 algorithm deterministically ignores a token, such as duplicate
forms. Textarea supplies its originating `InBody` mode because the handler has
already entered Text mode by finalization.

The subsequent AE9b select work generalizes the issue-specific helper name to
`finalize_html_start_tag_self_closing_flag` without moving ownership out of
dispatch. Existing AE9 callers and new select/input/HR callers still finalize
the original flag exactly once. This localized semantic rename is not the
repository-wide deprecated-insertion migration, which remains separate
follow-up work.

`max_open_elements_depth` limits retained non-void depth. A void transition may
observe one temporary additional entry, and the high-water metric records that
real depth. It cannot return, emit callbacks, process tokens, or expose a
snapshot while the entry remains present. Node/child limits are checked before
the transition; final order, counts, and foster cache are restored.

Exact-key removal updates stack counts/cache/counters and text coalescing but
does not detach a DOM node or emit `RemoveNode`. Form-pointer clearing, scope
validation, and exact removal remain separate operations. EOF does not force an
unclosed pointer to `None`.

## Supported algorithms

- `form` has full-document InBody start/end recovery and the supported InTable
  special path. The latter is form-dispatch orchestration: normal insert, set
  pointer, then exact-current stack removal.
- `input` is parser-created void insertion; only the `type=hidden` frameset
  distinction is modeled. AE9b adds the pinned full-document prelude: an
  in-scope select is diagnosed and popped before AFE reconstruction and input
  insertion. Direct-InTable hidden input remains its distinct table branch.
- `textarea` enters RCDATA/Text with identity-bound pending initial-LF state.
  Exactly one leading U+000A is suppressed by tree construction across chunks.
- `button` uses ordinary `ScopeKind::InScope`; button scope remains limited to
  algorithms such as paragraph recovery.
- `fieldset` participates in supported block-start/end recovery only.
- `keygen` is obsolete legacy parser compatibility only, with no runtime control
  semantics.

InTable template-specific behavior remains unsupported. The deterministic
template fallback records an error and ignores that form token; AE9a makes no
template or fragment parsing claim.

## State and fuzz observability

Form-pointer and pending-textarea-LF keys appear in state snapshots, progress
witnesses, and fuzz digests because they affect later token handling.

## Deliberate exclusions

AE9a excludes submission, reset, validation, form-owner reassociation beyond
the parser pointer, parser control values/checked state, disabled propagation,
focus, events, accessibility, layout, paint, JavaScript, `document.write`, full
DOM APIs, fragments, templates, and runtime select behavior. Static select
tree construction is defined separately by AE9b.

## Self-closing follow-up

The pre-AE9 conflated helper is private, deprecated, and frozen behind exact
`#[expect(deprecated)]` call-site expectations. AE9a establishes semantic
insertion for its own paths only; it does not claim global self-closing
conformance. Frozen legacy self-closing and known-void callers retain their
pre-AE9 attach-without-stack-transition behavior: no push/pop, stack high-water
change, or stack-cache transition. The AE9 real void transition applies only to
the new semantic `input` and `keygen` paths. Migration of all remaining calls
and helper deletion remains separate follow-up work.
