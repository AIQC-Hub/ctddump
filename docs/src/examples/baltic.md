# Baltic Sea

An end-to-end workflow for the Baltic Sea: download the source files, convert
them to Parquet, merge them, and export the metadata.

> This workflow uses the regional **NRT (BO)** and **CORA** products. The Global
> (GL) product is not used for the Baltic here.

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
# NRT — Baltic (BO)
copernicusmarine get -i cmems_obs-ins_bal_phybgcwav_mynrt_na_irr --dataset-part "history" --filter "*/CT/*"

# CORA — Baltic
copernicusmarine get -i cmems_obs-ins_glo_phy-temp-sal_my_cora_irr --filter "baltic/*/*_PR_CT.nc"
```

## 2. Convert NetCDF to Parquet

```shell
# NRT BO
ctddump batch convert nrt_bo --threads 10 --output ../process_data/ctddump/parquet/bo/bo ../source_data/ctddump/netcdf

# CORA BO
ctddump batch convert cora --threads 10 --output ../process_data/ctddump/parquet/bo/cora ../source_data/ctddump/netcdf/INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511/baltic
```

## 3. Merge the Parquet files

```shell
# NRT BO
ctddump concat convert --threads 10 ../process_data/ctddump/parquet/bo/bo ../process_data/ctddump/parquet/nrt_bo_bo.parquet

# CORA BO
ctddump concat convert --threads 10 ../process_data/ctddump/parquet/bo/cora ../process_data/ctddump/parquet/cora_bo.parquet
```

## 4. Export the metadata (headers)

```shell
# NRT BO
ctddump batch header nrt --threads 10 --pattern "BO_PR_CT_*.nc" --output ../process_data/ctddump/header/bo/bo ../source_data/ctddump/netcdf/INSITU_BAL_PHYBGCWAV_DISCRETE_MYNRT_013_032

# CORA BO
ctddump batch header cora --threads 10 --output ../process_data/ctddump/header/bo/cora ../source_data/ctddump/netcdf/INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511/baltic
```

## 5. Merge the header files

```shell
# NRT BO
ctddump concat header ../process_data/ctddump/header/bo/bo ../process_data/ctddump/header/nrt_bo_bo.yaml

# CORA BO
ctddump concat header ../process_data/ctddump/header/bo/cora ../process_data/ctddump/header/cora_bo.yaml
```

## 6. Summarise the results

Write a platform-level summary of each merged Parquet file and a per-file summary
of each merged header YAML (as TSV).

```shell
# NRT BO
ctddump report parquet --level platform ../process_data/ctddump/parquet/nrt_bo_bo.parquet ../process_data/ctddump/report/nrt_bo_bo.parquet.tsv
ctddump report yaml ../process_data/ctddump/header/nrt_bo_bo.yaml ../process_data/ctddump/report/nrt_bo_bo.yaml.tsv

# CORA BO
ctddump report parquet --level platform ../process_data/ctddump/parquet/cora_bo.parquet ../process_data/ctddump/report/cora_bo.parquet.tsv
ctddump report yaml ../process_data/ctddump/header/cora_bo.yaml ../process_data/ctddump/report/cora_bo.yaml.tsv
```
