#!/usr/bin/env bash
#
# clean_data.sh — clean the merged Parquet outputs of prepare_data.sh with
# ctddump, for the Arctic, Baltic, and Mediterranean seas. Four steps run in
# order, each reading the previous step's output:
#
#   dropqc   Drop profiles flagged bad in time_qc / position_qc.
#   dropna   Drop profiles that are all-NA in temp, psal, or pres.
#   filter   Keep profiles inside the region's bounding box(es).
#   report   Summarise the cleaned Parquet outputs as TSV.
#
# The stages chain through sub-directories of $OUT/clean:
#   $OUT/parquet -> clean/dropqc -> clean/dropna -> clean/filter
# and reports land in $OUT/report/clean/  (prepare_data.sh uses report/prepare/).
#
# Usage:
#   scripts/clean_data.sh [options] [command] [region ...]
#
# Commands:
#   dropqc      Drop bad-QC profiles.
#   dropna      Drop all-NA profiles.
#   filter      Keep profiles inside the region bounding box(es).
#   report      Summarise the cleaned Parquet outputs as TSV.
#   all         dropqc, dropna, filter, then report.  (default)
#   help        Show this help.
#
# Regions:  arctic  baltic  mediterranean   (default: all three; "all" also works)
#
# Options (may appear anywhere on the command line):
#   -o, --out DIR   root for the prepare_data.sh outputs and the cleaned
#                   outputs (default: output)
#   -y, --yes       Skip the confirmation prompt and start immediately.
#   -h, --help      Show this help.
#
# Requires: ctddump on PATH, and prepare_data.sh's merged Parquet in <out>/parquet.

set -euo pipefail

usage() { awk 'NR<3 {next} /^#/ {sub(/^# ?/, ""); print; next} {exit}' "$0"; }

# ---- Configuration (defaults; override with the options below) -----------
OUT=output
ASSUME_YES=0

# ---- Parse options -------------------------------------------------------
# Options may appear anywhere; the remaining words are the command and regions.
ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o|--out)  OUT="${2:?--out requires a directory}"; shift 2 ;;
    --out=*)   OUT="${1#*=}"; shift ;;
    -y|--yes)  ASSUME_YES=1; shift ;;
    -h|--help) usage; exit 0 ;;
    --)        shift; ARGS+=("$@"); break ;;
    -*)        echo "Unknown option: $1" >&2; usage; exit 1 ;;
    *)         ARGS+=("$1"); shift ;;
  esac
done

# Stage directories (each step reads the previous one).
SRC_DIR="$OUT/parquet"          # prepare_data.sh merged Parquet (input)
QC_DIR="$OUT/clean/dropqc"
NA_DIR="$OUT/clean/dropna"
CLEAN_DIR="$OUT/clean/filter"   # final cleaned data
REPORT_DIR="$OUT/report/clean"

REGIONS=(arctic baltic mediterranean)

# Merged-file stems produced by prepare_data.sh, per region.
stems_for() {  # <region>
  case "$1" in
    arctic)        echo nrt_ar_ar nrt_ar_gl cora_ar ;;
    baltic)        echo nrt_bo_bo cora_bo ;;
    mediterranean) echo nrt_mo_mo nrt_mo_gl cora_mo ;;
  esac
}

# ---- Logging -------------------------------------------------------------
# Announce each step (timestamped, to stderr) so the currently running process
# is visible.

log() { printf '[%s] %s\n' "$(date '+%H:%M:%S')" "$*" >&2; }

# Print the resolved configuration, then ask for confirmation unless -y/--yes was
# given. In a non-interactive shell without -y there is nothing to read, so abort
# with a hint rather than hang.
show_config() {  # <cmd> <region...>
  local cmd="$1"; shift
  {
    echo "Configuration:"
    echo "  command : $cmd"
    echo "  regions : $*"
    echo "  out     : $OUT"
  } >&2
}

