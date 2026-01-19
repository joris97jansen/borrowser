# HTML5 Entities Snapshot

This directory vendors the WHATWG HTML5 named entity list (`entities.json`).

The data is treated as **spec input**, not handwritten code. A generator script
produces a deterministic Rust lookup table that is checked into the repository.

---

## Normal workflow (recommended)

Use the Make targets from the repository root.

### Update the snapshot and regenerate the table (network required)

```bash
make html-entities-update
