// Copyright 2025 Agentic-Insights
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # ycbust
//!
//! A library for downloading and extracting the YCB (Yale-CMU-Berkeley) Object and Model Set
//! for 3D rendering and robotic simulation environments.
//!
//! ## Example
//!
//! ```no_run
//! use ycbust::{download_ycb, Subset, DownloadOptions};
//! use std::path::Path;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Download representative objects with default options
//!     download_ycb(
//!         Subset::Representative,
//!         Path::new("/tmp/ycb"),
//!         DownloadOptions::default(),
//!     ).await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Lower-level API
//!
//! For more control, you can use the individual functions:
//!
//! ```no_run
//! use ycbust::{fetch_objects, get_tgz_url, download_file, extract_tgz};
//! use reqwest::Client;
//! use std::path::Path;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let client = Client::new();
//!
//!     // Fetch available objects
//!     let objects = fetch_objects(&client).await?;
//!     println!("Available objects: {:?}", objects);
//!
//!     // Get URL for a specific object
//!     let url = get_tgz_url("003_cracker_box", "google_16k");
//!
//!     // Download and extract
//!     let dest = Path::new("/tmp/ycb/003_cracker_box_google_16k.tgz");
//!     download_file(&client, &url, dest, true).await?;
//!     extract_tgz(dest, Path::new("/tmp/ycb"), true)?;
//!
//!     Ok(())
//! }
//! ```
//!
mod error;
pub use error::{Result, YcbError};

use futures_util::stream::{self, StreamExt, TryStreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::Deserialize;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

/// Base URL for the YCB dataset on S3.
pub const BASE_URL: &str = "https://ycb-benchmarks.s3.amazonaws.com/data/";

/// URL for the objects index JSON file.
pub const OBJECTS_URL: &str = "https://ycb-benchmarks.s3.amazonaws.com/data/objects.json";

/// Relative path from a per-object directory to the `google_16k` mesh file.
///
/// Useful when callers already have an `object_dir` and want to compose the
/// final path themselves: `object_dir.join(GOOGLE_16K_MESH_RELATIVE)`.
pub const GOOGLE_16K_MESH_RELATIVE: &str = "google_16k/textured.obj";

/// Relative path from a per-object directory to the `google_16k` texture file.
pub const GOOGLE_16K_TEXTURE_RELATIVE: &str = "google_16k/texture_map.png";

/// Representative subset of 3 commonly used objects.
pub const REPRESENTATIVE_OBJECTS: &[&str] =
    &["003_cracker_box", "004_sugar_box", "005_tomato_soup_can"];

/// TBP standard 10-object benchmark set (distinct objects).
///
/// The canonical object set used by the Thousand Brains Project for their
/// standard accuracy benchmark. Objects chosen for geometric and color diversity.
///
/// Source: `tbp.monty` conf/.../ycb/distinct_objects.yaml
pub const TBP_STANDARD_OBJECTS: &[&str] = &[
    "025_mug",
    "024_bowl",
    "010_potted_meat_can",
    "031_spoon",
    "012_strawberry",
    "006_mustard_bottle",
    "062_dice",
    "058_golf_ball",
    "073-c_lego_duplo",
    "011_banana",
];

/// TBP similar 10-object benchmark set (harder — similar geometry).
///
/// Used by the Thousand Brains Project for harder discrimination benchmarks.
/// Objects have similar geometric features, requiring finer discrimination.
///
/// Source: `tbp.monty` conf/.../ycb/similar_objects.yaml
pub const TBP_SIMILAR_OBJECTS: &[&str] = &[
    "003_cracker_box",
    "004_sugar_box",
    "009_gelatin_box",
    "021_bleach_cleanser",
    "036_wood_block",
    "039_key",
    "040_large_marker",
    "051_large_clamp",
    "052_extra_large_clamp",
    "061_foam_brick",
];

/// Subset of objects to download.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum Subset {
    /// 3 representative objects (default).
    #[default]
    Representative,
    /// TBP standard 10-object benchmark set (distinct objects).
    ///
    /// The canonical set used by the Thousand Brains Project for standard accuracy benchmarks.
    TbpStandard,
    /// TBP similar 10-object benchmark set (harder discrimination).
    TbpSimilar,
    /// All available objects (~77).
    All,
}

