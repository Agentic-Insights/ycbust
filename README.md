# YCB Downloader (ycbust)

[![Crates.io](https://img.shields.io/crates/v/ycbust.svg)](https://crates.io/crates/ycbust)
[![GitHub release](https://img.shields.io/github/v/release/killerapp/ycbust)](https://github.com/killerapp/ycbust/releases)

A Rust CLI tool for efficiently downloading and extracting the YCB Object and Model Set. Designed for quick setup of 3D rendering and simulation environments (e.g., Bevy, Rapier).

## Installation

### From crates.io

```bash
cargo install ycbust
```

### From GitHub Releases

Pre-built binaries are available on the [Releases page](https://github.com/killerapp/ycbust/releases).

## Features

- **Configurable Output**: Specify the download directory.
- **Smart Subsets**: Download a representative subset (3 objects), a larger set (10 objects), or the entire dataset.
- **Optimized Defaults**: By default, downloads only the `google_16k` meshes (high-quality, water-tight meshes best for rendering and physics).
- **Full Dataset Option**: Optional flag to download all auxiliary files (point clouds, poisson reconstructions, etc.).
- **Visual Feedback**: Clean progress bars with filename indication.
- **Auto-Extraction**: Automatically handles `.tgz` extraction and cleanup.

## Usage

**Download Representative Subset (Default)**
Downloads 3 common objects (Cracker Box, Sugar Box, Tomato Soup Can) to `/tmp/ycb`.
```bash
ycbust --subset representative
```

**Download to Custom Directory**
```bash
ycbust -o ./my_ycb_data
```

**Download Full Dataset (All File Types)**
Includes `berkeley_processed`, `google_16k`, etc.
```bash
ycbust --full
```

**Download 10 Objects**
```bash
ycbust --subset ten
```

**Download All Objects**
```bash
ycbust --subset all
```

## CLI Options

```
Usage: ycbust [OPTIONS]

Options:
  -o, --output-dir <OUTPUT_DIR>  Output directory [default: /tmp/ycb]
  -s, --subset <SUBSET>          Subset to download [default: representative]
                                 (representative, ten, all)
      --overwrite                Overwrite existing files
      --full                     Download all file types (default: google_16k only)
  -h, --help                     Print help
```

## Output Structure

For each object, the tool creates a directory structure like this:

```
/output_dir/
  ├── 003_cracker_box/
  │   └── google_16k/
  │       ├── textured.obj      <-- Main mesh for rendering
  │       ├── texture_map.png   <-- Texture
  │       └── ...
  └── ...
```

## Integration with Bevy/Three-d

For rendering, point your asset loader to the `google_16k/textured.obj` file. It will automatically pick up the material and texture map if they are in the same folder.

## Development

This project uses `just` as a command runner.

```bash
# List all available commands
just

# Build the project
just build

# Run all tests
just test

# Download sample data (representative subset) to /tmp/ycb-test
just run-demo
```
