#!/usr/bin/env bash
#
# prepare_data.sh — download CTD data from the Copernicus Marine Service and
# convert/merge it to Parquet (data) and YAML (metadata) with ctddump, for the
# Arctic, Baltic, and Mediterranean seas.
#
# Usage:
#   scripts/prepare_data.sh [command] [region ...]
#
# Commands:
#   login       Log in to the Copernicus Marine Toolbox (once, interactively).
#   download    Download the source NetCDF files.
#   process     Convert + merge Parquet, export + merge headers.  (default)
#   all         download, then process.
#   help        Show this help.
#
# Regions:  arctic  baltic  mediterranean   (default: all three; "all" also works)
#
# Configuration (override via environment):
#   THREADS   worker threads for ctddump           (default: 10)
#   SRC       root of the downloaded NetCDF tree    (default: ../source_data/ctddump/netcdf)
#   OUT       root for the generated outputs        (default: ../process_data/ctddump)
#
# Requires: ctddump on PATH; copernicusmarine on PATH for the download/login steps.
# A free Copernicus Marine account is needed to download:
#   https://help.marine.copernicus.eu/en/collections/9080063-copernicus-marine-toolbox

set -euo pipefail

# ---- Configuration -------------------------------------------------------
THREADS="${THREADS:-10}"
SRC="${SRC:-../source_data/ctddump/netcdf}"
OUT="${OUT:-../process_data/ctddump}"

# Copernicus product directories under $SRC.
NRT_AR_DIR="INSITU_ARC_PHYBGCWAV_DISCRETE_MYNRT_013_031"
NRT_BO_DIR="INSITU_BAL_PHYBGCWAV_DISCRETE_MYNRT_013_032"
NRT_MO_DIR="INSITU_MED_PHYBGCWAV_DISCRETE_MYNRT_013_035"
CORA="INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511"

REGIONS=(arctic baltic mediterranean)

# ---- Reusable ctddump steps ----------------------------------------------
# Each creates its output location so a fresh run works from scratch.

convert() {  # <format> <src_dir> <out_dir>
  mkdir -p "$3"
  ctddump batch convert "$1" --threads "$THREADS" --output "$3" "$2"
}

merge() {  # <src_dir> <out_file>
  mkdir -p "$(dirname "$2")"
  ctddump concat convert --threads "$THREADS" "$1" "$2"
}

header_nrt() {  # <pattern> <src_dir> <out_dir>
  mkdir -p "$3"
  ctddump batch header nrt --threads "$THREADS" --pattern "$1" --output "$3" "$2"
}

header_cora() {  # <src_dir> <out_dir>
  mkdir -p "$2"
  ctddump batch header cora --threads "$THREADS" --output "$2" "$1"
}

merge_hdr() {  # <src_dir> <out_file>
  mkdir -p "$(dirname "$2")"
  ctddump concat header "$1" "$2"
}

# ---- Download ------------------------------------------------------------
login() { copernicusmarine login; }

download_arctic() {
  copernicusmarine get -i cmems_obs-ins_arc_phybgcwav_mynrt_na_irr --dataset-part "history" --filter "*/CT/*"
  copernicusmarine get -i cmems_obs-ins_glo_phy-temp-sal_my_cora_irr --filter "arctic/*/*_PR_CT.nc"
}

download_baltic() {
  copernicusmarine get -i cmems_obs-ins_bal_phybgcwav_mynrt_na_irr --dataset-part "history" --filter "*/CT/*"
  copernicusmarine get -i cmems_obs-ins_glo_phy-temp-sal_my_cora_irr --filter "baltic/*/*_PR_CT.nc"
}

download_mediterranean() {
  copernicusmarine get -i cmems_obs-ins_med_phybgcwav_mynrt_na_irr --dataset-part "history" --filter "*/CT/*"
  copernicusmarine get -i cmems_obs-ins_glo_phy-temp-sal_my_cora_irr --filter "mediterrane/*/*_PR_CT.nc"
}

# ---- Per-region pipelines ------------------------------------------------