/// Options for downloading YCB objects.
///
/// Marked `#[non_exhaustive]` so adding new fields is not a breaking change.
/// Construct with `DownloadOptions::default()` and override the fields you
/// care about, or use the `..Default::default()` struct-update syntax.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct DownloadOptions {
    /// Overwrite existing files.
    pub overwrite: bool,
    /// Download all file types (berkeley_processed, google_16k, etc.).
    /// If false, only downloads google_16k.
    pub full: bool,
    /// Show progress bars during download.
    pub show_progress: bool,
    /// Delete archive files after extraction.
    pub delete_archives: bool,
    /// Maximum number of concurrent per-object downloads.
    ///
    /// Default is `1` for behavior-preserving compatibility. Increase to
    /// parallelise large subset downloads (e.g. `4` is a sensible value for
    /// `Subset::TbpStandard`/`TbpSimilar`/`All`). Values of `0` are clamped
    /// to `1`.
    pub concurrency: usize,
    /// Verify download integrity after each archive completes.
    ///
    /// When `true` (default), the on-disk size of a cached archive is
    /// compared against the server-reported `Content-Length` on resume; a
    /// mismatch triggers a re-download. Disable to favour speed over
    /// correctness when network round trips are expensive.
    pub verify_integrity: bool,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            overwrite: false,
            full: false,
            show_progress: true,
            delete_archives: true,
            concurrency: 1,
            verify_integrity: true,
        }
    }
}

/// Response from the YCB objects API.
#[derive(Deserialize, Debug)]
struct ObjectsResponse {
    objects: Vec<String>,
}

pub(crate) async fn selected_objects_for_subset(
    subset: Subset,
    client: &Client,
) -> Result<Vec<String>> {
    match get_subset_objects(subset) {
        Some(objects) => Ok(objects),
        None => fetch_objects(client).await,
    }
}

fn download_file_types(full: bool) -> &'static [&'static str] {
    if full {
        &["berkeley_processed", "google_16k"]
    } else {
        &["google_16k"]
    }
}

fn local_artifact_exists(output_dir: &Path, object: &str, file_type: &str) -> bool {
    match file_type {
        "google_16k" => object_mesh_path(output_dir, object).exists(),
        _ => false,
    }
}

/// Fetches the list of available objects from the YCB dataset.
///
/// # Example
///
/// ```no_run
/// use ycbust::fetch_objects;
/// use reqwest::Client;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let client = Client::new();
///     let objects = fetch_objects(&client).await?;
///     println!("Found {} objects", objects.len());
///     Ok(())
/// }
/// ```
pub async fn fetch_objects(client: &Client) -> Result<Vec<String>> {
    let response = client.get(OBJECTS_URL).send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(YcbError::HttpStatus {
            status: status.as_u16(),
            url: OBJECTS_URL.to_string(),
        });
    }
    let body = response.text().await?;
    let objects_response: ObjectsResponse = serde_json::from_str(&body)
        .map_err(|e| YcbError::InvalidResponse(format!("YCB objects index: {e}")))?;
    Ok(objects_response.objects)
}

/// Constructs the download URL for a specific object and file type.
///
/// # Arguments
///
/// * `object` - The object name (e.g., "003_cracker_box")
/// * `file_type` - The file type (e.g., "google_16k", "berkeley_processed", "berkeley_rgbd")
///
/// # Example
///
/// ```
/// use ycbust::get_tgz_url;
///
/// let url = get_tgz_url("003_cracker_box", "google_16k");
/// assert!(url.contains("google/003_cracker_box_google_16k.tgz"));
/// ```
pub fn get_tgz_url(object: &str, file_type: &str) -> String {
    if file_type == "berkeley_rgbd" || file_type == "berkeley_rgb_highres" {
        format!(
            "{}berkeley/{}/{}_{}.tgz",
            BASE_URL, object, object, file_type
        )
    } else if file_type == "berkeley_processed" {
        format!(
            "{}berkeley/{}/{}_berkeley_meshes.tgz",
            BASE_URL, object, object
        )
    } else {
        format!("{}google/{}_{}.tgz", BASE_URL, object, file_type)
    }
}

