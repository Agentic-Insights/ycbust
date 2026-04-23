// Copyright 2025 Agentic-Insights
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0

//! Blocking wrappers around the async download APIs.
//!
//! Behind the `blocking` cargo feature. Each function spins up a
//! single-threaded current-thread Tokio runtime internally and blocks on
//! the corresponding async function — useful for non-async callers
//! (CLI binaries, build scripts, sync simulators) that don't want to
//! manage a runtime themselves.
//!
//! # Example
//!
//! ```no_run
//! use ycbust::blocking::download_objects_blocking;
//! use ycbust::DownloadOptions;
//! use std::path::Path;
//!
//! fn main() -> anyhow::Result<()> {
//!     download_objects_blocking(
//!         &["006_mustard_bottle"],
//!         Path::new("/tmp/ycb"),
//!         DownloadOptions::default(),
//!     )?;
//!     Ok(())
//! }
//! ```

use std::path::Path;

use crate::{download_objects, download_ycb, DownloadOptions, Result, Subset};

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build a current-thread Tokio runtime");
    rt.block_on(fut)
}

/// Synchronous wrapper around [`download_ycb`].
pub fn download_ycb_blocking(
    subset: Subset,
    output_dir: &Path,
    options: DownloadOptions,
) -> Result<()> {
    block_on(download_ycb(subset, output_dir, options))
}

/// Synchronous wrapper around [`download_objects`].
pub fn download_objects_blocking(
    objects: &[&str],
    output_dir: &Path,
    options: DownloadOptions,
) -> Result<()> {
    block_on(download_objects(objects, output_dir, options))
}
