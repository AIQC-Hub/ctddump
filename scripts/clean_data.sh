#!/usr/bin/env bash
#
# clean_data.sh — clean the merged Parquet outputs of convert_data.sh with
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
# and reports land in $OUT/report/clean/  (convert_data.sh uses report/convert/).
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
#   -o, --out DIR   root for the convert_data.sh outputs and the cleaned
#                   outputs (default: output)
#   --by-region     Parallelise per region instead of per file (coarser: one
#                   worker per region, its files processed in order).
#   --sequential    Process everything one file at a time (no parallelism).
#   -y, --yes       Skip the confirmation prompt and start immediately.
#   -h, --help      Show this help.
#
# By default the selected files (a <stem> within a region) are processed in
# parallel, one worker per file. Requires ctddump on PATH, and convert_data.sh's
# merged Parquet in <out>/parquet.

set -euo pipefail

usage() { awk 'NR<3 {next} /^#/ {sub(/^# ?/, ""); print; next} {exit}' "$0"; }

# ---- Configuration (defaults; override with the options below) -----------
OUT=output
ASSUME_YES=0
PARALLEL=file   # file | region | none

# ---- Parse options -------------------------------------------------------
# Options may appear anywhere; the remaining words are the command and regions.
ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o|--out)     OUT="${2:?--out requires a directory}"; shift 2 ;;
    --out=*)      OUT="${1#*=}"; shift ;;
    --by-region)  PARALLEL=region; shift ;;
    --sequential) PARALLEL=none; shift ;;
    -y|--yes)     ASSUME_YES=1; shift ;;
    -h|--help)    usage; exit 0 ;;
    --)           shift; ARGS+=("$@"); break ;;
    -*)           echo "Unknown option: $1" >&2; usage; exit 1 ;;
    *)            ARGS+=("$1"); shift ;;
  esac
done

# Stage directories (each step reads the previous one).
SRC_DIR="$OUT/parquet"          # convert_data.sh merged Parquet (input)
QC_DIR="$OUT/clean/dropqc"
NA_DIR="$OUT/clean/dropna"
CLEAN_DIR="$OUT/clean/filter"   # final cleaned data
REPORT_DIR="$OUT/report/clean"

REGIONS=(arctic baltic mediterranean)

# Merged-file stems produced by convert_data.sh, per region.
stems_for() {  # <region>
  case "$1" in
    arctic)        echo nrt_ar_ar nrt_ar_gl cora_ar ;;
    baltic)        echo nrt_bo_bo nrt_bo_gl cora_bo ;;
    mediterranean) echo nrt_mo_mo nrt_mo_gl cora_mo ;;
  esac
}

# ---- Logging -------------------------------------------------------------
# Announce each step (timestamped, to stderr) so the currently running process
# is visible.

# Each parallel region worker sets REGION so its lines are tagged "[region]".
log() {
  local p=""
  [[ -n "${REGION:-}" ]] && p="[$REGION] "
  printf '[%s] %s%s\n' "$(date '+%H:%M:%S')" "$p" "$*" >&2
}

