//! Compile-check for README code snippets that aren't reachable from doctests.
//!
//! If you change a README example, mirror the change here so drift gets caught
//! in CI.

#![allow(clippy::let_underscore_future, dead_code, unused_imports)]

// --- README §Error handling: match against YcbError variants
#[allow(dead_code)]
async fn readme_error_match_snippet() {
    use ycbust::{download_objects, DownloadOptions, YcbError};

    match download_objects(
        &["006_mustard_bottle"],
        std::path::Path::new("./data/ycb"),
        DownloadOptions::default(),
    )
    .await
    {
        Ok(()) => {}
        Err(YcbError::Network(_)) | Err(YcbError::HttpStatus { .. }) => { /* retry */ }
        Err(YcbError::Io(_)) => { /* disk full, permissions, etc */ }
        Err(other) => {
            eprintln!("{other}");
        }
    }
}

// --- README §Library Usage: parallel + no-integrity snippet
#[allow(dead_code)]
fn readme_parallel_noverify_snippet() {
    use ycbust::DownloadOptions;

    let mut options = DownloadOptions::default();
    options.concurrency = 4;
    options.verify_integrity = false;
    let _ = options; // silence unused in the compile-check
}

#[test]
fn readme_snippets_compile() {
    // Pure compile-check — constructing the helpers above wires them into
    // the test binary. We intentionally don't execute them (they'd need
    // network access and a writable /data/ycb).
    let _b = readme_parallel_noverify_snippet;
    let _c = readme_error_match_snippet;
}
