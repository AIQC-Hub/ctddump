# Baltic Sea

An end-to-end workflow for the Baltic Sea in two phases: **data preparation**
(download, convert, merge, and export the metadata) and **data cleaning** (drop
low-quality profiles and restrict the data to the region).

> This workflow uses the regional **NRT (BO)** and **CORA** products. The Global
> (GL) product is not used for the Baltic here.

## Data preparation

### Prerequisites

Downloading requires a free [Copernicus Marine](https://marine.copernicus.eu/)
account and the **Copernicus Marine Toolbox**
([documentation](https://help.marine.copernicus.eu/en/collections/9080063-copernicus-marine-toolbox)),
which provides the `copernicusmarine` command used below.

Run everything from a working directory (e.g. `ctddump`). Downloads land under
`input/`; ctddump writes its results under `output/` (both created as needed).
Create `input`, enter it, and log in once:

```shell
mkdir input
cd input
copernicusmarine login
```

### 1. Download the data

```shell
# NRT — Baltic (BO)
copernicusmarine get -i cmems_obs-ins_bal_phybgcwav_mynrt_na_irr --dataset-part "history" --filter "*/CT/*"

# CORA — Baltic
copernicusmarine get -i cmems_obs-ins_glo_phy-temp-sal_my_cora_irr --filter "baltic/*/*_PR_CT.nc"

# Back to the working root; the steps below use input/ and output/ relative to it.
cd ..
```

### 2. Convert NetCDF to Parquet

```shell
# NRT BO
ctddump batch convert nrt_bo --threads 10 --output output/parquet/bo/bo input

# CORA BO
ctddump batch convert cora --threads 10 --output output/parquet/bo/cora input/INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511/baltic
```

### 3. Merge the Parquet files

```shell
# NRT BO
ctddump concat convert --threads 10 output/parquet/bo/bo output/parquet/nrt_bo_bo.parquet

# CORA BO
ctddump concat convert --threads 10 output/parquet/bo/cora output/parquet/cora_bo.parquet
```

### 4. Export the metadata (headers)

```shell
# NRT BO
ctddump batch header nrt --threads 10 --pattern "BO_PR_CT_*.nc" --output output/header/bo/bo input/INSITU_BAL_PHYBGCWAV_DISCRETE_MYNRT_013_032

# CORA BO
ctddump batch header cora --threads 10 --output output/header/bo/cora input/INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511/baltic
```

### 5. Merge the header files

```shell
# NRT BO
ctddump concat header output/header/bo/bo output/header/nrt_bo_bo.yaml

# CORA BO
ctddump concat header output/header/bo/cora output/header/cora_bo.yaml
```

### 6. Summarise the results

Write a platform-level summary of each merged Parquet file and a per-file summary
of each merged header YAML (as TSV).

```shell
mkdir -p output/report/convert

# NRT BO
ctddump report parquet --level platform output/parquet/nrt_bo_bo.parquet output/report/convert/nrt_bo_bo.parquet.tsv
ctddump report yaml output/header/nrt_bo_bo.yaml output/report/convert/nrt_bo_bo.yaml.tsv

# CORA BO
ctddump report parquet --level platform output/parquet/cora_bo.parquet output/report/convert/cora_bo.parquet.tsv
ctddump report yaml output/header/cora_bo.yaml output/report/convert/cora_bo.yaml.tsv
```

## Data cleaning

Clean the merged Parquet from the preparation phase by dropping low-quality
profiles and restricting the data to the region. Each step reads the previous
step's output, so the stages chain `dropqc → dropna → filter`.

Create the output directories:

```shell
mkdir -p output/clean/dropqc output/clean/dropna output/clean/filter output/report/clean
```

### 1. Drop profiles with bad profile-level QC

Drop profiles whose `time_qc` or `position_qc` is a present, non-OK flag;
profiles that are OK (`"1"`) or have missing QC are kept.

```shell
# NRT BO
ctddump dropqc output/parquet/nrt_bo_bo.parquet output/clean/dropqc/nrt_bo_bo.parquet

# CORA BO
ctddump dropqc output/parquet/cora_bo.parquet output/clean/dropqc/cora_bo.parquet
```

### 2. Drop profiles with no usable data

Drop profiles that are entirely NA in any of `temp`, `psal`, or `pres`.

```shell
# NRT BO
ctddump dropna output/clean/dropqc/nrt_bo_bo.parquet output/clean/dropna/nrt_bo_bo.parquet

# CORA BO
ctddump dropna output/clean/dropqc/cora_bo.parquet output/clean/dropna/cora_bo.parquet
```

### 3. Filter to the Baltic region

Keep profiles inside the Baltic bounding box (longitude 6 to 30, latitude 53 to
66), then exclude the sub-box (longitude 6 to 15, latitude 60 to 66). The include
step writes an intermediate `.box.parquet` file that the exclude step consumes to
produce the final cleaned file.

```shell
# NRT BO
ctddump filter --min-lon 6 --max-lon 30 --min-lat 53 --max-lat 66 output/clean/dropna/nrt_bo_bo.parquet output/clean/filter/nrt_bo_bo.box.parquet
ctddump filter --mode exclude --min-lon 6 --max-lon 15 --min-lat 60 --max-lat 66 output/clean/filter/nrt_bo_bo.box.parquet output/clean/filter/nrt_bo_bo.parquet

# CORA BO
ctddump filter --min-lon 6 --max-lon 30 --min-lat 53 --max-lat 66 output/clean/dropna/cora_bo.parquet output/clean/filter/cora_bo.box.parquet
ctddump filter --mode exclude --min-lon 6 --max-lon 15 --min-lat 60 --max-lat 66 output/clean/filter/cora_bo.box.parquet output/clean/filter/cora_bo.parquet
```

### 4. Summarise the cleaned data

```shell
# NRT BO
ctddump report parquet --level platform output/clean/filter/nrt_bo_bo.parquet output/report/clean/nrt_bo_bo.parquet.tsv

# CORA BO
ctddump report parquet --level platform output/clean/filter/cora_bo.parquet output/report/clean/cora_bo.parquet.tsv
```

## Data de-duplication

De-duplicate the cleaned Parquet from the previous phase. Two profiles are
duplicates when they share the same date and position (longitude/latitude rounded
to 3 decimals) — ctddump's defaults, across platforms. `markdup` flags them (and
lists them in a TSV); `dedup` removes them, keeping the profile with the most
observations.

Create the output directories:

```shell
mkdir -p output/dedup/markdup output/dedup/dedup output/report/dedup/markdup output/report/dedup/dedup
```

### 1. Mark duplicate profiles

```shell
# NRT BO
ctddump markdup output/clean/filter/nrt_bo_bo.parquet output/dedup/markdup/nrt_bo_bo.parquet output/dedup/markdup/nrt_bo_bo.dups.tsv

# CORA BO
ctddump markdup output/clean/filter/cora_bo.parquet output/dedup/markdup/cora_bo.parquet output/dedup/markdup/cora_bo.dups.tsv
```

### 2. Summarise the marked data (duplicate counts)

```shell
# NRT BO
ctddump report parquet --level platform output/dedup/markdup/nrt_bo_bo.parquet output/report/dedup/markdup/nrt_bo_bo.parquet.tsv

# CORA BO
ctddump report parquet --level platform output/dedup/markdup/cora_bo.parquet output/report/dedup/markdup/cora_bo.parquet.tsv
```

### 3. Remove duplicate profiles

```shell
# NRT BO
ctddump dedup output/dedup/markdup/nrt_bo_bo.parquet output/dedup/dedup/nrt_bo_bo.parquet

# CORA BO
ctddump dedup output/dedup/markdup/cora_bo.parquet output/dedup/dedup/cora_bo.parquet
```

### 4. Summarise the de-duplicated data

```shell
# NRT BO
ctddump report parquet --level platform output/dedup/dedup/nrt_bo_bo.parquet output/report/dedup/dedup/nrt_bo_bo.parquet.tsv

# CORA BO
ctddump report parquet --level platform output/dedup/dedup/cora_bo.parquet output/report/dedup/dedup/cora_bo.parquet.tsv
```

> The pipeline is automated by
> [`scripts/download_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/download_data.sh),
> [`scripts/convert_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/convert_data.sh),
> [`scripts/clean_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/clean_data.sh),
> and [`scripts/dedup_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/dedup_data.sh)
> — see [Helper scripts](../scripts.md) for their commands and options.
