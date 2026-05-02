# Agent Instructions

This checkout inherits the umbrella contract from `../AGENTS.md`.

- Read `../docs/workspace-map.md`, `../docs/agent-protocol.md`, `../docs/reversibility.md`, `../docs/rust-standard.md`, `../docs/diagnostics.md`, `../docs/machine-workability.md`, and `../docs/security-privacy.md` before non-trivial edits.
- Treat client packets, editor commands, diagnostics, assets, resource streams, and local files as untrusted boundary data until validated for schema, authority, freshness, size, and ownership.
- Keep diagnostics structured, redacted, and invisible until enabled. Do not commit `println!`, `dbg!`, or ad hoc stderr output.
- Prefer typed IDs, static labels, small data carriers, and focused rejection tests for protocol/runtime changes.
- Do not format local path dependencies from this repo. Use package-scoped validation such as `cargo fmt-check`, `cargo check -p <package>`, and focused `cargo test -p <package>`.
- Update the umbrella `../summary.md` after meaningful modifying turns.
