# ycbust Agent Guide

## Mission
`ycbust` is a public Rust utility crate and CLI for downloading and extracting YCB assets quickly and reliably. It supports downstream Rust render and simulation workflows, including NeoCortx's TBP parity stack, by keeping YCB acquisition simple, fast, and predictable.

## Relationship To Sibling Repos
- Common local sibling checkouts:
  - `../bevy-sensor`
  - `../neocortx`
- Treat those repos as downstream consumers, not as places to hide `ycbust` defects.

## Operating Posture
- This repository is public and user-facing, so README language should stay broadly useful.
- Agent guidance can acknowledge NeoCortx as a key downstream consumer, but public docs should stay oriented toward general Rust users.
- Favor small, direct fixes that improve reliability, download/extract performance, and developer speed.

## Ownership Rules
- Own YCB download, extraction, subset definitions, filesystem layout, and related CLI/library behavior here.
- Do not force downstream repos to downgrade or carry long-lived patches for issues that can be fixed in `ycbust`.
- If the fix is straightforward and unblocks downstream work, make it here directly.
- If the fix is nontrivial, open a GitHub issue here at minimum so downstream work can continue in parallel.

## Release Guidance
- `ycbust` is already public on crates.io, so release early and often once local verification passes.
- Prefer small, frequent releases over large bundled ones.
- Downstream consumers should move back to published crate versions quickly after a fix lands.

## Verification
- Run the smallest useful local test first.
- Keep the crate efficient and Rust-native; startup cost, download throughput, extraction time, and error handling quality all matter.
