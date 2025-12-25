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

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use reqwest::Client;
use std::fs;
use std::path::PathBuf;
use ycbust::{
    download_file, extract_tgz, fetch_objects, get_tgz_url, url_exists, DownloadOptions,
    REPRESENTATIVE_OBJECTS, TEN_OBJECTS,
};

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

impl From<Subset> for ycbust::Subset {
    fn from(s: Subset) -> Self {
        match s {
            Subset::Representative => ycbust::Subset::Representative,
            Subset::Ten => ycbust::Subset::Ten,
            Subset::All => ycbust::Subset::All,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let client = Client::new();

    println!("Fetching object list...");
    let objects = fetch_objects(&client).await?;

    let selected_objects: Vec<String> = match args.subset {
        Subset::Representative => REPRESENTATIVE_OBJECTS
            .iter()
            .map(|s| s.to_string())
            .collect(),
        Subset::Ten => TEN_OBJECTS.iter().map(|s| s.to_string()).collect(),
        Subset::All => objects,
    };

    println!(
        "Downloading {} objects to {:?}",
        selected_objects.len(),
        args.output_dir
    );
    fs::create_dir_all(&args.output_dir).context("Failed to create output directory")?;

    let options = DownloadOptions {
        overwrite: args.overwrite,
        full: args.full,
        show_progress: true,
        delete_archives: true,
    };

    // Files to download for each object
    let file_types = if options.full {
        vec!["berkeley_processed", "google_16k"]
    } else {
        vec!["google_16k"]
    };

    for object in &selected_objects {
        for file_type in &file_types {
            let url = get_tgz_url(object, file_type);

            // Check if URL exists (HEAD request)
            if !url_exists(&client, &url).await? {
                println!("Skipping {} ({}): not found", object, file_type);
                continue;
            }

            let filename = format!("{}_{}.tgz", object, file_type);
            let dest_path = args.output_dir.join(&filename);

            if dest_path.exists() && !options.overwrite {
                println!("Skipping {} ({}): already exists", object, file_type);
            } else {
                download_file(&client, &url, &dest_path, options.show_progress).await?;
                extract_tgz(&dest_path, &args.output_dir, options.delete_archives)?;
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
