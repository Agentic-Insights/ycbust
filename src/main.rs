use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::Deserialize;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

const BASE_URL: &str = "http://ycb-benchmarks.s3-website-us-east-1.amazonaws.com/data/";
const OBJECTS_URL: &str = "https://ycb-benchmarks.s3.amazonaws.com/data/objects.json";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Output directory for downloaded files
    #[arg(short, long, default_value = "/tmp/ycb")]
    output_dir: PathBuf,

    /// Subset of objects to download
    #[arg(short, long, value_enum, default_value_t = Subset::Representative)]
    subset: Subset,

    /// Overwrite existing files
    #[arg(long, default_value_t = false)]
    overwrite: bool,

    /// Download all file types (including berkeley_processed, etc.)
    #[arg(long, default_value_t = false)]
    full: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum Subset {
    /// 3 representative objects
    Representative,
    /// 10 representative objects
    Ten,
    /// All objects
    All,
}

#[derive(Deserialize, Debug)]
struct ObjectsResponse {
    objects: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let client = Client::new();

    println!("Fetching object list...");
    let objects = fetch_objects(&client).await?;

    let selected_objects = match args.subset {
        Subset::Representative => vec!["003_cracker_box", "004_sugar_box", "005_tomato_soup_can"]
            .into_iter()
            .map(String::from)
            .collect(),
        Subset::Ten => vec![
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
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        Subset::All => objects,
    };

    println!(
        "Downloading {} objects to {:?}",
        selected_objects.len(),
        args.output_dir
    );
    fs::create_dir_all(&args.output_dir).context("Failed to create output directory")?;

    // Files to download for each object
    // Default: only google_16k (best for rendering/sim)
    // Full: include berkeley_processed (and others if added later)
    let file_types = if args.full {
        vec!["berkeley_processed", "google_16k"]
    } else {
        vec!["google_16k"]
    };

    for object in &selected_objects {
        for file_type in &file_types {
            let url = get_tgz_url(object, file_type);

            // Check if URL exists (HEAD request)
            let response = client.head(&url).send().await?;
            if !response.status().is_success() {
                println!("Skipping {} ({}): not found", object, file_type);
                continue;
            }

            let filename = format!("{}_{}.tgz", object, file_type);
            let dest_path = args.output_dir.join(&filename);

            if dest_path.exists() && !args.overwrite {
                println!("Skipping {} ({}): already exists", object, file_type);
            } else {
                download_file(&client, &url, &dest_path).await?;
                extract_tgz(&dest_path, &args.output_dir)?;
            }
        }
    }

    println!("\nDownload complete!");
    println!("Files are located in: {}", args.output_dir.display());
    if let Some(first_obj) = selected_objects.first() {
        println!("Example path for Bevy/rendering:");
        println!(
            "  {}/{}/google_16k/textured.obj",
            args.output_dir.display(),
            first_obj
        );
    }

    Ok(())
}

async fn fetch_objects(client: &Client) -> Result<Vec<String>> {
    let response = client.get(OBJECTS_URL).send().await?;
    let objects_response: ObjectsResponse = response.json().await?;
    Ok(objects_response.objects)
}

fn get_tgz_url(object: &str, file_type: &str) -> String {
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

async fn download_file(client: &Client, url: &str, dest_path: &Path) -> Result<()> {
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
    println!("Downloading {}", filename);

    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .expect("Invalid progress bar template - this is a bug")
            .progress_chars("#>-"),
    );

    let mut file = File::create(dest_path).context("Failed to create file")?;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.context("Error while downloading chunk")?;
        file.write_all(&chunk)
            .context("Error while writing to file")?;
        pb.inc(chunk.len() as u64);
    }

    pb.finish_with_message("Done");
    Ok(())
}

fn extract_tgz(tgz_path: &Path, output_dir: &Path) -> Result<()> {
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

        entry
            .unpack(&dest)
            .with_context(|| format!("Failed to extract: {}", path.display()))?;
    }

    fs::remove_file(tgz_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tgz_url_google_16k() {
        let url = get_tgz_url("003_cracker_box", "google_16k");
        assert_eq!(
            url,
            "http://ycb-benchmarks.s3-website-us-east-1.amazonaws.com/data/google/003_cracker_box_google_16k.tgz"
        );
    }

    #[test]
    fn test_get_tgz_url_berkeley_processed() {
        let url = get_tgz_url("003_cracker_box", "berkeley_processed");
        assert_eq!(
            url,
            "http://ycb-benchmarks.s3-website-us-east-1.amazonaws.com/data/berkeley/003_cracker_box/003_cracker_box_berkeley_meshes.tgz"
        );
    }

    #[test]
    fn test_get_tgz_url_berkeley_rgbd() {
        let url = get_tgz_url("003_cracker_box", "berkeley_rgbd");
        assert_eq!(
            url,
            "http://ycb-benchmarks.s3-website-us-east-1.amazonaws.com/data/berkeley/003_cracker_box/003_cracker_box_berkeley_rgbd.tgz"
        );
    }

    #[test]
    fn test_get_tgz_url_berkeley_rgb_highres() {
        let url = get_tgz_url("003_cracker_box", "berkeley_rgb_highres");
        assert_eq!(
            url,
            "http://ycb-benchmarks.s3-website-us-east-1.amazonaws.com/data/berkeley/003_cracker_box/003_cracker_box_berkeley_rgb_highres.tgz"
        );
    }

    #[test]
    fn test_get_tgz_url_different_objects() {
        // Test with different object IDs
        let url1 = get_tgz_url("004_sugar_box", "google_16k");
        assert!(url1.contains("004_sugar_box"));

        let url2 = get_tgz_url("005_tomato_soup_can", "google_16k");
        assert!(url2.contains("005_tomato_soup_can"));
    }
}
