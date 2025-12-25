// Copyright 2024-2025 Agentic-Insights
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

use anyhow::{Context, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::Deserialize;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

/// Base URL for the YCB dataset on S3.
pub const BASE_URL: &str = "https://ycb-benchmarks.s3.amazonaws.com/data/";

/// URL for the objects index JSON file.
pub const OBJECTS_URL: &str = "https://ycb-benchmarks.s3.amazonaws.com/data/objects.json";

/// Representative subset of 3 commonly used objects.
pub const REPRESENTATIVE_OBJECTS: &[&str] =
    &["003_cracker_box", "004_sugar_box", "005_tomato_soup_can"];

/// Subset of 10 commonly used objects.
pub const TEN_OBJECTS: &[&str] = &[
    "003_cracker_box",
    "004_sugar_box",
    "005_tomato_soup_can",
    "006_mustard_bottle",
    "007_tuna_fish_can",
    "008_pudding_box",
    "009_gelatin_box",
    "010_potted_meat_can",
    "011_banana",
    "019_pitcher_base",
];

/// Subset of objects to download.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum Subset {
    /// 3 representative objects (default).
    #[default]
    Representative,
    /// 10 representative objects.
    Ten,
    /// All available objects.
    All,
}

/// Options for downloading YCB objects.
#[derive(Clone, Debug)]
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
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            overwrite: false,
            full: false,
            show_progress: true,
            delete_archives: true,
        }
    }
}

/// Response from the YCB objects API.
#[derive(Deserialize, Debug)]
struct ObjectsResponse {
    objects: Vec<String>,
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
    let response = client
        .get(OBJECTS_URL)
        .send()
        .await
        .context("Failed to fetch objects list")?;
    let objects_response: ObjectsResponse = response
        .json()
        .await
        .context("Failed to parse objects JSON")?;
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
    let res = client
        .get(url)
        .send()
        .await
        .context("Failed to send request")?;
    let total_size = res.content_length().unwrap_or(0);
    let filename = dest_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let pb = if show_progress {
        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .expect("Invalid progress bar template - this is a bug")
                .progress_chars("#>-"),
        );
        pb.set_message(format!("Downloading {}", filename));
        Some(pb)
    } else {
        None
    };

    let mut file = File::create(dest_path).context("Failed to create file")?;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.context("Error while downloading chunk")?;
        file.write_all(&chunk)
            .context("Error while writing to file")?;
        if let Some(ref pb) = pb {
            pb.inc(chunk.len() as u64);
        }
    }

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
    let tar_gz = File::open(tgz_path)?;
    let tar = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(tar);

    // Validate and extract each entry to prevent path traversal attacks
    for entry in archive
        .entries()
        .context("Failed to read archive entries")?
    {
        let mut entry = entry.context("Failed to read archive entry")?;
        let path = entry
            .path()
            .context("Failed to get entry path")?
            .to_path_buf();

        // Reject paths with parent directory components (..)
        if path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            anyhow::bail!(
                "Archive contains invalid path with '..': {}",
                path.display()
            );
        }

        let dest = output_dir.join(&path);

        // Ensure destination is within output_dir (canonicalization check)
        let canonical_output = output_dir
            .canonicalize()
            .unwrap_or_else(|_| output_dir.to_path_buf());
        if let Ok(canonical_dest) = dest.canonicalize() {
            if !canonical_dest.starts_with(&canonical_output) {
                anyhow::bail!(
                    "Archive tries to write outside output directory: {}",
                    dest.display()
                );
            }
        }

        // Create parent directories if they don't exist
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        entry
            .unpack(&dest)
            .with_context(|| format!("Failed to extract: {}", path.display()))?;
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
    let response = client
        .head(url)
        .send()
        .await
        .context("Failed to check URL")?;
    Ok(response.status().is_success())
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

    let selected_objects: Vec<String> = match subset {
        Subset::Representative => REPRESENTATIVE_OBJECTS
            .iter()
            .map(|s| s.to_string())
            .collect(),
        Subset::Ten => TEN_OBJECTS.iter().map(|s| s.to_string()).collect(),
        Subset::All => fetch_objects(&client).await?,
    };

    fs::create_dir_all(output_dir).context("Failed to create output directory")?;

    let file_types = if options.full {
        vec!["berkeley_processed", "google_16k"]
    } else {
        vec!["google_16k"]
    };

    for object in &selected_objects {
        for file_type in &file_types {
            let url = get_tgz_url(object, file_type);

            if !url_exists(&client, &url).await? {
                continue;
            }

            let filename = format!("{}_{}.tgz", object, file_type);
            let dest_path = output_dir.join(&filename);

            if dest_path.exists() && !options.overwrite {
                continue;
            }

            download_file(&client, &url, &dest_path, options.show_progress).await?;
            extract_tgz(&dest_path, output_dir, options.delete_archives)?;
        }
    }

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
        Subset::Ten => Some(TEN_OBJECTS.iter().map(|s| s.to_string()).collect()),
        Subset::All => None,
    }
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
    }

    #[test]
    fn test_get_subset_objects_representative() {
        let objects = get_subset_objects(Subset::Representative);
        assert_eq!(objects.unwrap().len(), 3);
    }

    #[test]
    fn test_get_subset_objects_ten() {
        let objects = get_subset_objects(Subset::Ten);
        assert_eq!(objects.unwrap().len(), 10);
    }

    #[test]
    fn test_get_subset_objects_all() {
        let objects = get_subset_objects(Subset::All);
        assert!(objects.is_none());
    }
}
