# `convert`

Convert a **single** NetCDF file into a Parquet file.

```
ctddump convert <subcommand> [OPTIONS] <src_file> <target_file>
```

The subcommand selects the source format and its defaults:

| Subcommand | Source |
|------------|--------|
| `nrt_ar` | NRT — Arctic Sea |
| `nrt_bo` | NRT — Baltic Sea |
| `nrt_mo` | NRT — Mediterranean Sea |
| `nrt_gl` | NRT — Global |
| `cora` | CORA — current format |
| `cora_legacy` | CORA — legacy format |

## Examples

```bash
ctddump convert nrt_ar      input.nc output.parquet
ctddump convert nrt_bo      input.nc output.parquet
ctddump convert cora        input.nc output.parquet
ctddump convert cora_legacy input.nc output.parquet
```

Use a saved TOML preset, or override individual fields with flags:

```bash
# Apply a config preset
ctddump convert nrt_ar --config my_preset.toml input.nc output.parquet

# Override a single field
ctddump convert nrt_bo --no-deph-source input.nc output.parquet
```

See [Configuration](../configuration.md) for the available flags and the TOML
format, and [Output schema](../output-schema.md) for the columns produced.