process_arctic() {
  local P="$OUT/parquet" H="$OUT/header"
  local nrt="$SRC/$NRT_AR_DIR" cora="$SRC/$CORA/arctic"

  convert nrt_ar "$SRC"  "$P/ar/ar"
  convert nrt_gl "$nrt"  "$P/ar/gl"
  convert cora   "$cora" "$P/ar/cora"

  merge "$P/ar/ar"   "$P/nrt_ar_ar.parquet"
  merge "$P/ar/gl"   "$P/nrt_ar_gl.parquet"
  merge "$P/ar/cora" "$P/cora_ar.parquet"

  header_nrt "AR_PR_CT_*.nc" "$nrt"  "$H/ar/ar"
  header_nrt "GL_PR_CT_*.nc" "$nrt"  "$H/ar/gl"
  header_cora                "$cora" "$H/ar/cora"

  merge_hdr "$H/ar/ar"   "$H/nrt_ar_ar.yaml"
  merge_hdr "$H/ar/gl"   "$H/nrt_ar_gl.yaml"
  merge_hdr "$H/ar/cora" "$H/cora_ar.yaml"
}

# The Baltic workflow uses only the regional NRT (BO) and CORA products; the
# Global (GL) product is not processed here.
process_baltic() {
  local P="$OUT/parquet" H="$OUT/header"
  local nrt="$SRC/$NRT_BO_DIR" cora="$SRC/$CORA/baltic"

  convert nrt_bo "$SRC"  "$P/bo/bo"
  convert cora   "$cora" "$P/bo/cora"

  merge "$P/bo/bo"   "$P/nrt_bo_bo.parquet"
  merge "$P/bo/cora" "$P/cora_bo.parquet"

  header_nrt "BO_PR_CT_*.nc" "$nrt"  "$H/bo/bo"
  header_cora                "$cora" "$H/bo/cora"

  merge_hdr "$H/bo/bo"   "$H/nrt_bo_bo.yaml"
  merge_hdr "$H/bo/cora" "$H/cora_bo.yaml"
}

process_mediterranean() {
  local P="$OUT/parquet" H="$OUT/header"
  local nrt="$SRC/$NRT_MO_DIR" cora="$SRC/$CORA/mediterrane"

  convert nrt_mo "$SRC"  "$P/mo/mo"
  convert nrt_gl "$nrt"  "$P/mo/gl"
  convert cora   "$cora" "$P/mo/cora"

  merge "$P/mo/mo"   "$P/nrt_mo_mo.parquet"
  merge "$P/mo/gl"   "$P/nrt_mo_gl.parquet"
  merge "$P/mo/cora" "$P/cora_mo.parquet"

  header_nrt "MO_PR_CT_*.nc" "$nrt"  "$H/mo/mo"
  header_nrt "GL_PR_CT_*.nc" "$nrt"  "$H/mo/gl"
  header_cora                "$cora" "$H/mo/cora"

  merge_hdr "$H/mo/mo"   "$H/nrt_mo_mo.yaml"
  merge_hdr "$H/mo/gl"   "$H/nrt_mo_gl.yaml"
  merge_hdr "$H/mo/cora" "$H/cora_mo.yaml"
}

# ---- Dispatch ------------------------------------------------------------

usage() { sed -n '3,26p' "$0" | sed 's/^# \{0,1\}//'; }

is_region() {
  local r
  for r in "${REGIONS[@]}"; do [[ "$r" == "$1" ]] && return 0; done
  return 1
}

main() {
  local cmd="${1:-process}"
  [[ $# -gt 0 ]] && shift

  case "$cmd" in
    -h|--help|help) usage; return 0 ;;
    login) login; return 0 ;;
    download|process|all) ;;
    *) echo "Unknown command: $cmd" >&2; usage; return 1 ;;
  esac

  # Remaining args are regions; default to all, and "all" is an alias.
  local -a regions=("$@")
  if [[ ${#regions[@]} -eq 0 || "${regions[0]}" == "all" ]]; then
    regions=("${REGIONS[@]}")
  fi
  for r in "${regions[@]}"; do
    is_region "$r" || { echo "Unknown region: $r" >&2; usage; return 1; }
  done

  for r in "${regions[@]}"; do
    case "$cmd" in
      download) "download_$r" ;;
      process)  "process_$r" ;;
      all)      "download_$r"; "process_$r" ;;
    esac
  done
}

main "$@"
