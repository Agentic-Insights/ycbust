# YCB Downloader (ycbust)

A Rust CLI tool for efficiently downloading and extracting the YCB Object and Model Set. Designed for quick setup of 3D rendering and simulation environments (e.g., Bevy, Rapier).

## Features

- **Configurable Output**: Specify the download directory.
- **Smart Subsets**: Download a representative subset (3 objects), a larger set (10 objects), or the entire dataset.
- **Optimized Defaults**: By default, downloads only the `google_16k` meshes (high-quality, water-tight meshes best for rendering and physics).
- **Full Dataset Option**: Optional flag to download all auxiliary files (point clouds, poisson reconstructions, etc.).
- **Visual Feedback**: Clean progress bars with filename indication.
- **Auto-Extraction**: Automatically handles `.tgz` extraction and cleanup.

## Usage

### Prerequisites

- Rust and Cargo installed.

### Build

```bash
cargo build --release
```

### Running

**Download Representative Subset (Default)**
Downloads 3 common objects (Cracker Box, Sugar Box, Tomato Soup Can) to `/tmp/ycb`.
```bash
cargo run --release -- --subset representative
```

**Download to Custom Directory**
```bash
cargo run --release -- -o ./my_ycb_data
```

**Download Full Dataset (All File Types)**
Includes `berkeley_processed`, `google_16k`, etc.
```bash
cargo run --release -- --full
```

**Download 10 Objects**
```bash
cargo run --release -- --subset ten
```

**Download All Objects**
```bash
cargo run --release -- --subset all
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