/// Downloads a file from a URL to the specified destination path.
///
/// # Arguments
///
/// * `client` - The reqwest client to use for the download
/// * `url` - The URL to download from
/// * `dest_path` - The local path to save the file to
/// * `show_progress` - Whether to show a progress bar
///
/// # Example
///
/// ```no_run
/// use ycbust::download_file;
/// use reqwest::Client;
/// use std::path::Path;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let client = Client::new();
///     let url = "https://example.com/file.tgz";
///     download_file(&client, url, Path::new("/tmp/file.tgz"), true).await?;
///     Ok(())
/// }
/// ```
pub async fn download_file(
    client: &Client,
    url: &str,
    dest_path: &Path,
    show_progress: bool,
) -> Result<()> {
    download_file_inner(client, url, dest_path, show_progress, None).await
}

async fn download_file_inner(
    client: &Client,
    url: &str,
    dest_path: &Path,
    show_progress: bool,
    multi: Option<&MultiProgress>,
) -> Result<()> {
    let res = client.get(url).send().await?;
    let status = res.status();
    if !status.is_success() {
        return Err(YcbError::HttpStatus {
            status: status.as_u16(),
            url: url.to_string(),
        });
    }
    let total_size = res.content_length().unwrap_or(0);
    let filename = dest_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let pb = if show_progress {
        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
                .expect("Invalid progress bar template - this is a bug")
                .progress_chars("#>-"),
        );
        pb.set_message(format!("Downloading {}", filename));
        Some(match multi {
            Some(m) => m.add(pb),
            None => pb,
        })
    } else {
        None
    };

    let mut file = BufWriter::new(File::create(dest_path)?);
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk)?;
        if let Some(ref pb) = pb {
            pb.inc(chunk.len() as u64);
        }
    }

    file.flush()?;

    if let Some(pb) = pb {
        pb.finish_with_message("Done");
    }
    Ok(())
}

/// Extracts a .tgz (gzip-compressed tar) archive to the specified output directory.
///
/// This function includes security hardening against path traversal attacks.
///
/// # Arguments
///
/// * `tgz_path` - Path to the .tgz file to extract
/// * `output_dir` - Directory to extract files into
/// * `delete_archive` - Whether to delete the archive file after extraction
///
/// # Example
///
/// ```no_run
/// use ycbust::extract_tgz;
/// use std::path::Path;
///
/// fn main() -> anyhow::Result<()> {
///     extract_tgz(
///         Path::new("/tmp/file.tgz"),
///         Path::new("/tmp/output"),
///         true,
///     )?;
///     Ok(())
/// }
/// ```
pub fn extract_tgz(tgz_path: &Path, output_dir: &Path, delete_archive: bool) -> Result<()> {
    let tgz_str = tgz_path.display().to_string();
    fs::create_dir_all(output_dir)?;
    let canonical_output = output_dir
        .canonicalize()
        .unwrap_or_else(|_| output_dir.to_path_buf());

    let tar_gz = File::open(tgz_path)?;
    let tar = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(tar);

    let entries = archive
        .entries()
        .map_err(|e| YcbError::extraction(&tgz_str, e))?;

    for entry in entries {
        let mut entry = entry.map_err(|e| YcbError::extraction(&tgz_str, e))?;
        let path = entry
            .path()
            .map_err(|e| YcbError::extraction(&tgz_str, e))?
            .to_path_buf();

        if path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(YcbError::UnsafeArchive(format!(
                "archive entry contains '..': {}",
                path.display()
            )));
        }

        let dest = output_dir.join(&path);

        if let Ok(canonical_dest) = dest.canonicalize() {
            if !canonical_dest.starts_with(&canonical_output) {
                return Err(YcbError::UnsafeArchive(format!(
                    "archive entry escapes output dir: {}",
                    dest.display()
                )));
            }
        }

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        entry
            .unpack(&dest)
            .map_err(|e| YcbError::extraction(&tgz_str, e))?;
    }

    if delete_archive {
        fs::remove_file(tgz_path)?;
    }
    Ok(())
}

