# `batch`

Process a **whole directory tree** in parallel. `batch` recursively finds every
matching `.nc` file and converts each one, using all CPU cores by default.

```
ctddump batch convert <subcommand> [OPTIONS] <src_dir>
ctddump batch header  <subcommand> [OPTIONS] <src_dir>
```

- `batch convert` writes one **Parquet** file per source file.
- `batch header` writes one **YAML** metadata file per source file.

Each output keeps its source's filename stem with a new extension. If
`--output` is omitted, each result is written alongside its source; with
`--output <dir>` all results land flat in that directory.

## Examples

```bash
# Convert all NRT Arctic files to Parquet, flat into ./output
ctddump batch convert nrt_ar --output ./output ./data/arctic

# Limit to 4 threads
ctddump batch convert nrt_ar --threads 4 --output ./output ./data/arctic

# Override the filename pattern
ctddump batch convert nrt_ar --pattern "AR_PR_CT_ITP-*.nc" --output ./output ./data

# Extract YAML metadata for all NRT files
ctddump batch header nrt --output ./output ./data/arctic
```

## Default filename patterns

`--pattern` matches the **filename only** (not the path) and supports `*`, `?`
and `[…]`. When omitted, each subcommand uses a sensible default:

| Subcommand | Pattern |
|------------|---------|
| `nrt_ar` | `AR_PR_CT_*.nc` |
| `nrt_bo` | `BO_PR_CT_*.nc` |
| `nrt_mo` | `MO_PR_CT_*.nc` |
| `nrt_gl` | `GL_PR_CT_*.nc` |
| `cora`, `cora_legacy` | `*.nc` |
| `batch header nrt`, `batch header cora` | `*.nc` |

> **Threads.** `--threads N` caps the worker count; omit it to use all logical
> cores. Memory scales with the thread count, so lower it if you are processing
> very large files on a memory-constrained machine.
