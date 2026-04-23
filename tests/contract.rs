//! Public-API contract tests, mirrored from
//! [bevy-sensor#61](https://github.com/killerapp/bevy-sensor/issues/61) so
//! ycbust catches its own surface drift before it reaches downstream
//! consumers.
//!
//! Each test pins one invariant the bevy-sensor / neocortx pipeline relies on.
//! If you intentionally break one of these, downstream consumers also need to
//! migrate — bump the major version and update this file in the same PR.

#![allow(clippy::let_underscore_future, dead_code)]

use std::path::Path;

use ycbust::{
    download_objects, get_subset_objects, get_tgz_url, object_mesh_path, object_texture_path,
    DownloadOptions, ObjectValidation, Result as YcbResult, Subset, YcbError,
    GOOGLE_16K_MESH_RELATIVE, GOOGLE_16K_TEXTURE_RELATIVE, REPRESENTATIVE_OBJECTS,
    TBP_SIMILAR_OBJECTS, TBP_STANDARD_OBJECTS,
};

// 1. Path consts have the literal expected values.
//    Defends against accidental Windows-style "google_16k\textured.obj"
//    or a `_highres` suffix being added.
#[test]
fn contract_google_16k_const_literal_values() {
    assert_eq!(GOOGLE_16K_MESH_RELATIVE, "google_16k/textured.obj");
    assert_eq!(GOOGLE_16K_TEXTURE_RELATIVE, "google_16k/texture_map.png");
}

// 2. Compose-invariant: the consts and the helpers must agree.
//    If they ever disagree, consumers using `object_dir.join(CONST)`
//    silently load the wrong file relative to what `object_mesh_path`
//    reports — a particularly nasty class of bug.
#[test]
fn contract_path_const_compose_matches_helpers() {
    let root = Path::new("ycb-root");
    let id = "006_mustard_bottle";
    let object_dir = root.join(id);

    assert_eq!(
        object_dir.join(GOOGLE_16K_MESH_RELATIVE),
        object_mesh_path(root, id)
    );
    assert_eq!(
        object_dir.join(GOOGLE_16K_TEXTURE_RELATIVE),
        object_texture_path(root, id)
    );
}

// 3. REPRESENTATIVE_OBJECTS is bevy-sensor's fixture sentinel set.
//    Exact-content assertion — `003_cracker_box` in particular is hardcoded
//    in `bevy-sensor::ycb::models_exist` as the presence sentinel.
#[test]
fn contract_representative_objects_exact_content() {
    assert_eq!(
        REPRESENTATIVE_OBJECTS,
        &["003_cracker_box", "004_sugar_box", "005_tomato_soup_can"]
    );
}

// 4. TBP standard and similar sets — count + a couple of canonical entries.
//    Length-only would silently allow content drift; canonical entries make
//    accidental swaps loud.
#[test]
fn contract_tbp_standard_set_canonical_entries() {
    assert_eq!(TBP_STANDARD_OBJECTS.len(), 10);
    assert!(TBP_STANDARD_OBJECTS.contains(&"025_mug"));
    assert!(TBP_STANDARD_OBJECTS.contains(&"011_banana"));
}

#[test]
fn contract_tbp_similar_set_canonical_entries() {
    assert_eq!(TBP_SIMILAR_OBJECTS.len(), 10);
    assert!(TBP_SIMILAR_OBJECTS.contains(&"003_cracker_box"));
    assert!(TBP_SIMILAR_OBJECTS.contains(&"051_large_clamp"));
}

// 5. YcbError exhaustive match — every named variant + `_` for the
//    `#[non_exhaustive]` future. Catches accidental variant *removal*; the
//    wildcard absorbs additions (which are deliberate, non-breaking).
#[test]
fn contract_ycb_error_exhaustive_match_with_non_exhaustive_fallthrough() {
    fn matches_all(e: &YcbError) -> &'static str {
        match e {
            YcbError::Network(_) => "network",
            YcbError::HttpStatus { .. } => "http",
            YcbError::Extraction { .. } => "extraction",
            YcbError::Io(_) => "io",
            YcbError::Integrity { .. } => "integrity",
            YcbError::InvalidResponse(_) => "invalid_response",
            YcbError::UnsafeArchive(_) => "unsafe_archive",
            YcbError::Other(_) => "other",
            // `#[non_exhaustive]` makes a wildcard required — that's the point.
            _ => "future_variant",
        }
    }

    assert_eq!(
        matches_all(&YcbError::HttpStatus {
            status: 404,
            url: "https://example.com".into(),
        }),
        "http"
    );
    assert_eq!(
        matches_all(&YcbError::Integrity {
            path: "x".into(),
            reason: "y".into(),
        }),
        "integrity"
    );
    assert_eq!(
        matches_all(&YcbError::UnsafeArchive("..".into())),
        "unsafe_archive"
    );
    assert_eq!(
        matches_all(&YcbError::InvalidResponse("bad json".into())),
        "invalid_response"
    );
    assert_eq!(
        matches_all(&YcbError::Other(anyhow::anyhow!("wrapped"))),
        "other"
    );
}

