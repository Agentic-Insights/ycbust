// Copyright 2025 Agentic-Insights
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0

//! Typed error type for all `ycbust` public APIs.

use thiserror::Error;

/// Errors returned by the `ycbust` public API.
///
/// Marked `#[non_exhaustive]` so adding new variants is not a breaking change.
/// Implements `From<YcbError> for anyhow::Error` for callers using `anyhow`.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum YcbError {
    /// A network request failed (DNS, TLS, connection, body read).
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// An HTTP response returned a non-success status.
    #[error("http {status} for {url}")]
    HttpStatus {
        /// HTTP status code returned by the server.
        status: u16,
        /// URL that returned the status.
        url: String,
    },

    /// Tar / gzip extraction failed.
    #[error("extraction failed for {path}: {source}")]
    Extraction {
        /// Path of the archive that failed to extract.
        path: String,
        /// Underlying I/O error from the tar/gzip layer.
        #[source]
        source: std::io::Error,
    },

    /// A filesystem operation failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A downloaded artifact failed an integrity check (size / etag mismatch).
    #[error("integrity check failed for {path}: {reason}")]
    Integrity {
        /// Path to the artifact that failed verification.
        path: String,
        /// Human-readable reason (e.g. "expected 1024 bytes, got 512").
        reason: String,
    },

    /// JSON deserialization failed (e.g. parsing the YCB object index).
    #[error("invalid response: {0}")]
    InvalidResponse(String),

    /// An archive entry attempted a path traversal (contained `..` or
    /// canonicalized outside the output directory).
    #[error("unsafe archive entry: {0}")]
    UnsafeArchive(String),

    /// A wrapped error from a lower-level operation that doesn't fit a more
    /// specific variant. Prefer matching the concrete variants above; `Other`
    /// is the forward-compatibility escape hatch.
    ///
    /// # Nesting note
    ///
    /// Because `From<anyhow::Error> for YcbError` routes into this variant,
    /// round-tripping a `YcbError` through `anyhow::Error` and back will
    /// produce `Other(Other(...))`. This is intentional — the conversion
    /// is lossy by design so the concrete variants above can't be silently
    /// dropped. Callers who don't want the extra nesting should match on
    /// concrete variants before converting to `anyhow::Error`.
    #[error("{0}")]
    Other(#[source] anyhow::Error),
}

/// Convenience alias for `Result<T, YcbError>`.
pub type Result<T> = std::result::Result<T, YcbError>;

impl From<anyhow::Error> for YcbError {
    fn from(err: anyhow::Error) -> Self {
        YcbError::Other(err)
    }
}

impl YcbError {
    pub(crate) fn extraction(path: impl Into<String>, source: std::io::Error) -> Self {
        YcbError::Extraction {
            path: path.into(),
            source,
        }
    }
}
