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

## Build from source

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
