# Draft Contract — Adoption Agency Algorithm For Supported Formatting End Tags

Status: draft
Milestone: H — Active formatting elements + adoption agency algorithm

## Goal

Implement the HTML5 Adoption Agency Algorithm (AAA) for Borrowser's supported
formatting-element end tags, with deterministic AFE/SOE behavior,
identity-preserving runtime mutations, and chunk-equivalent patch semantics.

## Why This Contract Exists

Current Milestone H reconstruction work is integrated into `In body`, but
supported formatting end tags still flow through a transitional generic
scope-pop path. That is not the final HTML5 behavior for mis-nested formatting
content.

AAA is the algorithm that makes end-tag recovery coherent for the supported
formatting-element set and is the main remaining reason the formatting recovery
pipeline is still explicitly transitional.

## Current Boundary

Current behavior is intentionally incomplete:

- supported formatting end tags still use generic scope-pop closure
- reconstruction is validated only against that transitional path
- identity-preserving structural move semantics remain governed by
  [`ADR-002`](../adr/ADR-002-runtime-patch-move-semantics.md)

This contract resolves the supported formatting end-tag side of that boundary.

## Required Behavior

- enter AAA for supported formatting end tags only
- perform marker-bounded AFE searches and deterministic SOE scans
- honor the HTML5 outer-loop bound of 8 iterations
- preserve the spec-defined inner-loop behavior and thresholds
- select formatting element, furthest block, and common ancestor through
  explicit deterministic scans
- keep AFE and SOE synchronized when entries are removed, replaced, or adopted
- emit direct `DomPatch` mutations that preserve identity for moved nodes
  according to the runtime move-semantics contract

## Acceptance Criteria

- supported formatting end tags no longer rely on the generic transitional
  scope-pop path
- mis-nested formatting cases recover through AAA with deterministic final DOM
- patch streams remain deterministic and chunk-equivalent for representative
  AAA-heavy cases
- AFE entries never retain dangling keys across adoption/replacement steps
- golden fixtures and targeted tests cover representative mis-nesting patterns

## Evidence Expectations

- golden DOM fixtures for mis-nested formatting recovery
- golden patch fixtures that demonstrate deterministic creation/move ordering
- explicit chunk-parity coverage for representative AAA cases
- tests that exercise AFE/SOE synchronization during adoption/replacement
- representative cases added here should also be reflected in the Milestone H
  WPT/policy tracking as coverage expands

## Dependencies

- builds on [`ADR-002 — Runtime Patch Move Semantics For AAA-Compatible Structural Reparenting`](../adr/ADR-002-runtime-patch-move-semantics.md)
- builds on [`Contract — Reconstruct Active Formatting Elements`](reconstruct-active-formatting-elements.md)
- complements the special `a` / `nobr` start-tag work tracked by
  [`Contract — Special a / nobr recovery paths`](special-a-nobr-recovery.md)
