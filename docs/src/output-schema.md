# Output schema

Every converter produces the **same** observation-level flat table, so files
from different sources can be merged directly.

| Column | Type | Notes |
|--------|------|-------|
| `platform_code` | `String` | |
| `profile_no` | `u32` | |
| `profile_time` | `f64` | days since 1950-01-01 |
| `profile_timestamp` | `Datetime(ms)` | Unix milliseconds |
| `observation_no` | `u32` | |
| `longitude` / `latitude` | `f32` (NRT) / `f64` (CORA) | |
| `profile_longitude` / `profile_latitude` | `f32` (NRT) / `f64` (CORA) | from `PRECISE_*` or expanded `DEPLOY_*`; NaN when unavailable |
| `time_qc` / `position_qc` | `String` | `""` if absent |
| `filename` | `String` | source file stem |
| `temp`, `psal`, `pres`, `deph` | `f32` | NaN where missing |
| `temp_qc`, `psal_qc`, `pres_qc`, `deph_qc` | `String` | single-char flag; `""` if missing |
| `pres_conv`, `deph_conv` | `i8` | `1` = value derived by conversion |

Pressure and depth are converted to each other with the TEOS-10 (`gsw`)
routines when only one is present, and the `*_conv` columns flag which values
were derived rather than measured.
