# Configuration

All `convert` and `batch convert` subcommands accept a `--config` TOML file plus
individual flag overrides. The priority order is:

```
built-in default  <  --config file  <  individual CLI flags
```

`--config` may set any subset of fields; a per-field flag then overrides the
config for that field only.

## NRT flags

| Flag | Field | Default |
|------|-------|---------|
| `--deph-source` / `--no-deph-source` | `has_deph_source` | `true` for BO/GL, `false` for AR/MO |
| `--profile-coords` / `--no-profile-coords` | `has_profile_coords` | `true` for BO, `false` otherwise |
| `--pattern <GLOB>` | `pattern` | see the [batch patterns](./commands/batch.md#default-filename-patterns) |

> **DEPH is auto-detected.** NRT always performs the bidirectional PRES↔DEPH
> conversion when the file contains a `DEPH` variable, regardless of
> `has_deph_source`. The flag only forces DEPH handling on for files where the
> variable might otherwise be skipped.

## CORA flags

| Flag | Field | `cora` default | `cora_legacy` default |
|------|-------|----------------|-----------------------|
| `--time-var <VAR>` | `time_var` | `TIME` | `JULD` |
| `--qc-type <int\|char>` | `qc_type` | `int` | `char` |
| `--time-qc` / `--no-time-qc` | `has_time_qc` | `true` | `false` |
| `--deph-source` / `--no-deph-source` | `has_deph_source` | `true` | `false` |
| `--pattern <GLOB>` | `pattern` | `*.nc` | `*.nc` |

## TOML config format

```toml
# NRT
has_deph_source    = true
has_profile_coords = false
pattern            = "AR_PR_CT_*.nc"  # optional

# CORA
time_var        = "TIME"
qc_type         = "int"   # "int" or "char"
has_time_qc     = true
has_deph_source = true
pattern         = "*.nc"  # optional
```

## Environment variables

`ctddump` honours a few environment variables for operational tuning. They do
**not** change the *contents* of the output — only memory use, speed, and the
on-disk row-group layout — which is why they live in the environment rather than
as per-command flags.

| Variable | Effect | When unset |
|----------|--------|------------|
| `CTDDUMP_CHUNK_ROWS` | Observation rows assembled per streamed chunk (each written as one Parquet row group). Lower = less memory but more row groups; higher = more memory but fewer. Applies to `convert`, `batch`, `concat`, and the streaming filters (`filter` / `dropqc` / `dropna` / `markdup` / `dedup`). | `1000000` |
| `POLARS_MAX_THREADS` | Size of Polars' internal thread pool. `batch` / `concat` pin this to `1` so their own `--threads` is the real knob, but a value you set is respected. | pinned to `1` by `batch` / `concat` |
| `RUST_MIN_STACK` | Worker-thread stack size, in bytes. Raised to 16 MiB for the deep call chains in Polars' Parquet writer; a larger value you set is respected. | raised to 16 MiB |

`CTDDUMP_CHUNK_ROWS` is the only one most users would ever change — see
[Technical notes](./technical-notes.md) for why streaming in chunks matters. The
[helper scripts](./scripts.md) expose it directly as `--chunk-rows N`, which
exports it for every `ctddump` process they launch.
