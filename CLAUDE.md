# CLAUDE.md

This repository is a public Rust utility for YCB downloads. It also serves as part of the effective system boundary for NeoCortx and bevy-sensor when YCB-based tests or render pipelines are involved.

## Mission

- Keep YCB acquisition fast, reliable, and simple for Rust users.
- Support downstream consumers, including NeoCortx's TBP parity workflow, by fixing YCB ownership issues here instead of pushing workarounds downstream.
- Prefer changes that improve efficiency, reliability, and release velocity.

## Sibling Repos

Common local layout:
- `../bevy-sensor`
- `../neocortx`

When those repos expose a YCB download, extraction, subset, or filesystem-layout issue:
- Fix it here if practical.
- If the fix is more involved, at least open a GitHub issue here so someone else can take it in parallel.
- Do not rely on downstream downgrades or indefinite local overrides as the steady state.

Issue command:
- `gh issue create --repo Agentic-Insights/ycbust`

## Public Repo Posture

- Keep `README.md` and release notes broadly useful for public consumers.
- Avoid repo-local docs that assume readers know NeoCortx internals unless that detail is necessary.
- Internal agent guidance can still note that downstream parity work is a major consumer.

## Commands

```sh
cargo build
cargo test
just
just build
just test
```

## Release Guidance

- This crate should ship small fixes quickly once verified.
- Prefer published releases over leaving downstream repos on path dependencies for long periods.
- If a change materially affects downstream consumers, note the expected follow-up release in the PR or issue.
