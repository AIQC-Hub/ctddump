# Arctic Sea

An end-to-end workflow for the Arctic Sea: download the source files, convert
them to Parquet, merge them, and export the metadata.

## Prerequisites

Downloading requires a free [Copernicus Marine](https://marine.copernicus.eu/)
account and the **Copernicus Marine Toolbox**
([documentation](https://help.marine.copernicus.eu/en/collections/9080063-copernicus-marine-toolbox)),
which provides the `copernicusmarine` command used below.

Create working directories and log in once:

```shell
mkdir copernicus parquet
cd copernicus
copernicusmarine login
```

## 1. Download the data

```shell
# NRT — Arctic (AR) and Global (GL)
copernicusmarine get -i cmems_obs-ins_arc_phybgcwav_mynrt_na_irr --dataset-part "history" --filter "*/CT/*"

# CORA — Arctic
copernicusmarine get -i cmems_obs-ins_glo_phy-temp-sal_my_cora_irr --filter "arctic/*/*_PR_CT.nc"
```

## 2. Convert NetCDF to Parquet

```shell
# NRT AR
ctddump batch convert nrt_ar --threads 10 --output ../process_data/ctddump/parquet/ar/ar ../source_data/ctddump/netcdf

# NRT GL
ctddump batch convert nrt_gl --threads 10 --output ../process_data/ctddump/parquet/ar/gl ../source_data/ctddump/netcdf/INSITU_ARC_PHYBGCWAV_DISCRETE_MYNRT_013_031

# CORA AR
ctddump batch convert cora --threads 10 --output ../process_data/ctddump/parquet/ar/cora ../source_data/ctddump/netcdf/INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511/arctic
```

## 3. Merge the Parquet files

```shell
# NRT AR
ctddump concat convert --threads 10 ../process_data/ctddump/parquet/ar/ar ../process_data/ctddump/parquet/nrt_ar_ar.parquet

# NRT GL
ctddump concat convert --threads 10 ../process_data/ctddump/parquet/ar/gl ../process_data/ctddump/parquet/nrt_ar_gl.parquet

# CORA AR
ctddump concat convert --threads 10 ../process_data/ctddump/parquet/ar/cora ../process_data/ctddump/parquet/cora_ar.parquet
```

## 4. Export the metadata (headers)

```shell
# NRT AR
ctddump batch header nrt --threads 10 --pattern "AR_PR_CT_*.nc" --output ../process_data/ctddump/header/ar/ar ../source_data/ctddump/netcdf/INSITU_ARC_PHYBGCWAV_DISCRETE_MYNRT_013_031

# NRT GL
ctddump batch header nrt --threads 10 --pattern "GL_PR_CT_*.nc" --output ../process_data/ctddump/header/ar/gl ../source_data/ctddump/netcdf/INSITU_ARC_PHYBGCWAV_DISCRETE_MYNRT_013_031

# CORA AR
ctddump batch header cora --threads 10 --output ../process_data/ctddump/header/ar/cora ../source_data/ctddump/netcdf/INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511/arctic
```

## 5. Merge the header files

```shell
# NRT AR
ctddump concat header ../process_data/ctddump/header/ar/ar ../process_data/ctddump/header/nrt_ar_ar.yaml

# NRT GL
ctddump concat header ../process_data/ctddump/header/ar/gl ../process_data/ctddump/header/nrt_ar_gl.yaml

# CORA AR
ctddump concat header ../process_data/ctddump/header/ar/cora ../process_data/ctddump/header/cora_ar.yaml
```

## 6. Summarise the results

Write a global-level summary of each merged Parquet file and a per-file summary
of each merged header YAML (as TSV).

```shell
# NRT AR
ctddump report parquet --level global ../process_data/ctddump/parquet/nrt_ar_ar.parquet ../process_data/ctddump/report/nrt_ar_ar.parquet.tsv
ctddump report yaml ../process_data/ctddump/header/nrt_ar_ar.yaml ../process_data/ctddump/report/nrt_ar_ar.yaml.tsv

# NRT GL
ctddump report parquet --level global ../process_data/ctddump/parquet/nrt_ar_gl.parquet ../process_data/ctddump/report/nrt_ar_gl.parquet.tsv
ctddump report yaml ../process_data/ctddump/header/nrt_ar_gl.yaml ../process_data/ctddump/report/nrt_ar_gl.yaml.tsv

# CORA AR
ctddump report parquet --level global ../process_data/ctddump/parquet/cora_ar.parquet ../process_data/ctddump/report/cora_ar.parquet.tsv
ctddump report yaml ../process_data/ctddump/header/cora_ar.yaml ../process_data/ctddump/report/cora_ar.yaml.tsv
```
