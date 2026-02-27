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

//! S3 destination support for streaming YCB objects directly to cloud storage.
//!
//! This module enables downloading YCB objects and streaming them directly to an S3 bucket
//! without requiring local disk storage. It uses AWS credentials from the environment
//! or AWS configuration files.
//!
//! # Example
//!
//! ```no_run
//! use ycbust::s3::{S3Destination, download_ycb_to_s3};
//! use ycbust::{Subset, DownloadOptions};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let dest = S3Destination::from_url("s3://my-bucket/ycb-data/")?;
//!     download_ycb_to_s3(
//!         Subset::Representative,
//!         dest,
//!         DownloadOptions::default(),
//!         None, // Use default AWS profile
//!     ).await?;
//!     Ok(())
//! }
//! ```

use anyhow::{anyhow, bail, Context, Result};
use async_compression::futures::bufread::GzipDecoder;
use async_tar::Archive;
use futures_util::{AsyncReadExt, StreamExt, TryStreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use s3::creds::Credentials;
use s3::error::S3Error;
use s3::region::Region;
use s3::Bucket;

use crate::{fetch_objects, get_tgz_url, url_exists, DownloadOptions, Subset};
use crate::{REPRESENTATIVE_OBJECTS, TEN_OBJECTS};

/// Represents an S3 destination for uploading YCB objects.
#[derive(Clone, Debug)]
pub struct S3Destination {
    /// The S3 bucket name
    pub bucket: String,
    /// The prefix (path) within the bucket
    pub prefix: String,
    /// AWS region (defaults to us-east-1)
    pub region: String,
}

impl S3Destination {
    /// Parse an S3 URL into bucket and prefix components.
    ///
    /// # Supported formats
    /// - `s3://bucket-name/prefix/path/`
    /// - `s3://bucket-name/prefix/path` (trailing slash optional)
    /// - `s3://bucket-name/` (root of bucket)
    /// - `s3://bucket-name` (root of bucket)
    ///
    /// # Example
    ///
    /// ```
    /// use ycbust::s3::S3Destination;
    ///
    /// let dest = S3Destination::from_url("s3://my-bucket/ycb-data/").unwrap();
    /// assert_eq!(dest.bucket, "my-bucket");
    /// assert_eq!(dest.prefix, "ycb-data/");
    /// ```
    pub fn from_url(url: &str) -> Result<Self> {
        let url = url.trim();

        if !url.starts_with("s3://") {
            bail!("S3 URL must start with 's3://', got: {}", url);
        }

        let path = &url[5..]; // Remove "s3://"

        if path.is_empty() {
            bail!("S3 URL must include a bucket name");
        }

        let (bucket, prefix) = match path.find('/') {
            Some(idx) => {
                let bucket = &path[..idx];
                let mut prefix = path[idx + 1..].to_string();
                // Ensure prefix ends with / if not empty (for proper path joining)
                if !prefix.is_empty() && !prefix.ends_with('/') {
                    prefix.push('/');
                }
                (bucket.to_string(), prefix)
            }
            None => (path.to_string(), String::new()),
        };

        if bucket.is_empty() {
            bail!("S3 URL must include a bucket name");
        }

        Ok(Self {
            bucket,
            prefix,
            region: "us-east-1".to_string(),
        })
    }

    /// Set the AWS region for this destination.
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = region.into();
        self
    }

    /// Get the full S3 path for a given object path.
    pub fn full_path(&self, path: &str) -> String {
        format!("{}{}", self.prefix, path)
    }

    /// Get the S3 URL representation of this destination.
    pub fn to_url(&self) -> String {
        format!("s3://{}/{}", self.bucket, self.prefix)
    }
}

/// Statistics from an S3 upload operation.
#[derive(Clone, Debug, Default)]
pub struct S3UploadStats {
    /// Number of files uploaded
    pub files_uploaded: usize,
    /// Number of files skipped (already exist)
    pub files_skipped: usize,
    /// Total bytes uploaded
    pub bytes_uploaded: u64,
}

/// Check if AWS credentials are available and valid.
///
/// # Arguments
///
/// * `profile` - Optional AWS profile name. If None, uses default credential chain.
///
/// # Returns
///
/// Returns the AWS identity info if credentials are valid.
pub async fn check_aws_credentials(profile: Option<&str>) -> Result<String> {
    let creds = get_credentials(profile)?;

    // Verify both access key and secret key are present
    let access_key = creds
        .access_key
        .as_ref()
        .ok_or_else(|| anyhow!("No AWS access key found"))?;

    let secret_key = creds
        .secret_key
        .as_ref()
        .ok_or_else(|| anyhow!("No AWS secret key found"))?;

    if access_key.is_empty() {
        bail!("AWS access key is empty");
    }

    if secret_key.is_empty() {
        bail!("AWS secret key is empty");
    }

    // Return a masked version of the access key as identity
    let masked = if access_key.len() > 8 {
        format!(
            "{}...{}",
            &access_key[..4],
            &access_key[access_key.len() - 4..]
        )
    } else {
        "****".to_string()
    };

    Ok(format!("AWS credentials loaded (access key: {})", masked))
}

