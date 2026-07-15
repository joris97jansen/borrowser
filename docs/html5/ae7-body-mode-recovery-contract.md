# AE7: Body-Mode Recovery, Implied End Tags, And Formatting Foundation

Last updated: 2026-07-15
Status: Milestone AE implementation contract
Scope: `crates/html/src/html5/tree_builder`, `InBody` only

This document defines Borrowser's AE7 supported subset for malformed body
content recovery. AE7 is intentionally narrower than full WHATWG `In body`
tree-construction conformance.

## Ownership

AE7 recovery is owned by HTML tree construction. The parser consumes tokens,
updates insertion-mode state, mutates the stack of open elements, maintains the
active formatting elements list where already supported, and emits parser-owned
DOM patches.

CSS, Layout, Paint, and Browser/runtime must not recover malformed body markup,
reinterpret parser insertion modes, or repair parser-created DOM after the
fact.

## Supported Subset

AE7 supports implied-end-tag generation for:

- `p`
- `li`

AE10 extends the same deliberately bounded helper with `option` and
`optgroup`, including its existing exception parameter. That extension is
defined by the AE10 contract and does not broaden support to other implied-end
members.

The supported implied-end-tag helper is deliberately subset-scoped. It accepts
an optional excluded tag and pops only supported implied tags from the current
node position. It does not claim the full HTML implied-end-tag taxonomy.

AE7 supports paragraph auto-close before these body start tags:

- `address`
- `article`
- `aside`
- `blockquote`
- `div`
- `footer`
- `header`
- `h1` through `h6`
- `hr`
- `li`
- `main`
- `nav`
- `ol`
- `p`
- `pre`
- `section`
- `ul`

AE9a additionally places `fieldset` in the supported paragraph-closing block
start classification. Its generic block-end behavior remains ordinary InBody
scope recovery; this is not form runtime semantics.

`table` keeps its existing quirks-aware path and is not part of this shared
AE7 block-start helper.

AE7 supports list-item sibling recovery:

- a new `li` closes a previous `li` only when the previous `li` is in
  list-item scope;
- supported implied end tags are generated except `li` before closing the old
  `li`;
- if an open `p` is inside the old `li`, it is closed by the same supported
  implied-end-tag path;
- AE7 does not synthesize missing `ul` or `ol` elements.

## Unmatched `</p>`

When an end tag `</p>` is seen and no `p` element is in button scope, AE7:

1. records `in-body-p-end-tag-missing-p`;
2. inserts a missing parser-created `p` element through the normal tree-builder
   element insertion path;
3. if insertion succeeds, closes that `p` through the same paragraph-close
   helper used for matched paragraph end tags;
4. does not fall through to generic end-tag handling.

This means unmatched `</p>` is not silently ignored and does not emit the
generic `in-body-any-other-end-tag-blocked-by-special` diagnostic.

If normal resource-limit checks prevent synthetic `p` insertion, the parser
records the relevant resource-limit diagnostic and does not attempt a fake
close.

## Formatting Elements

AE7 does not broaden Borrowser's supported active-formatting or
adoption-agency tag set. Formatting participation remains limited to the
existing supported formatting elements:

- `a`
- `b`
- `big`
- `code`
- `em`
- `font`
- `i`
- `nobr`
- `s`
- `small`
- `strike`
- `strong`
- `tt`
- `u`

Paragraph and list recovery must not remove AFE entries merely because
formatting elements are popped from SOE while closing `p` or `li`. Existing
reconstruction then recreates supported active formatting elements when later
phrasing/text insertion requires it.

## Deterministic Parse Errors

AE7 uses deterministic tree-builder diagnostics for the supported malformed
cases:

- `in-body-p-end-tag-missing-p`
- `in-body-p-start-tag-closes-open-p`
- `in-body-p-end-tag-implied-close-mismatch`
- `in-body-block-start-tag-closes-open-p`
- `in-body-li-start-tag-closes-previous-li`
- `in-body-li-start-tag-implied-close-mismatch`
- `in-body-li-start-tag-closes-open-p`
- `in-body-li-end-tag-missing-li`
- `in-body-li-end-tag-implied-close-mismatch`
- `in-body-end-tag-implied-close-mismatch`
- `in-body-any-other-end-tag-blocked-by-special`

Outside the dedicated AE7 `p` and `li` paths, the current InBody “any other
end tag” algorithm performs one reverse SOE scan. This is not a scope lookup.
If the scan encounters a special HTML element before the requested target, it
records `in-body-any-other-end-tag-blocked-by-special` and ignores the token.
An absent target in the supported rooted full-document parser normally reaches
the special `html` root and takes that same malformed-input path. Exhausting
SOE instead is `EngineInvariantError`, not a parse error. If the target is
matched but is not the current node after supported implied-end processing,
`in-body-end-tag-implied-close-mismatch` remains the deterministic diagnostic.

`applet`, `marquee`, and `object` end tags do not use this generic algorithm.
They retain dedicated marker-end handling and the dedicated
`in-body-marker-end-tag-not-in-scope` and
`in-body-marker-end-tag-implied-close-mismatch` diagnostics.

Parse errors are regression/debug signals. They are not public runtime APIs.

## Patch And DOM Invariants

AE7 preserves:

- deterministic patch order;
- monotonic non-reused `PatchKey` allocation;
- live-tree and patch-derived DOM equivalence;
- stack-of-open-elements scope semantics;
- active-formatting marker and identity invariants;
- chunk-equivalent final DOM and patch semantics;
- no `Clear` or full-document rebuild recovery.

Closing elements affects parser state. It does not emit close patches.

## Deliberate Exclusions

AE7 does not implement:

- full WHATWG `In body` conformance;
- the full implied-end-tag taxonomy;
- full list parsing semantics;
- missing list container synthesis;
- heading auto-close behavior beyond paragraph-close classification;
- `pre` newline handling or frameset behavior;
- table/template/body/head mode expansion beyond later AE contracts; current
  select handling is owned by AE10 and does not add a select mode;
- broad WPT status promotion;
- CSS, Layout, Paint, Browser/runtime, or DOM post-processing recovery.
