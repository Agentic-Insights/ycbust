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

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use reqwest::Client;
use std::fs;
use std::path::PathBuf;
use ycbust::{
    download_file, download_ycb, extract_tgz, fetch_objects, get_subset_objects, get_tgz_url,
    url_exists, validate_objects, DownloadOptions, REPRESENTATIVE_OBJECTS, TBP_SIMILAR_OBJECTS,
    TBP_STANDARD_OBJECTS,
};

#[cfg(feature = "s3")]
use ycbust::s3::{check_aws_credentials, download_ycb_to_s3, S3Destination};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum SubsetArg {
    /// 3 representative objects (quick test)
    Representative,
    /// TBP standard 10-object benchmark (distinct objects) [recommended]
    TbpStandard,
    /// TBP similar 10-object benchmark (harder discrimination)
    TbpSimilar,
    /// All ~77 objects (~1GB)
    All,
}

impl From<SubsetArg> for ycbust::Subset {
    fn from(s: SubsetArg) -> Self {
        match s {
            SubsetArg::Representative => ycbust::Subset::Representative,
            SubsetArg::TbpStandard => ycbust::Subset::TbpStandard,
            SubsetArg::TbpSimilar => ycbust::Subset::TbpSimilar,
            SubsetArg::All => ycbust::Subset::All,
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Download and manage YCB (Yale-CMU-Berkeley) 3D object models",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Download YCB objects to local disk or S3
    Download {
        /// Output directory (or S3 URL with --features s3)
        #[arg(short, long, default_value = "/tmp/ycb")]
        output_dir: String,

        /// Preset subset of objects to download
        #[arg(short, long, value_enum)]
        subset: Option<SubsetArg>,

        /// Download specific objects by name (e.g. --objects 006_mustard_bottle 011_banana)
        #[arg(long, num_args = 1..)]
        objects: Vec<String>,

        /// Overwrite existing files
        #[arg(long, default_value_t = false)]
        overwrite: bool,

        /// Download all file types (includes berkeley_processed)
        #[arg(long, default_value_t = false)]
        full: bool,

        #[cfg(feature = "s3")]
        #[arg(long)]
        profile: Option<String>,

        #[cfg(feature = "s3")]
        #[arg(long, default_value = "us-east-1")]
        region: String,
    },

    /// Validate that YCB objects are present and complete
    Validate {
        #[arg(short, long, default_value = "/tmp/ycb")]
        output_dir: PathBuf,

        #[arg(short, long, value_enum, default_value_t = SubsetArg::TbpStandard)]
        subset: SubsetArg,
    },

    /// List objects in a subset
    List {
        #[arg(short, long, value_enum, default_value_t = SubsetArg::TbpStandard)]
        subset: SubsetArg,

        /// Fetch full list from YCB S3 (only needed for --subset all)
        #[arg(long, default_value_t = false)]
        fetch: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Download {
            output_dir,
            subset,
            objects,
            overwrite,
            full,
            #[cfg(feature = "s3")]
            profile,
            #[cfg(feature = "s3")]
            region,
        } => {
            if output_dir.starts_with("s3://") {
                #[cfg(feature = "s3")]
                {
                    return run_s3_download(output_dir, subset, overwrite, full, profile, region)
                        .await;
                }
                #[cfg(not(feature = "s3"))]
                anyhow::bail!(
                    "S3 destination requires the 's3' feature.\n\
                     Rebuild with: cargo install ycbust --features s3"
                );
            }
            run_local_download(output_dir, subset, objects, overwrite, full).await
        }

        Commands::Validate { output_dir, subset } => run_validate(output_dir, subset),

        Commands::List { subset, fetch } => run_list(subset, fetch).await,
    }
}

async fn run_local_download(
    output_dir: String,
    subset: Option<SubsetArg>,
    objects: Vec<String>,
    overwrite: bool,
    full: bool,
) -> Result<()> {
    let output_path = PathBuf::from(&output_dir);
    let options = DownloadOptions {
        overwrite,
        full,
        show_progress: true,
        delete_archives: true,
    };

    if !objects.is_empty() {
        let client = Client::new();
        fs::create_dir_all(&output_path).context("Failed to create output directory")?;
        let file_types = if full {
            vec!["berkeley_processed", "google_16k"]
        } else {
            vec!["google_16k"]
        };

        println!(
            "Downloading {} object(s) to {:?}",
            objects.len(),
            output_path
        );
        for object in &objects {
            for file_type in &file_types {
                let url = get_tgz_url(object, file_type);
                if !url_exists(&client, &url).await? {
                    println!("  ⚠  {} ({}) not found, skipping", object, file_type);
                    continue;
                }
                let dest_path = output_path.join(format!("{}_{}.tgz", object, file_type));
                if dest_path.exists() && !overwrite {
                    println!("  ✓  {} already present", object);
                    continue;
                }
                download_file(&client, &url, &dest_path, true).await?;
                extract_tgz(&dest_path, &output_path, true)?;
                println!("  ✓  {}", object);
            }
        }
        println!("\n✅ Done → {}", output_path.display());
        return Ok(());
    }

    let subset = subset.unwrap_or(SubsetArg::TbpStandard);
    println!(
        "Downloading {} to {:?}...",
        subset_display_name(subset),
        output_path
    );
    download_ycb(subset.into(), &output_path, options).await?;
    println!("\n✅ Done → {}", output_path.display());
    println!(
        "   Run 'ycbust validate --subset {}' to verify",
        subset_cli_name(subset)
    );
    Ok(())
}

fn run_validate(output_dir: PathBuf, subset: SubsetArg) -> Result<()> {
    let objects: &[&str] = match subset {
        SubsetArg::TbpStandard => TBP_STANDARD_OBJECTS,
        SubsetArg::TbpSimilar => TBP_SIMILAR_OBJECTS,
        SubsetArg::Representative => REPRESENTATIVE_OBJECTS,
        SubsetArg::All => anyhow::bail!(
            "Cannot validate 'all' without a full object list.\n\
             Use --subset tbp-standard or tbp-similar."
        ),
    };

    println!(
        "Validating {} ({} objects) in {:?}\n",
        subset_display_name(subset),
        objects.len(),
        output_dir
    );

    let results = validate_objects(&output_dir, objects);
    let mut present = 0;
    let mut missing_names = Vec::new();

    for r in &results {
        if r.is_complete() {
            println!("  ✓  {}", r.name);
            present += 1;
        } else if r.mesh_present {
            println!("  ⚠  {} (mesh ✓, texture missing)", r.name);
            missing_names.push(r.name.clone());
        } else {
            println!("  ✗  {} ← MISSING", r.name);
            missing_names.push(r.name.clone());
        }
    }

    println!("\n{}/{} objects present.", present, objects.len());

    if !missing_names.is_empty() {
        println!("\nTo download missing objects:");
        println!(
            "  ycbust download --output-dir {} --objects {}",
            output_dir.display(),
            missing_names.join(" ")
        );
    } else {
        println!("✅ All objects verified.");
    }

    Ok(())
}

async fn run_list(subset: SubsetArg, fetch: bool) -> Result<()> {
    if fetch || matches!(subset, SubsetArg::All) {
        println!("Fetching full object list from YCB S3...");
        let client = Client::new();
        let objects = fetch_objects(&client).await?;
        println!("Available objects ({}):", objects.len());
        for obj in &objects {
            println!("  {}", obj);
        }
        return Ok(());
    }

    match get_subset_objects(subset.into()) {
        Some(list) => {
            println!("{} ({} objects):", subset_display_name(subset), list.len());
            for obj in &list {
                println!("  {}", obj);
            }
        }
        None => println!("Use --fetch to retrieve the full object list from YCB S3."),
    }
    Ok(())
}

fn subset_display_name(subset: SubsetArg) -> &'static str {
    match subset {
        SubsetArg::Representative => "representative (3 objects)",
        SubsetArg::TbpStandard => "TBP standard 10-object benchmark",
        SubsetArg::TbpSimilar => "TBP similar 10-object benchmark",
        SubsetArg::All => "full dataset (~77 objects)",
    }
}

fn subset_cli_name(subset: SubsetArg) -> &'static str {
    match subset {
        SubsetArg::Representative => "representative",
        SubsetArg::TbpStandard => "tbp-standard",
        SubsetArg::TbpSimilar => "tbp-similar",
        SubsetArg::All => "all",
    }
}

#[cfg(feature = "s3")]
async fn run_s3_download(
    output_url: String,
    subset: Option<SubsetArg>,
    overwrite: bool,
    full: bool,
    profile: Option<String>,
    region: String,
) -> Result<()> {
    let mut dest = S3Destination::from_url(&output_url)?;
    dest = dest.with_region(&region);
    println!("Checking AWS credentials...");
    let identity = check_aws_credentials(profile.as_deref()).await?;
    println!("{}", identity);
    let options = DownloadOptions {
        overwrite,
        full,
        show_progress: true,
        delete_archives: true,
    };
    let ycb_subset: ycbust::Subset = subset.unwrap_or(SubsetArg::TbpStandard).into();
    println!("\nStreaming to S3...");
    let stats = download_ycb_to_s3(ycb_subset, dest.clone(), options, profile.as_deref()).await?;
    println!("\n✅ Done → {}", dest.to_url());
    println!(
        "   Uploaded: {} files ({:.1} MB)",
        stats.files_uploaded,
        stats.bytes_uploaded as f64 / 1_048_576.0
    );
    println!("   Skipped:  {} files", stats.files_skipped);
    Ok(())
}