/// Get AWS credentials from environment or config files.
fn get_credentials(profile: Option<&str>) -> Result<Credentials> {
    // First try environment variables
    if std::env::var("AWS_ACCESS_KEY_ID").is_ok() {
        return Credentials::from_env()
            .map_err(|e| anyhow!("Failed to load AWS credentials from environment: {}", e));
    }

    // Then try profile from config file
    let profile_name = profile
        .map(|s| s.to_string())
        .or_else(|| std::env::var("AWS_PROFILE").ok())
        .unwrap_or_else(|| "default".to_string());

    Credentials::from_profile(Some(&profile_name)).map_err(|e| {
        anyhow!(
            "Failed to load AWS credentials for profile '{}': {}",
            profile_name,
            e
        )
    })
}

/// Create an S3 bucket handle for operations.
async fn create_bucket(dest: &S3Destination, profile: Option<&str>) -> Result<Box<Bucket>> {
    let creds = get_credentials(profile)?;
    let region = Region::Custom {
        region: dest.region.clone(),
        endpoint: format!("https://s3.{}.amazonaws.com", dest.region),
    };

    let bucket = Bucket::new(&dest.bucket, region, creds)
        .map_err(|e| anyhow!("Failed to create S3 bucket handle: {}", e))?
        .with_path_style();

    Ok(bucket)
}

/// Check if an S3 object already exists.
async fn object_exists(bucket: &Bucket, path: &str) -> Result<bool> {
    match bucket.head_object(path).await {
        Ok(_) => Ok(true),
        Err(S3Error::HttpFailWithBody(404, _)) => Ok(false),
        Err(e) => {
            // Check if it's a 404 by inspecting the error message
            let err_str = e.to_string();
            if err_str.contains("404")
                || err_str.contains("Not Found")
                || err_str.contains("NoSuchKey")
            {
                Ok(false)
            } else {
                Err(anyhow!("Failed to check if object exists: {}", e))
            }
        }
    }
}

/// Download YCB objects and stream them directly to S3.
///
/// This function downloads from the YCB dataset and streams the extracted files
/// directly to an S3 bucket without requiring local disk storage.
///
/// # Arguments
///
/// * `subset` - Which subset of objects to download
/// * `dest` - S3 destination (bucket and prefix)
/// * `options` - Download options
/// * `profile` - Optional AWS profile name
///
/// # Example
///
/// ```no_run
/// use ycbust::s3::{S3Destination, download_ycb_to_s3};
/// use ycbust::{Subset, DownloadOptions};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let dest = S3Destination::from_url("s3://my-bucket/ycb-data/")?;
///     download_ycb_to_s3(
///         Subset::Representative,
///         dest,
///         DownloadOptions::default(),
///         None,
///     ).await?;
///     Ok(())
/// }
/// ```
pub async fn download_ycb_to_s3(
    subset: Subset,
    dest: S3Destination,
    options: DownloadOptions,
    profile: Option<&str>,
) -> Result<S3UploadStats> {
    let http_client = Client::new();
    let s3_bucket = create_bucket(&dest, profile).await?;

    let mut stats = S3UploadStats::default();

    // Get list of objects to download
    let selected_objects: Vec<String> = match subset {
        Subset::Representative => REPRESENTATIVE_OBJECTS
            .iter()
            .map(|s| s.to_string())
            .collect(),
        Subset::Ten => TEN_OBJECTS.iter().map(|s| s.to_string()).collect(),
        Subset::All => fetch_objects(&http_client).await?,
    };

    let file_types = if options.full {
        vec!["berkeley_processed", "google_16k"]
    } else {
        vec!["google_16k"]
    };

    println!(
        "Streaming {} objects to {}",
        selected_objects.len(),
        dest.to_url()
    );

    for object in &selected_objects {
        for file_type in &file_types {
            let url = get_tgz_url(object, file_type);

            // Check if URL exists
            if !url_exists(&http_client, &url).await? {
                if options.show_progress {
                    println!("Skipping {} ({}): not found on source", object, file_type);
                }
                continue;
            }

            // Stream and extract the tarball to S3
            let result = stream_tgz_to_s3(
                &http_client,
                &url,
                &s3_bucket,
                &dest.prefix,
                object,
                file_type,
                &options,
            )
            .await;

            match result {
                Ok((uploaded, skipped, bytes)) => {
                    stats.files_uploaded += uploaded;
                    stats.files_skipped += skipped;
                    stats.bytes_uploaded += bytes;
                }
                Err(e) => {
                    eprintln!("Error processing {} ({}): {}", object, file_type, e);
                }
            }
        }
    }

    Ok(stats)
}

