# Installation

There are three ways to install ctddump. Pick the first one that applies:

| Route | Needs | Best for |
|-------|-------|----------|
| [Prebuilt binary](#prebuilt-binary) | nothing | most users |
| [crates.io](#install-from-cratesio) | Rust, HDF5 headers | Rust users, other platforms |
| [From source](#build-from-source) | Rust, HDF5 headers, git | contributors |

## Prebuilt binary

Each [release](https://github.com/AIQC-Hub/ctddump/releases) ships an archive per
platform. It contains the `ctddump` executable with HDF5 and netCDF built in, so
**no Rust toolchain and no system libraries are required**:

| Platform | Archive |
|----------|---------|
| Linux, Intel/AMD 64-bit | `ctddump-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` |
| Linux, ARM 64-bit | `ctddump-vX.Y.Z-aarch64-unknown-linux-gnu.tar.gz` |
| macOS, Apple Silicon | `ctddump-vX.Y.Z-aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `ctddump-vX.Y.Z-x86_64-apple-darwin.tar.gz` |

Download, extract, and put the binary somewhere on your `PATH`:

```bash
VERSION=v0.28.0
TARGET=x86_64-unknown-linux-gnu

curl -LO "https://github.com/AIQC-Hub/ctddump/releases/download/$VERSION/ctddump-$VERSION-$TARGET.tar.gz"
tar -xzf "ctddump-$VERSION-$TARGET.tar.gz"
cd "ctddump-$VERSION-$TARGET"

./ctddump --version
```

To install it for your user, move it onto your `PATH`:

```bash
mkdir -p ~/.local/bin
mv ctddump ~/.local/bin/
```

If `ctddump` is still not found afterwards, `~/.local/bin` is not on your `PATH`;
add `export PATH="$HOME/.local/bin:$PATH"` to your shell profile.

Every archive is listed in `SHA256SUMS` on the same release page. To verify a
download before trusting it:

```bash
curl -LO "https://github.com/AIQC-Hub/ctddump/releases/download/$VERSION/SHA256SUMS"
sha256sum -c SHA256SUMS --ignore-missing
```

### What else is in the archive

Alongside the binary are the [helper scripts](scripts.md), plus `README.md`,
`LICENSE`, and `CHANGELOG.md`. Five scripts (`convert_data.sh`, `clean_data.sh`,
`dedup_data.sh`, `compare_data.sh`, and `summary_data.sh`) need only `ctddump` on
your `PATH`, so they work as soon as the step above is done. Three others call
external tools that are not bundled and must be installed separately:
`download_data.sh` needs `copernicusmarine`, `summary_site.sh` needs `mdbook`,
and `fetch_test_data.sh` needs `gh` and `unzip`.

The scripts are bash, so on Windows they need WSL or Git Bash.

## System dependencies

The two routes below compile ctddump themselves and link the HDF5 C library, so
the development headers must be installed first. (The prebuilt binaries above
need none of this.)

```bash
# Ubuntu / Debian
sudo apt-get install libhdf5-dev libnetcdf-dev

# macOS (Homebrew)
brew install hdf5
```

## Install from crates.io

With the system dependencies in place, install the published crate:

```bash
cargo install ctddump
```

This builds the binary and puts it in `~/.cargo/bin`, which is normally already
on your `PATH`. Check it works:

```bash
ctddump --help
```

If the build stops with "A system version of libnetcdf could not be found", the
development headers are missing or are somewhere the build script does not look.
Install them as shown above, or point at them explicitly:

```bash
NETCDF_DIR=/path/to/netcdf HDF5_DIR=/path/to/hdf5 cargo install ctddump
```

Failing that, the `static-netcdf` feature builds HDF5 and netCDF from source
instead of looking for system ones. It is much slower but needs no headers:

```bash
cargo install ctddump --features static-netcdf
```

## Build from source

To work on ctddump itself, or to run a version that is not published yet:

```bash
git clone https://github.com/AIQC-Hub/ctddump.git
cd ctddump
cargo build --release
```

The binary is placed at `target/release/ctddump`. Copy it somewhere on your
`PATH` (or run it directly) and check it works:

```bash
ctddump --help
```

Every command and subcommand supports `-h` / `--help`, so you can always
discover the available options interactively:

```bash
ctddump convert --help
ctddump batch convert nrt_ar --help
```

> **Note:** On systems with HDF5 ≤ 1.10 you may see harmless `HDF5-DIAG`
> messages in the output. The data is read correctly and results are unaffected.