/// Checks if a URL exists by sending a HEAD request.
///
/// # Arguments
///
/// * `client` - The reqwest client to use
/// * `url` - The URL to check
///
/// # Returns
///
/// `true` if the URL returns a successful status code, `false` otherwise.
pub async fn url_exists(client: &Client, url: &str) -> Result<bool> {
    let response = client.head(url).send().await?;
    Ok(response.status().is_success())
}

/// Returns the server-reported `Content-Length` for `url`, if any.
///
/// `Ok(None)` means the server did not return a `Content-Length` header
/// (or returned a value that wasn't a parsable `u64`); the integrity check
/// is then skipped for that URL.
async fn fetch_content_length(client: &Client, url: &str) -> Result<Option<u64>> {
    let response = client.head(url).send().await?;
    if !response.status().is_success() {
        return Ok(None);
    }
    Ok(response.content_length())
}

/// High-level function to download YCB objects.
///
/// This is the recommended entry point for most use cases. It handles
/// fetching the object list, filtering by subset, downloading, and extracting.
///
/// # Arguments
///
/// * `subset` - Which subset of objects to download
/// * `output_dir` - Directory to save downloaded files to
/// * `options` - Download options (overwrite, full, progress, etc.)
///
/// # Example
///
/// ```no_run
/// use ycbust::{download_ycb, Subset, DownloadOptions};
/// use std::path::Path;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     download_ycb(
///         Subset::Representative,
///         Path::new("/tmp/ycb"),
///         DownloadOptions::default(),
///     ).await?;
///     Ok(())
/// }
/// ```
pub async fn download_ycb(
    subset: Subset,
    output_dir: &Path,
    options: DownloadOptions,
) -> Result<()> {
    let client = Client::new();
    let selected_objects = selected_objects_for_subset(subset, &client).await?;
    let refs: Vec<&str> = selected_objects.iter().map(String::as_str).collect();
    download_objects(&refs, output_dir, options).await
}

async fn process_work_item(
    client: &Client,
    output_dir: &Path,
    options: &DownloadOptions,
    multi: Option<&MultiProgress>,
    object: &str,
    file_type: &'static str,
) -> Result<()> {
    // Fast paths first: if the extracted artifact is already on disk we can
    // skip without a network round trip, even with `verify_integrity = true`.
    // This matters for callers that keep `delete_archives = false` and for
    // warm-cache `Subset::All` runs (up to 77 HEAD requests saved).
    if !options.overwrite && local_artifact_exists(output_dir, object, file_type) {
        return Ok(());
    }

    let filename = format!("{}_{}.tgz", object, file_type);
    let dest_path = output_dir.join(&filename);
    let url = get_tgz_url(object, file_type);

    let mut have_valid_archive = false;
    if !options.overwrite && dest_path.exists() {
        if options.verify_integrity {
            match fetch_content_length(client, &url).await? {
                Some(expected) => {
                    let actual = std::fs::metadata(&dest_path)?.len();
                    if actual == expected {
                        have_valid_archive = true;
                    } else {
                        // Stale / partial cache — drop it so we re-fetch below.
                        let _ = std::fs::remove_file(&dest_path);
                    }
                }
                None => {
                    have_valid_archive = true;
                }
            }
        } else {
            have_valid_archive = true;
        }
    }

    if !options.overwrite && have_valid_archive {
        return Ok(());
    }

    match download_file_inner(client, &url, &dest_path, options.show_progress, multi).await {
        Ok(()) => {}
        Err(YcbError::HttpStatus { status: 404, .. }) => return Ok(()),
        Err(err) => return Err(err),
    }

    extract_tgz(&dest_path, output_dir, options.delete_archives)?;
    Ok(())
}