/// Validate and sanitize a path from a tar archive entry.
/// Returns None if the path is invalid or attempts path traversal.
fn sanitize_tar_path(path: &std::path::Path) -> Option<String> {
    // Reject paths with parent directory components (..)
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return None;
    }

    // Reject absolute paths
    if path.is_absolute() {
        return None;
    }

    // Convert to string and normalize separators to forward slashes for S3
    let path_str = path.to_string_lossy();
    let normalized = path_str.replace('\\', "/");

    // Reject empty paths or paths that start with /
    if normalized.is_empty() || normalized.starts_with('/') {
        return None;
    }

    Some(normalized)
}

/// Stream a .tgz file from HTTP directly to S3, extracting as we go.
async fn stream_tgz_to_s3(
    client: &Client,
    url: &str,
    bucket: &Bucket,
    prefix: &str,
    object: &str,
    file_type: &str,
    options: &DownloadOptions,
) -> Result<(usize, usize, u64)> {
    let mut uploaded = 0usize;
    let mut skipped = 0usize;
    let mut bytes = 0u64;

    // Start HTTP request
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to start download")?;

    if !response.status().is_success() {
        bail!("HTTP request failed with status: {}", response.status());
    }

    // Create progress bar (tracks files processed, not bytes - since we're streaming)
    let pb = if options.show_progress {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} [{elapsed_precise}] {msg}")
                .expect("Invalid progress bar template"),
        );
        pb.set_message(format!("{} ({}) - extracting...", object, file_type));
        Some(pb)
    } else {
        None
    };

    // Convert response body to a stream with futures-compatible error type
    let byte_stream = response.bytes_stream().map_err(std::io::Error::other);

    // Create an async reader from the stream (futures-based)
    let stream_reader = byte_stream.into_async_read();

    // Wrap in a buffered reader for GzipDecoder
    let buf_reader = futures_util::io::BufReader::new(stream_reader);

    // Decompress gzip (uses futures::AsyncBufRead)
    let decoder = GzipDecoder::new(buf_reader);

    // Read tar archive (async-tar expects futures::AsyncRead)
    let archive = Archive::new(decoder);
    let mut entries = archive.entries().context("Failed to read tar entries")?;

    while let Some(entry_result) = entries.next().await {
        let mut entry = entry_result.context("Failed to read tar entry")?;
        let path = entry
            .path()
            .context("Failed to get entry path")?
            .to_path_buf();

        // Skip directories
        if entry.header().entry_type().is_dir() {
            continue;
        }

        // Sanitize and validate the path (security: prevent path traversal)
        // Convert to std::path::Path for validation
        let std_path = std::path::Path::new(path.as_os_str());
        let sanitized_path = match sanitize_tar_path(std_path) {
            Some(p) => p,
            None => {
                eprintln!(
                    "Warning: Skipping invalid/unsafe path in archive: {}",
                    path.display()
                );
                continue;
            }
        };

        // Build S3 path
        let s3_path = format!("{}{}", prefix, sanitized_path);

        // Check if object already exists (unless overwrite is enabled)
        // Properly propagate errors instead of swallowing them
        if !options.overwrite {
            match object_exists(bucket, &s3_path).await {
                Ok(true) => {
                    skipped += 1;
                    continue;
                }
                Ok(false) => {} // Object doesn't exist, proceed with upload
                Err(e) => {
                    // Log the error but continue - don't fail the whole operation
                    eprintln!("Warning: Failed to check if {} exists: {}", s3_path, e);
                }
            }
        }

        // Read entry content into memory (tar entries must be read sequentially)
        // Note: This is necessary because tar entries are sequential and we can't
        // seek back. For very large files, this could use significant memory.
        let mut content = Vec::new();
        entry
            .read_to_end(&mut content)
            .await
            .context("Failed to read tar entry content")?;
        let content_len = content.len() as u64;

        // Determine content type
        let content_type = guess_content_type(&sanitized_path);

        // Upload to S3 with content-type header
        bucket
            .put_object_with_content_type(&s3_path, &content, content_type)
            .await
            .map_err(|e| anyhow!("Failed to upload {}: {}", s3_path, e))?;

        uploaded += 1;
        bytes += content_len;

        if let Some(ref pb) = pb {
            pb.set_message(format!(
                "{} ({}) - {} files uploaded",
                object, file_type, uploaded
            ));
        }
    }

    if let Some(pb) = pb {
        pb.finish_with_message(format!("{} ({}) - {} files", object, file_type, uploaded));
    }

    Ok((uploaded, skipped, bytes))
}

