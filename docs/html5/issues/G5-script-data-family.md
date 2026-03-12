# G5 — Implement Full HTML Script-Data State Family

Status: follow-up to the Core-v0 script text-mode subset
Milestone: G — Rawtext / RCDATA / script correctness

## Current Scope Boundary

The current tokenizer supports a Core-v0 script text-mode subset for
`<script>` that treats script contents as raw text until a matching
ASCII-case-insensitive `</script>` end tag is recognized, with chunk-safe
end-tag detection, literal EOF tail handling, and linear-time scanning.

This is intentionally narrower than the full HTML script-data state family.

## Follow-up Goal

Implement the dedicated script tokenizer state family rather than treating
script content as the shared text-mode subset.

The follow-up must add explicit state-machine handling for script-data
transitions triggered by `<`, end-tag openings, and script-specific
escaped/comment-like sequences, while preserving:

- chunk safety
- linear-time scanning
- deterministic token boundaries

## Out Of Scope For Core-v0 Subset

- script-data escaped states
- script-data double-escaped states
- legacy comment-like script tokenizer branches beyond the current subset
- parser pause/suspension and script execution integration