/// Downloads an arbitrary list of YCB objects by ID.
///
/// Same per-object pipeline as [`download_ycb`] (skip-when-cached, fetch,
/// extract), but bypasses the [`Subset`] indirection so callers can pass an
/// ad-hoc list — e.g. a single object selected by a render harness.
///
/// Resume detection skips an object's `google_16k` slot when either the
/// cached `.tgz` archive or the extracted `textured.obj` mesh is already on
/// disk (unless `options.overwrite` is set).
///
/// # Example
///
/// ```no_run
/// use ycbust::{download_objects, DownloadOptions};
/// use std::path::Path;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     download_objects(
///         &["006_mustard_bottle", "011_banana"],
///         Path::new("/tmp/ycb"),
///         DownloadOptions::default(),
///     )
///     .await?;
///     Ok(())
/// }
/// ```
pub async fn download_objects(
    objects: &[&str],
    output_dir: &Path,
    options: DownloadOptions,
) -> Result<()> {
    if objects.is_empty() {
        return Ok(());
    }

    let client = Client::new();
    fs::create_dir_all(output_dir).map_err(YcbError::Io)?;

    let file_types = download_file_types(options.full);
    let concurrency = options.concurrency.max(1);
    let multi = if options.show_progress && concurrency > 1 {
        Some(MultiProgress::new())
    } else {
        None
    };

    let work: Vec<(&str, &'static str)> = objects
        .iter()
        .flat_map(|o| file_types.iter().map(move |ft| (*o, *ft)))
        .collect();

    stream::iter(work)
        .map(|(object, file_type)| {
            let client = &client;
            let multi = multi.as_ref();
            let options = &options;
            async move {
                process_work_item(client, output_dir, options, multi, object, file_type).await
            }
        })
        .buffer_unordered(concurrency)
        .try_for_each(|_| async { Ok::<(), YcbError>(()) })
        .await?;

    Ok(())
}

/// Returns the list of objects for a given subset without fetching from the network.
///
/// For `Subset::All`, this returns `None` since the full list requires a network fetch.
///
/// # Example
///
/// ```
/// use ycbust::{get_subset_objects, Subset};
///
/// let objects = get_subset_objects(Subset::Representative);
/// assert_eq!(objects, Some(vec![
///     "003_cracker_box".to_string(),
///     "004_sugar_box".to_string(),
///     "005_tomato_soup_can".to_string(),
/// ]));
///
/// let all = get_subset_objects(Subset::All);
/// assert_eq!(all, None); // Requires network fetch
/// ```
pub fn get_subset_objects(subset: Subset) -> Option<Vec<String>> {
    match subset {
        Subset::Representative => Some(
            REPRESENTATIVE_OBJECTS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ),
        Subset::TbpStandard => Some(TBP_STANDARD_OBJECTS.iter().map(|s| s.to_string()).collect()),
        Subset::TbpSimilar => Some(TBP_SIMILAR_OBJECTS.iter().map(|s| s.to_string()).collect()),
        Subset::All => None,
    }
}

/// Returns the expected path to an object's mesh file.
///
/// # Example
/// ```
/// use ycbust::object_mesh_path;
/// use std::path::Path;
/// let p = object_mesh_path(Path::new("/tmp/ycb"), "006_mustard_bottle");
/// assert_eq!(
///     p,
///     Path::new("/tmp/ycb")
///         .join("006_mustard_bottle")
///         .join("google_16k")
///         .join("textured.obj")
/// );
/// ```
pub fn object_mesh_path(ycb_dir: &Path, object: &str) -> std::path::PathBuf {
    ycb_dir.join(object).join(GOOGLE_16K_MESH_RELATIVE)
}

/// Returns the expected path to an object's texture file.
pub fn object_texture_path(ycb_dir: &Path, object: &str) -> std::path::PathBuf {
    ycb_dir.join(object).join(GOOGLE_16K_TEXTURE_RELATIVE)
}

/// Result of validating a single object.
#[derive(Debug, Clone)]
pub struct ObjectValidation {
    /// Object name (e.g. "006_mustard_bottle")
    pub name: String,
    /// Whether the mesh file exists
    pub mesh_present: bool,
    /// Whether the texture file exists
    pub texture_present: bool,
}

impl ObjectValidation {
    /// Returns `true` if the object is fully present (mesh + texture).
    pub fn is_complete(&self) -> bool {
        self.mesh_present && self.texture_present
    }
}

/// Validates that YCB objects are present and complete in the given directory.
///
/// Checks each object in the provided list for the existence of the
/// `google_16k/textured.obj` mesh and `google_16k/texture_map.png` texture.
///
/// # Example
/// ```no_run
/// use ycbust::{validate_objects, TBP_STANDARD_OBJECTS};
/// use std::path::Path;
/// let results = validate_objects(Path::new("/tmp/ycb"), TBP_STANDARD_OBJECTS);
/// let missing: Vec<_> = results.iter().filter(|r| !r.is_complete()).collect();
/// println!("{} objects missing", missing.len());
/// ```
pub fn validate_objects(ycb_dir: &Path, objects: &[&str]) -> Vec<ObjectValidation> {
    objects
        .iter()
        .map(|name| ObjectValidation {
            name: name.to_string(),
            mesh_present: object_mesh_path(ycb_dir, name).exists(),
            texture_present: object_texture_path(ycb_dir, name).exists(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tgz_url_google_16k() {
        let url = get_tgz_url("003_cracker_box", "google_16k");
        assert_eq!(
            url,
            "https://ycb-benchmarks.s3.amazonaws.com/data/google/003_cracker_box_google_16k.tgz"
        );
    }

    #[test]
    fn test_get_tgz_url_berkeley_processed() {
        let url = get_tgz_url("003_cracker_box", "berkeley_processed");
        assert_eq!(
            url,
            "https://ycb-benchmarks.s3.amazonaws.com/data/berkeley/003_cracker_box/003_cracker_box_berkeley_meshes.tgz"
        );
    }

    #[test]
    fn test_get_tgz_url_berkeley_rgbd() {
        let url = get_tgz_url("003_cracker_box", "berkeley_rgbd");
        assert_eq!(
            url,
            "https://ycb-benchmarks.s3.amazonaws.com/data/berkeley/003_cracker_box/003_cracker_box_berkeley_rgbd.tgz"
        );
    }

    #[test]
    fn test_get_tgz_url_berkeley_rgb_highres() {
        let url = get_tgz_url("003_cracker_box", "berkeley_rgb_highres");
        assert_eq!(
            url,
            "https://ycb-benchmarks.s3.amazonaws.com/data/berkeley/003_cracker_box/003_cracker_box_berkeley_rgb_highres.tgz"
        );
    }

    #[test]
    fn test_get_tgz_url_different_objects() {
        let url1 = get_tgz_url("004_sugar_box", "google_16k");
        assert!(url1.contains("004_sugar_box"));

        let url2 = get_tgz_url("005_tomato_soup_can", "google_16k");
        assert!(url2.contains("005_tomato_soup_can"));
    }

    #[test]
    fn test_subset_default() {
        let subset = Subset::default();
        assert_eq!(subset, Subset::Representative);
    }

    #[test]
    fn test_download_options_default() {
        let options = DownloadOptions::default();
        assert!(!options.overwrite);
        assert!(!options.full);
        assert!(options.show_progress);
        assert!(options.delete_archives);
        assert_eq!(options.concurrency, 1);
        assert!(options.verify_integrity);
    }

    #[test]
    fn test_get_subset_objects_representative() {
        let objects = get_subset_objects(Subset::Representative);
        assert_eq!(objects.unwrap().len(), 3);
    }

    #[test]
    fn test_get_subset_objects_tbp_standard() {
        let objects = get_subset_objects(Subset::TbpStandard);
        assert_eq!(objects.unwrap().len(), 10);
    }

    #[test]
    fn test_get_subset_objects_tbp_similar() {
        let objects = get_subset_objects(Subset::TbpSimilar);
        assert_eq!(objects.unwrap().len(), 10);
    }

    #[test]
    fn test_get_subset_objects_all() {
        let objects = get_subset_objects(Subset::All);
        assert!(objects.is_none());
    }

    #[test]
    fn test_local_artifact_exists_for_google_16k_mesh() {
        let dir = tempfile::tempdir().unwrap();
        let mesh_path = object_mesh_path(dir.path(), "003_cracker_box");
        fs::create_dir_all(mesh_path.parent().unwrap()).unwrap();
        File::create(&mesh_path).unwrap();

        assert!(local_artifact_exists(
            dir.path(),
            "003_cracker_box",
            "google_16k"
        ));
        assert!(!local_artifact_exists(
            dir.path(),
            "003_cracker_box",
            "berkeley_processed"
        ));
    }

    #[test]
    fn test_path_consts_compose_with_object_helpers() {
        let root = Path::new("ycb-root");
        let object = "006_mustard_bottle";

        assert_eq!(
            object_mesh_path(root, object),
            root.join(object).join(GOOGLE_16K_MESH_RELATIVE)
        );
        assert_eq!(
            object_texture_path(root, object),
            root.join(object).join(GOOGLE_16K_TEXTURE_RELATIVE)
        );
    }

    #[test]
    fn test_path_consts_have_expected_values() {
        assert_eq!(GOOGLE_16K_MESH_RELATIVE, "google_16k/textured.obj");
        assert_eq!(GOOGLE_16K_TEXTURE_RELATIVE, "google_16k/texture_map.png");
    }

    #[tokio::test]
    async fn test_download_objects_empty_slice_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let result = download_objects(&[], dir.path(), DownloadOptions::default()).await;
        assert!(result.is_ok());
        // Output dir should not even be created for an empty list.
        let entries = fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(entries, 0);
    }

    #[tokio::test]
    async fn test_download_objects_skips_when_mesh_present() {
        let dir = tempfile::tempdir().unwrap();
        let mesh_path = object_mesh_path(dir.path(), "003_cracker_box");
        fs::create_dir_all(mesh_path.parent().unwrap()).unwrap();
        File::create(&mesh_path).unwrap();

        // No network call should happen because the mesh is already on disk.
        // If skip logic regresses, this hits the network and fails in offline CI.
        let options = DownloadOptions {
            show_progress: false,
            ..DownloadOptions::default()
        };
        let result = download_objects(&["003_cracker_box"], dir.path(), options).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_download_objects_mesh_skip_bypasses_head_even_with_archive_present() {
        // Regression guard: when both the archive and the extracted mesh are
        // present, the mesh-exists check must short-circuit before the
        // integrity HEAD request runs. If this test ever needs the network
        // to pass, the skip ordering in process_work_item has regressed.
        let dir = tempfile::tempdir().unwrap();
        let object = "003_cracker_box";

        // Stub archive (wrong size — would fail integrity if we ever hit it)
        let archive_path = dir.path().join(format!("{object}_google_16k.tgz"));
        let mut f = File::create(&archive_path).unwrap();
        f.write_all(b"not a real archive").unwrap();

        // Extracted mesh also present
        let mesh_path = object_mesh_path(dir.path(), object);
        fs::create_dir_all(mesh_path.parent().unwrap()).unwrap();
        File::create(&mesh_path).unwrap();

        let options = DownloadOptions {
            show_progress: false,
            verify_integrity: true,
            ..DownloadOptions::default()
        };
        let result = download_objects(&[object], dir.path(), options).await;
        assert!(result.is_ok());
        // Mesh-skip wins — archive is left untouched since we never entered
        // the integrity branch that would have deleted it.
        assert!(archive_path.exists());
    }

    #[tokio::test]
    async fn test_download_objects_concurrent_skips_when_all_meshes_present() {
        // Pre-populate meshes for the full TBP standard set; with
        // concurrency=4, the function should still skip every item
        // without any network calls.
        let dir = tempfile::tempdir().unwrap();
        for object in TBP_STANDARD_OBJECTS {
            let mesh_path = object_mesh_path(dir.path(), object);
            fs::create_dir_all(mesh_path.parent().unwrap()).unwrap();
            File::create(&mesh_path).unwrap();
        }

        let options = DownloadOptions {
            show_progress: false,
            concurrency: 4,
            ..DownloadOptions::default()
        };
        let refs: Vec<&str> = TBP_STANDARD_OBJECTS.to_vec();
        let result = download_objects(&refs, dir.path(), options).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_ycb_error_converts_to_anyhow() {
        let y = YcbError::HttpStatus {
            status: 404,
            url: "https://example.com".into(),
        };
        let a: anyhow::Error = y.into();
        assert!(a.to_string().contains("404"));
    }
}