/// Guess MIME content type from file extension.
fn guess_content_type(path: &str) -> &'static str {
    if path.ends_with(".obj") {
        "model/obj"
    } else if path.ends_with(".mtl") {
        "model/mtl"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".ply") {
        "application/ply"
    } else if path.ends_with(".json") {
        "application/json"
    } else if path.ends_with(".txt") {
        "text/plain"
    } else {
        "application/octet-stream"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s3_destination_from_url_basic() {
        let dest = S3Destination::from_url("s3://my-bucket/prefix/path/").unwrap();
        assert_eq!(dest.bucket, "my-bucket");
        assert_eq!(dest.prefix, "prefix/path/");
    }

    #[test]
    fn test_s3_destination_from_url_no_trailing_slash() {
        let dest = S3Destination::from_url("s3://my-bucket/prefix/path").unwrap();
        assert_eq!(dest.bucket, "my-bucket");
        assert_eq!(dest.prefix, "prefix/path/");
    }

    #[test]
    fn test_s3_destination_from_url_bucket_only() {
        let dest = S3Destination::from_url("s3://my-bucket").unwrap();
        assert_eq!(dest.bucket, "my-bucket");
        assert_eq!(dest.prefix, "");
    }

    #[test]
    fn test_s3_destination_from_url_bucket_with_slash() {
        let dest = S3Destination::from_url("s3://my-bucket/").unwrap();
        assert_eq!(dest.bucket, "my-bucket");
        assert_eq!(dest.prefix, "");
    }

    #[test]
    fn test_s3_destination_from_url_invalid() {
        assert!(S3Destination::from_url("http://example.com").is_err());
        assert!(S3Destination::from_url("s3://").is_err());
        assert!(S3Destination::from_url("/local/path").is_err());
    }

    #[test]
    fn test_s3_destination_full_path() {
        let dest = S3Destination::from_url("s3://my-bucket/ycb/").unwrap();
        assert_eq!(
            dest.full_path("003_cracker_box/google_16k/textured.obj"),
            "ycb/003_cracker_box/google_16k/textured.obj"
        );
    }

    #[test]
    fn test_s3_destination_to_url() {
        let dest = S3Destination::from_url("s3://my-bucket/prefix/").unwrap();
        assert_eq!(dest.to_url(), "s3://my-bucket/prefix/");
    }

    #[test]
    fn test_guess_content_type() {
        assert_eq!(guess_content_type("model.obj"), "model/obj");
        assert_eq!(guess_content_type("texture.png"), "image/png");
        assert_eq!(guess_content_type("data.json"), "application/json");
        assert_eq!(
            guess_content_type("unknown.xyz"),
            "application/octet-stream"
        );
    }

    #[test]
    fn test_sanitize_tar_path_valid() {
        use std::path::Path;
        assert_eq!(
            sanitize_tar_path(Path::new("foo/bar/file.txt")),
            Some("foo/bar/file.txt".to_string())
        );
        assert_eq!(
            sanitize_tar_path(Path::new("file.obj")),
            Some("file.obj".to_string())
        );
    }

    #[test]
    fn test_sanitize_tar_path_traversal() {
        use std::path::Path;
        // Parent directory traversal should be rejected
        assert_eq!(sanitize_tar_path(Path::new("../etc/passwd")), None);
        assert_eq!(sanitize_tar_path(Path::new("foo/../bar")), None);
        assert_eq!(sanitize_tar_path(Path::new("foo/bar/../../baz")), None);
    }

    #[test]
    fn test_sanitize_tar_path_absolute() {
        use std::path::Path;
        // Absolute paths should be rejected
        assert_eq!(sanitize_tar_path(Path::new("/etc/passwd")), None);
    }

    #[test]
    fn test_sanitize_tar_path_empty() {
        use std::path::Path;
        assert_eq!(sanitize_tar_path(Path::new("")), None);
    }

    #[cfg(windows)]
    #[test]
    fn test_sanitize_tar_path_windows_separators() {
        use std::path::Path;
        // Windows paths should have separators normalized
        let result = sanitize_tar_path(Path::new("foo\\bar\\file.txt"));
        assert_eq!(result, Some("foo/bar/file.txt".to_string()));
    }
}
