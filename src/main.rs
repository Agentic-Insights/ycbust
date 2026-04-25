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
use std::path::PathBuf;
use ycbust::{
    download_objects, download_ycb, fetch_objects, get_subset_objects, validate_objects,
    DownloadOptions, REPRESENTATIVE_OBJECTS, TBP_SIMILAR_OBJECTS, TBP_STANDARD_OBJECTS,
};

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

fn default_output_dir_path() -> PathBuf {
    std::env::temp_dir().join("ycb")
}

fn default_output_dir_string() -> String {
    default_output_dir_path().display().to_string()
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
    /// Download YCB objects to local disk
    Download {
        /// Output directory
        #[arg(short, long, default_value_t = default_output_dir_string())]
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

        /// Maximum concurrent per-object downloads (default 1)
        #[arg(long, default_value_t = 1)]
        concurrency: usize,
    },

    /// Validate that YCB objects are present and complete
    Validate {
        #[arg(short, long, default_value_os_t = default_output_dir_path())]
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
            concurrency,
        } => run_local_download(output_dir, subset, objects, overwrite, full, concurrency).await,

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
    concurrency: usize,
) -> Result<()> {
    let output_path = PathBuf::from(&output_dir);
    let mut options = DownloadOptions::default();
    options.overwrite = overwrite;
    options.full = full;
    options.concurrency = concurrency;

    if !objects.is_empty() {
        println!(
            "Downloading {} object(s) to {}",
            objects.len(),
            output_path.display()
        );
        let object_refs: Vec<&str> = objects.iter().map(String::as_str).collect();
        download_objects(&object_refs, &output_path, options)
            .await
            .context("Failed to download requested objects")?;
        println!("\n✅ Done → {}", output_path.display());
        return Ok(());
    }

    let subset = subset.unwrap_or(SubsetArg::TbpStandard);
    println!(
        "Downloading {} to {}...",
        subset_display_name(subset),
        output_path.display()
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
        "Validating {} ({} objects) in {}\n",
        subset_display_name(subset),
        objects.len(),
        output_dir.display()
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
