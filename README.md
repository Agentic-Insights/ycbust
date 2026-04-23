# YCB Downloader (`ycbust`)

[![Crates.io](https://img.shields.io/crates/v/ycbust.svg)](https://crates.io/crates/ycbust)
[![GitHub release](https://img.shields.io/github/v/release/Agentic-Insights/ycbust)](https://github.com/Agentic-Insights/ycbust/releases)

`ycbust` is a Rust library and CLI for downloading and extracting assets from the YCB Object and Model Set. It is aimed at rendering, robotics, and simulation workflows that need a predictable local YCB layout with minimal setup.

## Features

- Library and CLI interfaces for the same download/extract workflow
- Fast presets for `representative`, `tbp-standard`, `tbp-similar`, and `all`
- `google_16k` meshes by default, with `--full` for extra Berkeley assets
- Validation helpers for checking that benchmark objects are fully present
- Parallel downloads via `DownloadOptions::concurrency` (default 1)
- `Content-Length` integrity check on resume (toggle via `verify_integrity`)
- Typed `YcbError` enum so consumers can match on network / http / io / integrity / extraction failures
- Optional `s3` feature for streaming extracted assets directly to S3
- Optional `blocking` feature with sync wrappers for non-async callers

## Installation

Install the CLI from crates.io:

```bash
cargo install ycbust
```

Install with S3 support:

```bash
cargo install ycbust --features s3
```

Prebuilt binaries are also available on the [GitHub releases page](https://github.com/Agentic-Insights/ycbust/releases).

## Quick Start

The CLI uses subcommands. The default local output path is your OS temp directory plus `ycb`:

- Linux/macOS: `/tmp/ycb`
- Windows: `%TEMP%\ycb`

Download the default TBP standard subset:

```bash
ycbust download
```

Download a quick 3-object smoke-test set:

```bash
ycbust download --subset representative
```

Download specific objects to a custom directory:

```bash
ycbust download --output-dir ./data/ycb --objects 006_mustard_bottle 011_banana
```

Download all supported file types for the standard subset:

```bash
ycbust download --full
```

Validate a local dataset directory:

```bash
ycbust validate --output-dir ./data/ycb --subset tbp-standard
```

List the objects in a built-in subset:

```bash
ycbust list --subset tbp-similar
```

Fetch the full upstream object list from YCB S3:

```bash
ycbust list --subset all --fetch
```

## Subsets

- `representative`: 3 common objects for quick end-to-end checks
- `tbp-standard`: the TBP standard 10-object benchmark set
- `tbp-similar`: the TBP harder 10-object discrimination set
- `all`: every object advertised by the YCB dataset index

## Output Layout

For a typical `google_16k` download, `ycbust` produces:

```text
<output-dir>/
  003_cracker_box/
    google_16k/
      textured.obj
      texture_map.png
      textured.mtl
      ...
```

For rendering workflows, point your asset loader at `google_16k/textured.obj`. The relative path is also exposed as the `GOOGLE_16K_MESH_RELATIVE` constant for callers that already hold an `object_dir`.

## Library Usage

The crate can also be used directly from Rust:

```rust,no_run
use std::path::Path;
use ycbust::{download_ycb, DownloadOptions, Subset};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    download_ycb(
        Subset::TbpStandard,
        Path::new("./data/ycb"),
        DownloadOptions::default(),
    )
    .await?;

    Ok(())
}
```

For an ad-hoc list of object IDs (no `Subset` indirection), use `download_objects`:

```rust,no_run
use std::path::Path;
use ycbust::{download_objects, DownloadOptions};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    download_objects(
        &["006_mustard_bottle", "011_banana"],
        Path::new("./data/ycb"),
        DownloadOptions::default(),
    )
    .await?;
    Ok(())
}
```

To download in parallel and skip the integrity check:

```rust,no_run
use ycbust::DownloadOptions;

let mut options = DownloadOptions::default();
options.concurrency = 4;
options.verify_integrity = false;
```

### Performance note on `verify_integrity`

When `verify_integrity = true` (default), each cached `.tgz` archive triggers
one HEAD request on resume so the local size can be compared against the
server-reported `Content-Length`. Archives whose extracted `google_16k` mesh
is already on disk **do not** trigger a HEAD — the extracted artifact is the
fast path. Set `verify_integrity = false` for offline-ish workflows or if
you keep `delete_archives = false` and want to avoid a HEAD per object per
run; the tradeoff is that a truncated archive from an interrupted run will
look valid on the next pass.

For non-async callers, enable the `blocking` feature and use the synchronous wrappers:

```toml
[dependencies]
ycbust = { version = "0.4", features = ["blocking"] }
```

```rust,ignore
use ycbust::blocking::download_objects_blocking;
use ycbust::DownloadOptions;

download_objects_blocking(
    &["006_mustard_bottle"],
    std::path::Path::new("./data/ycb"),
    DownloadOptions::default(),
)?;
```

API docs: [docs.rs/ycbust](https://docs.rs/ycbust)

## Error handling

All public APIs return `Result<T, ycbust::YcbError>`. Match on the variants for granular handling:

```rust,no_run
use ycbust::{download_objects, DownloadOptions, YcbError};

# async fn run() {
match download_objects(&["006_mustard_bottle"], std::path::Path::new("./data/ycb"), DownloadOptions::default()).await {
    Ok(()) => {},
    Err(YcbError::Network(_)) | Err(YcbError::HttpStatus { .. }) => { /* retry */ },
    Err(YcbError::Io(_)) => { /* disk full, permissions, etc */ },
    Err(YcbError::Integrity { .. }) => { /* re-download or alert */ },
    Err(other) => { eprintln!("{other}"); },
}
# }
```

`anyhow` users get `From<YcbError> for anyhow::Error` for free.

## S3 Streaming

With the `s3` feature enabled, downloads can be extracted directly into an S3 bucket:

```bash
ycbust download --output-dir s3://my-bucket/ycb --subset tbp-standard --region us-east-1
```

Use `--profile <name>` if you do not want to rely on the default AWS credential chain.

## Development

This repo uses `just` for common tasks:

```bash
just test
just test-s3
just lint
just lint-s3
just ci
just ci-s3
```

This is a public utility crate, so changes should stay small, general-purpose, and easy to verify.
