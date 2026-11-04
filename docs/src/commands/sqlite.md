# `sqlite`

Export a produced **Parquet** data file to a **SQLite** database, normalised
into three related tables. This turns the flat observation table into something
you can query with SQL, index, and open in any SQLite-aware tool.

```
ctddump sqlite [OPTIONS] <src.parquet> [dest.sqlite]
```

Without `dest` the output is the source path with its extension changed to
`.sqlite`. The command refuses to overwrite an existing output unless `--force`
is given.

## The schema

The flat Parquet repeats every profile-level value on each observation row.
`sqlite` splits that back apart by grain:

- **`platform`** holds one row per `platform_code`.
- **`profile`** holds one row per `(platform_code, profile_no)`, carrying every
  column that is constant within a profile (time, position, QC, filename), with a
  foreign key to `platform`.
- **`observation`** holds one row per observation (the measurements and their QC
  and conversion flags), with a foreign key to `profile`.

```sql
CREATE TABLE platform (
  platform_id   INTEGER PRIMARY KEY,
  platform_code TEXT UNIQUE NOT NULL
);
CREATE TABLE profile (
  profile_id        INTEGER PRIMARY KEY,
  platform_id       INTEGER NOT NULL REFERENCES platform(platform_id),
  profile_no        INTEGER NOT NULL,
  profile_time      REAL,      -- days since 1950-01-01
  profile_timestamp INTEGER,   -- Unix milliseconds
  longitude         REAL,
  latitude          REAL,
  profile_longitude REAL,
  profile_latitude  REAL,
  time_qc           TEXT,
  position_qc       TEXT,
  filename          TEXT,
  UNIQUE(platform_id, profile_no)
);
CREATE TABLE observation (
  observation_id INTEGER PRIMARY KEY,
  profile_id     INTEGER NOT NULL REFERENCES profile(profile_id),
  observation_no INTEGER NOT NULL,
  temp REAL, temp_qc TEXT, temp_dm TEXT,
  psal REAL, psal_qc TEXT, psal_dm TEXT,
  pres REAL, pres_qc TEXT, pres_dm TEXT, pres_conv INTEGER,
  deph REAL, deph_qc TEXT, deph_conv INTEGER
);
```

Indexes are created on `profile(platform_id)`, `observation(profile_id)`, and
`profile(profile_timestamp)` so the common joins and time queries are fast.

Only the columns of the [standard output schema](../output-schema.md) are
exported; any other column in the Parquet (for example `markdup`'s `is_dup`) is
ignored unless you ask for it with `--add-col`. A column that is absent from the
input is simply left out of its table, so a file that never had `deph` produces a
schema without the depth columns.

## NULL handling

The Parquet uses sentinels for missing values: `NaN` for floats and an empty
string for absent QC flags and data modes. Both are stored as SQL `NULL`, so queries like
`WHERE temp IS NOT NULL` behave the way SQL expects.

## Adding your own columns

Two options attach extra columns to a chosen table. Both name their target as
`TABLE.COLUMN`, where `TABLE` is `platform`, `profile`, or `observation`.

**`--add TABLE.COLUMN=VALUE`** adds a constant column filled with one value on
every row of the table. It is handy for tagging an export with where it came
from. The value type is inferred: a whole number becomes an integer, a decimal
becomes a real, anything else is stored as text. Repeat the flag for several
columns.

**`--add-col TABLE.COLUMN`** copies an existing Parquet column of that name into
the table. For a column added to `observation` the value is taken per row. For a
column added to `profile` or `platform` the value must be **constant within each
group** (each profile, or each platform): if it varies, the export stops with an
error naming the column and the group. Nulls are ignored when checking, so a
column that is set on some rows of a profile and null on others is accepted.

A column name must be a plain identifier and must not clash with a column the
table already has.

## Examples

```bash
# Straight export next to the source (writes arctic.sqlite)
ctddump sqlite arctic.parquet

# Explicit output, overwriting any earlier file
ctddump sqlite --force arctic.parquet /data/arctic.sqlite

# Tag every row with the source product and region
ctddump sqlite --add profile.dataset=NRT_AR --add platform.source=copernicus \
  arctic.parquet arctic.sqlite

# Carry a per-profile institution and a per-observation extra parameter through
ctddump sqlite --add-col profile.institution --add-col observation.chla \
  arctic.parquet arctic.sqlite
```

A typical query against the result, joining all three tables:

```sql
SELECT pl.platform_code, p.profile_no, p.profile_timestamp,
       o.pres, o.temp, o.psal
FROM observation o
JOIN profile  p  ON o.profile_id  = p.profile_id
JOIN platform pl ON p.platform_id = pl.platform_id
WHERE o.temp IS NOT NULL
ORDER BY pl.platform_code, p.profile_no, o.pres;
```

## Memory

The export streams in two passes and never loads the whole file at once. The
first pass builds the `platform` and `profile` rows, so its memory follows the
number of profiles rather than the file size (the same assumption `markdup` and
`dedup` rely on). The second pass streams the observations in fixed windows and
inserts them, so an arbitrarily large observation body loads a window at a time.