# Print the resolved configuration, then ask for confirmation unless -y/--yes was
# given. In a non-interactive shell without -y there is nothing to read, so abort
# with a hint rather than hang.
show_config() {  # <cmd> <region...>
  local cmd="$1"; shift
  local -a rs=("$@")
  local nfiles=0 r s
  for r in "${rs[@]}"; do for s in $(stems_for "$r"); do nfiles=$((nfiles + 1)); done; done
  local mode
  case "$PARALLEL" in
    none)   mode="sequential" ;;
    region) [[ ${#rs[@]} -gt 1 ]] && mode="parallel (per region)" || mode="sequential" ;;
    file)   [[ $nfiles -gt 1 ]] && mode="parallel (per file)" || mode="sequential" ;;
  esac
  {
    echo "Configuration:"
    echo "  command : $cmd"
    echo "  regions : ${rs[*]}"
    echo "  files   : $nfiles"
    echo "  out     : $OUT"
    echo "  mode    : $mode"
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

# ---- Per-file steps ------------------------------------------------------
# Each operates on a single merged file (a <stem> within a <region>) and creates
# its output location so a fresh run works from scratch. The <region> selects the
# bounding box for the filter step; other steps ignore it.

# A file whose input is missing is skipped (with a note), not an error — so an
# unavailable dataset such as the Baltic Global (GL) product doesn't fail the run.

do_dropqc() {  # <region> <stem>
  [[ -f "$SRC_DIR/$2.parquet" ]] || { log "skip dropqc $2 (missing input)"; return 0; }
  mkdir -p "$QC_DIR"; log "dropqc $2"
  ctddump dropqc "$SRC_DIR/$2.parquet" "$QC_DIR/$2.parquet"
}

do_dropna() {  # <region> <stem>
  [[ -f "$QC_DIR/$2.parquet" ]] || { log "skip dropna $2 (missing input)"; return 0; }
  mkdir -p "$NA_DIR"; log "dropna $2"
  ctddump dropna "$QC_DIR/$2.parquet" "$NA_DIR/$2.parquet"
}

do_filter() {  # <region> <stem>
  [[ -f "$NA_DIR/$2.parquet" ]] || { log "skip filter $2 (missing input)"; return 0; }
  mkdir -p "$CLEAN_DIR"; log "filter $2"
  "filter_box_$1" "$NA_DIR/$2.parquet" "$CLEAN_DIR/$2.parquet"
}

do_report() {  # <region> <stem>
  [[ -f "$CLEAN_DIR/$2.parquet" ]] || { log "skip report $2 (missing input)"; return 0; }
  mkdir -p "$REPORT_DIR"; log "report $2"
  ctddump report parquet --level platform "$CLEAN_DIR/$2.parquet" "$REPORT_DIR/$2.parquet.tsv"
}

# Run <cmd> for one file. For `all` the steps chain dropqc -> dropna -> filter ->
# report on that file (each stage reads the previous stage's output).
run_file() {  # <cmd> <region> <stem>
  local cmd="$1" r="$2" s="$3"
  case "$cmd" in
    dropqc) do_dropqc "$r" "$s" ;;
    dropna) do_dropna "$r" "$s" ;;
    filter) do_filter "$r" "$s" ;;
    report) do_report "$r" "$s" ;;
    all)    do_dropqc "$r" "$s"; do_dropna "$r" "$s"; do_filter "$r" "$s"; do_report "$r" "$s" ;;
  esac
}

# Run <cmd> for every file of <region>, in order.
run_region() {  # <cmd> <region>
  local cmd="$1" r="$2" s
  for s in $(stems_for "$r"); do run_file "$cmd" "$r" "$s"; done
}

# ---- Dispatch ------------------------------------------------------------

is_region() {
  local r
  for r in "${REGIONS[@]}"; do [[ "$r" == "$1" ]] && return 0; done
  return 1
}

# Run <cmd> across all files of the selected regions, honoring $PARALLEL:
#   file   (default) — one background worker per file (finest granularity)
#   region           — one background worker per region (its files run in order)
#   none             — everything sequentially, on the main shell
# Each worker detaches stdin and tags its log lines with its region. Worker
# failures are collected; exit is non-zero if any unit failed.
run_all() {  # <cmd> <region...>
  local cmd="$1"; shift
  local -a regions=("$@")

  # Build the (region, stem) file list.
  local -a jr=() js=()
  local r s
  for r in "${regions[@]}"; do
    for s in $(stems_for "$r"); do jr+=("$r"); js+=("$s"); done
  done

  # Sequential, or nothing worth parallelizing.
  if [[ "$PARALLEL" == none || ${#jr[@]} -le 1 ]]; then
    for r in "${regions[@]}"; do
      log "===== $cmd: $r ====="
      run_region "$cmd" "$r"
    done
    return 0
  fi

  local -a pids=() labels=()
  local i
  if [[ "$PARALLEL" == region ]]; then
    log "starting ${#regions[@]} regions in parallel (--sequential to disable)"
    for r in "${regions[@]}"; do
      ( REGION="$r"; run_region "$cmd" "$r" ) </dev/null &
      pids+=("$!"); labels+=("$r")
    done
  else  # file (default)
    log "starting ${#jr[@]} files in parallel (--by-region or --sequential to change)"
    for i in "${!jr[@]}"; do
      ( REGION="${jr[$i]}"; run_file "$cmd" "${jr[$i]}" "${js[$i]}" ) </dev/null &
      pids+=("$!"); labels+=("${jr[$i]}/${js[$i]}")
    done
  fi

  local fail=0
  for i in "${!pids[@]}"; do
    if ! wait "${pids[$i]}"; then
      log "'${labels[$i]}' FAILED"; fail=1
    fi
  done
  return "$fail"
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

  run_all "$cmd" "${regions[@]}" || { log "one or more units failed."; return 1; }
  log "done."
}

main ${ARGS[@]+"${ARGS[@]}"}