confirm() {
  [[ "$ASSUME_YES" == 1 ]] && return 0
  if [[ ! -t 0 ]]; then
    log "non-interactive shell: pass -y/--yes to proceed without confirmation."
    return 1
  fi
  local reply
  read -r -p "Proceed? [y/N] " reply
  [[ "$reply" == [yY] || "$reply" == [yY][eE][sS] ]]
}

# ---- Region bounding boxes ------------------------------------------------
# Keep profiles inside a region, then carve out excluded sub-boxes. `filter`
# defaults to --mode include (keep inside the box); --mode exclude drops inside
# it. Multi-box regions chain through temp files, removed afterwards.

filter_box_arctic() {  # <src> <dest>
  ctddump filter --min-lon -180 --max-lon 180 --min-lat 60 --max-lat 90 "$1" "$2"
}

filter_box_baltic() {  # <src> <dest>
  local t="$2.tmp1"
  ctddump filter                --min-lon 6 --max-lon 30 --min-lat 53 --max-lat 66 "$1" "$t"
  ctddump filter --mode exclude --min-lon 6 --max-lon 15 --min-lat 60 --max-lat 66 "$t" "$2"
  rm -f "$t"
}

filter_box_mediterranean() {  # <src> <dest>
  local t1="$2.tmp1" t2="$2.tmp2"
  ctddump filter                --min-lon -5.61 --max-lon 35.567 --min-lat 28.378 --max-lat 45.755 "$1"  "$t1"
  ctddump filter --mode exclude --min-lon 27    --max-lon 36     --min-lat 41     --max-lat 46      "$t1" "$t2"
  ctddump filter --mode exclude --min-lon -5.61 --max-lon 0      --min-lat 42     --max-lat 46      "$t2" "$2"
  rm -f "$t1" "$t2"
}

# ---- Reusable per-region steps -------------------------------------------
# Each creates its output location so a fresh run works from scratch.

dropqc_region() {  # <region>
  mkdir -p "$QC_DIR"
  local s
  for s in $(stems_for "$1"); do
    log "dropqc $1/$s"
    ctddump dropqc "$SRC_DIR/$s.parquet" "$QC_DIR/$s.parquet"
  done
}

dropna_region() {  # <region>
  mkdir -p "$NA_DIR"
  local s
  for s in $(stems_for "$1"); do
    log "dropna $1/$s"
    ctddump dropna "$QC_DIR/$s.parquet" "$NA_DIR/$s.parquet"
  done
}

filter_region() {  # <region>
  mkdir -p "$CLEAN_DIR"
  local s
  for s in $(stems_for "$1"); do
    log "filter $1/$s"
    "filter_box_$1" "$NA_DIR/$s.parquet" "$CLEAN_DIR/$s.parquet"
  done
}

report_region() {  # <region>
  mkdir -p "$REPORT_DIR"
  local s
  for s in $(stems_for "$1"); do
    log "report $1/$s"
    ctddump report parquet --level platform "$CLEAN_DIR/$s.parquet" "$REPORT_DIR/$s.parquet.tsv"
  done
}

# ---- Dispatch ------------------------------------------------------------

is_region() {
  local r
  for r in "${REGIONS[@]}"; do [[ "$r" == "$1" ]] && return 0; done
  return 1
}

main() {
  local cmd="${1:-all}"
  [[ $# -gt 0 ]] && shift

  case "$cmd" in
    -h|--help|help) usage; return 0 ;;
    dropqc|dropna|filter|report|all) ;;
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

  show_config "$cmd" "${regions[@]}"
  confirm || { log "aborted."; return 1; }

  for r in "${regions[@]}"; do
    log "===== $cmd: $r ====="
    case "$cmd" in
      dropqc) "dropqc_region" "$r" ;;
      dropna) "dropna_region" "$r" ;;
      filter) "filter_region" "$r" ;;
      report) "report_region" "$r" ;;
      all)    "dropqc_region" "$r"; "dropna_region" "$r"; "filter_region" "$r"; "report_region" "$r" ;;
    esac
  done
  log "done."
}

main ${ARGS[@]+"${ARGS[@]}"}
