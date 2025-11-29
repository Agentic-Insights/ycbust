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
        Subset::Representative => vec![
            "003_cracker_box",
            "004_sugar_box",
            "005_tomato_soup_can",
        ]
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

    println!("Downloading {} objects to {:?}", selected_objects.len(), args.output_dir);
    fs::create_dir_all(&args.output_dir).context("Failed to create output directory")?;

    // Files to download for each object
    // Based on reference: ["berkeley_processed", "google_16k"]
    let file_types = vec!["berkeley_processed", "google_16k"];

    for object in selected_objects {
        for file_type in &file_types {
            let url = get_tgz_url(&object, file_type);
            
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

    println!("Done!");
    Ok(())
}

async fn fetch_objects(client: &Client) -> Result<Vec<String>> {
    let response = client.get(OBJECTS_URL).send().await?;
    let objects_response: ObjectsResponse = response.json().await?;
    Ok(objects_response.objects)
}

fn get_tgz_url(object: &str, file_type: &str) -> String {
    if file_type == "berkeley_rgbd" || file_type == "berkeley_rgb_highres" {
        format!("{}berkeley/{}/{}_{}.tgz", BASE_URL, object, object, file_type)
    } else if file_type == "berkeley_processed" {
        format!("{}berkeley/{}/{}_berkeley_meshes.tgz", BASE_URL, object, object)
    } else {
        format!("{}google/{}_{}.tgz", BASE_URL, object, file_type)
    }
}

async fn download_file(client: &Client, url: &str, dest_path: &Path) -> Result<()> {
    let res = client.get(url).send().await.context("Failed to send request")?;
    let total_size = res.content_length().unwrap_or(0);

    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .unwrap()
        .progress_chars("#>-"));
    pb.set_message(format!("Downloading {}", dest_path.file_name().unwrap().to_string_lossy()));

    let mut file = File::create(dest_path).context("Failed to create file")?;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.context("Error while downloading chunk")?;
        file.write_all(&chunk).context("Error while writing to file")?;
        pb.inc(chunk.len() as u64);
    }

    pb.finish_with_message("Downloaded");
    Ok(())
}

fn extract_tgz(tgz_path: &Path, output_dir: &Path) -> Result<()> {
    let tar_gz = File::open(tgz_path)?;
    let tar = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(tar);
    archive.unpack(output_dir)?;
    fs::remove_file(tgz_path)?;
    Ok(())
}