// 6. Bidirectional `From` between `YcbError` and `anyhow::Error`.
//    `YcbError -> anyhow::Error` keeps `?` working for anyhow callers.
//    `anyhow::Error -> YcbError` (via `Other`) lets internal helpers
//    that still use anyhow bubble up cleanly.
#[test]
fn contract_ycb_error_anyhow_round_trip() {
    let original = YcbError::HttpStatus {
        status: 502,
        url: "https://example.com".into(),
    };
    let as_anyhow: anyhow::Error = original.into();
    assert!(as_anyhow.to_string().contains("502"));

    let anyhow_err = anyhow::anyhow!("disk full");
    let as_ycb: YcbError = anyhow_err.into();
    assert!(matches!(as_ycb, YcbError::Other(_)));
}

// 7. `ycbust::Result<T>` alias is the same as `Result<T, YcbError>`.
//    Compile-only check — if these types ever diverge this won't build.
#[test]
fn contract_result_alias_equals_result_of_ycb_error() {
    fn _alias_check(r: YcbResult<()>) -> std::result::Result<(), YcbError> {
        r
    }
}

// 8. Blocking wrapper signatures. Compile-only — guards the `(args, ...) -> Result<(), YcbError>`
//    shape so a refactor of the `blocking` module can't silently change the surface.
#[cfg(feature = "blocking")]
#[test]
fn contract_blocking_signatures() {
    // Coerce to fn pointers with the exact expected signatures.
    let _by_subset: fn(Subset, &Path, DownloadOptions) -> YcbResult<()> =
        ycbust::blocking::download_ycb_blocking;
    let _by_objects: fn(&[&str], &Path, DownloadOptions) -> YcbResult<()> =
        ycbust::blocking::download_objects_blocking;
}

// Bonus — pin `download_objects` async signature. Compile-only: we never
// `.await` the future (would hit the network), but constructing it proves
// the signature `(&[&str], &Path, DownloadOptions) -> impl Future<Output = Result<(), YcbError>>`
// still holds.
#[test]
fn contract_download_objects_signature() {
    fn _shape_check() {
        let fut = download_objects(&[], Path::new("/dev/null"), DownloadOptions::default());
        // Drop without polling — purely a type check.
        let _: std::pin::Pin<Box<dyn std::future::Future<Output = YcbResult<()>>>> = Box::pin(fut);
    }
}

// And the `Subset` default lives where it should (matches what callers expect
// when they don't pass a subset).
#[test]
fn contract_subset_default_is_representative() {
    assert_eq!(Subset::default(), Subset::Representative);
}

// `DownloadOptions::default()` is the construction path consumers must use
// under `#[non_exhaustive]`. Pin the field values so a future Default impl
// change is loud.
#[test]
fn contract_download_options_default_field_values() {
    let o = DownloadOptions::default();
    assert!(!o.overwrite);
    assert!(!o.full);
    assert!(o.show_progress);
    assert!(o.delete_archives);
    assert_eq!(o.concurrency, 1);
    assert!(o.verify_integrity);
}

// `ObjectValidation` shape — bevy-sensor pattern-matches on these fields.
#[test]
fn contract_object_validation_shape() {
    let v = ObjectValidation {
        name: "003_cracker_box".to_string(),
        mesh_present: true,
        texture_present: false,
    };
    assert_eq!(v.name, "003_cracker_box");
    assert!(v.mesh_present);
    assert!(!v.texture_present);
    assert!(!v.is_complete());
}

// `get_subset_objects(All)` returns None — caller must fetch from network.
// `get_subset_objects(Representative)` returns the same exact list as the const.
#[test]
fn contract_get_subset_objects_invariants() {
    assert!(get_subset_objects(Subset::All).is_none());

    let rep: Vec<String> = REPRESENTATIVE_OBJECTS
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(get_subset_objects(Subset::Representative), Some(rep));
}

// `get_tgz_url` URL format for `google_16k`. Defends against accidental host
// or path-segment changes.
#[test]
fn contract_get_tgz_url_google_16k_format() {
    assert_eq!(
        get_tgz_url("003_cracker_box", "google_16k"),
        "https://ycb-benchmarks.s3.amazonaws.com/data/google/003_cracker_box_google_16k.tgz"
    );
}
