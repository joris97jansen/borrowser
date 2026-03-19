# Milestone H Formatting WPT-Style Cases

This directory contains the curated Milestone H tree-construction cases used by
the `wpt_html5_formatting` slice.

Provenance policy:

- These files are WPT-style local conformance fixtures for Borrowser's current
  formatting/reconstruction/adoption-agency surface.
- They are stored under an upstream-shaped `vendor/html/syntax/parsing/...`
  path so they flow through the same manifest, policy, and runner machinery as
  the rest of the WPT subset.
- They are not currently claimed to be verbatim upstream WPT imports.
- When a real upstream WPT case covers the same behavior well enough, prefer
  replacing the local fixture with the upstream-derived file and record that
  provenance in `tests/wpt/README.md` or a nearby note.
