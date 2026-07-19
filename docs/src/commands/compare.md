# `compare`

Compare two produced **Parquet** data files and report how far each one covers
the other's platforms and profiles.

```
ctddump compare [OPTIONS] <a.parquet> <b.parquet> [dest]
```

Without `dest` the report goes to stdout.

## What it answers

Two questions, for a pair of files:

- **How much do they have in common?** Platforms present in both, and profiles
  present in both.
- **Do the matched profiles agree?** For every profile found in both files,
  whether it has the same number of observation rows in each.

Coverage is reported **in both directions**, because it is not symmetric. A small
file fully contained in a large one is 100% covered while covering only a
fraction of the large one. The first output row uses the **second** file as the
reference, the second row uses the first.

## How profiles are matched

A profile's key is built from its platform code, its time reduced to a **date**,
and its longitude and latitude rounded to **3 decimals**. This is the same key
[`markdup`](./markdup.md) uses for duplicates, with one difference: `markdup`
deliberately leaves the platform code out, while `compare` includes it by
default. Two files being compared are usually different extracts of the same
platforms, so the platform code is normally a wanted part of the identity. Pass
`--no-platform-key` to match on time and position alone, which finds the same
cast recorded under two different platform codes.

Profiles whose time or position is missing get no key and can never match. They
are counted separately as `ref_unkeyed_profiles` so a low coverage figure can be
told apart from a file full of unusable positions.

The time column may be either a datetime (`profile_timestamp`) or a float of days
since 1950 (`profile_time`); the type is detected and handled automatically.

## Output columns

One row per direction:

| Column | Meaning |
|--------|---------|
| `reference` | File the percentages are relative to |
| `compared` | The other file |
| `ref_platforms` | Distinct platform codes in the reference |
| `common_platforms` | Platform codes present in both files |
| `platform_cov_pct` | `common_platforms` as a percentage of `ref_platforms` |
| `ref_profiles` | Profiles in the reference |
| `ref_unkeyed_profiles` | Reference profiles with no usable time or position |
| `matched_profiles` | Reference profiles whose key is present in the other file |
| `profile_cov_pct` | `matched_profiles` as a percentage of `ref_profiles` |
| `same_nobs` | Matched profiles with the same observation count in both files |
| `diff_nobs` | Matched profiles whose observation count differs |
| `nobs_agree_pct` | `same_nobs` as a percentage of `matched_profiles` |
| `ref_observations` | Observation rows in the reference |
| `matched_observations` | Observation rows in the reference's matched profiles |

`profile_cov_pct` is taken over **all** reference profiles, including unkeyed
ones, since a profile that cannot be matched is still a profile the other file
does not demonstrably have. A percentage with no denominator (comparing against
an empty file, or `nobs_agree_pct` when nothing matched) is left empty rather
than shown as zero.

When several profiles share one key, the observation counts agree if **any** of
the profiles carrying that key in the other file has the same count.

## Options

| Option | Default | Meaning |
|--------|---------|---------|
| `--time-format <FMT>` | `%Y-%m-%d` | strftime format applied to the time column for the key |
| `--decimals <N>` | `3` | decimal places `longitude`/`latitude` are rounded to |
| `--round-mode <MODE>` | `round` | `round`, `floor`, `ceil`, or `trunc` |
| `--platform-col <NAME>` | `platform_code` | column holding the platform code |
| `--time-col <NAME>` | `profile_time` | column holding the profile time |
| `--lon-col <NAME>` | `longitude` | column holding the longitude |
| `--lat-col <NAME>` | `latitude` | column holding the latitude |
| `--no-platform-key` | off | match on time and position only |
| `--format <FMT>` | `tsv` | `tsv`, `text`, or `json` |

## Examples

```bash
# Default key, report to stdout as an aligned table
ctddump compare --format text old.parquet new.parquet

# Save the summary
ctddump compare old.parquet new.parquet compare.tsv

# Ignore platform codes, so the same cast filed under two codes still matches
ctddump compare --no-platform-key old.parquet new.parquet

# Match to the hour and to 4 decimals instead of the date and 3
ctddump compare --time-format '%Y-%m-%dT%H' --decimals 4 old.parquet new.parquet
```

Reading the result: if the smaller file shows `profile_cov_pct` near 100 while
the larger shows much less, the smaller is close to a subset. If both are low,
the files overlap only partly. A high `profile_cov_pct` with a low
`nobs_agree_pct` means the same profiles are present in both but carry different
numbers of observations, which usually points at different QC or cleaning having
been applied.

Memory is bounded: each file is streamed and reduced to one record per profile,
so peak use follows the profile count rather than the file size.
