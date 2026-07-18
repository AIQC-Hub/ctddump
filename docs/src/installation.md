# Installation

## System dependencies

The `netcdf` crate links against the HDF5 C library, so the development headers
must be installed first:

```bash
# Ubuntu / Debian
sudo apt-get install libhdf5-dev libnetcdf-dev

# macOS (Homebrew)
brew install hdf5
```

## Install from crates.io

With the system dependencies above in place, install the published crate:

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
