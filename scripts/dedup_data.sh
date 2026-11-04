#!/usr/bin/env bash
#
# dedup_data.sh — de-duplicate the cleaned Parquet outputs of clean_data.sh with
# ctddump, for the Arctic, Baltic, and Mediterranean seas. Four steps run in
# order, each reading the previous step's output:
#
#   markdup   Mark duplicate profiles (adds an is_dup column) + write a dups TSV.
#   report    Summarise the marked Parquet (with duplicate counts) as TSV.
#   dedup     Remove duplicates, keeping the profile with the most observations.
#   report    Summarise the de-duplicated Parquet as TSV.
#
# Duplicates are decided by profile_timestamp (date only), longitude, and
# latitude (rounded to 3 decimals) — ctddump's defaults. The stages chain
# through sub-directories of $OUT/dedup:
#   $OUT/clean/filter -> dedup/markdup -> dedup/dedup
# and reports land in $REPORT/dedup/{markdup,dedup}/.
#
# Usage:
#   scripts/dedup_data.sh [options] [command] [region ...]
#
# Commands:
#   markdup     Mark duplicate profiles.
#   report      Summarise the marked and/or de-duplicated Parquet as TSV.
#   dedup       Remove duplicate profiles.
#   all         markdup, report (marked), dedup, then report (deduped).  (default)
#   help        Show this help.
#
# Regions:  arctic  baltic  mediterranean   (default: all three; "all" also works)
#
# Options (may appear anywhere on the command line):
#   -o, --out DIR   root for the clean_data.sh outputs and the de-duplicated
#                   outputs (default: output)
#   -r, --report DIR  root for the summary reports (default: report)
#   --chunk-rows N  streaming chunk size in rows for ctddump; lower uses less
#                   memory but writes more Parquet row groups. Exported as
#                   CTDDUMP_CHUNK_ROWS   (default: ctddump's built-in 1,000,000)
#   --by-region     Parallelise per region instead of per file (coarser: one
#                   worker per region, its files processed in order).
#   --sequential    Process everything one file at a time (no parallelism).
#   --time          Measure each ctddump step with GNU time and log its wall
#                   clock and peak memory. Off by default; needs the `time`
#                   package (not the shell builtin). Peak RSS is per process;
#                   for meaningful wall times pair it with --sequential.
#   --time-log FILE Write the --time measurements to FILE (implies --time), so
#                   the screen keeps only the normal progress output.
#   -y, --yes       Skip the confirmation prompt and start immediately.
#   -h, --help      Show this help.
#
# By default the selected files (a <stem> within a region) are processed in
# parallel, one worker per file. Requires ctddump on PATH, and clean_data.sh's
# cleaned Parquet in <out>/clean/filter.

set -euo pipefail

usage() { awk 'NR<3 {next} /^#/ {sub(/^# ?/, ""); print; next} {exit}' "$0"; }

# ---- Configuration (defaults; override with the options below) -----------
OUT=output
REPORT=report
CHUNK_ROWS=     # empty → ctddump's built-in default (see CTDDUMP_CHUNK_ROWS)
ASSUME_YES=0
PARALLEL=file   # file | region | none
TIMING=0        # 1 → wrap each ctddump step in GNU time (--time)
TIME_LOG=       # non-empty → write timing lines here, not to stderr (--time-log)

# ---- Parse options -------------------------------------------------------
# Options may appear anywhere; the remaining words are the command and regions.
ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o|--out)     OUT="${2:?--out requires a directory}"; shift 2 ;;
    --out=*)      OUT="${1#*=}"; shift ;;
    -r|--report)  REPORT="${2:?--report requires a directory}"; shift 2 ;;
    --report=*)   REPORT="${1#*=}"; shift ;;
    --chunk-rows) CHUNK_ROWS="${2:?--chunk-rows requires a value}"; shift 2 ;;
    --chunk-rows=*) CHUNK_ROWS="${1#*=}"; shift ;;
    --by-region)  PARALLEL=region; shift ;;
    --sequential) PARALLEL=none; shift ;;
    --time)       TIMING=1; shift ;;
    --time-log)   TIME_LOG="${2:?--time-log requires a file}"; TIMING=1; shift 2 ;;
    --time-log=*) TIME_LOG="${1#*=}"; TIMING=1; shift ;;
    -y|--yes)     ASSUME_YES=1; shift ;;
    -h|--help)    usage; exit 0 ;;
    --)           shift; ARGS+=("$@"); break ;;
    -*)           echo "Unknown option: $1" >&2; usage; exit 1 ;;
    *)            ARGS+=("$1"); shift ;;
  esac
done

# A --chunk-rows value is passed to every ctddump child process via the env var
# it reads (CTDDUMP_CHUNK_ROWS); leaving it unset keeps ctddump's built-in default.
[[ -n "$CHUNK_ROWS" ]] && export CTDDUMP_CHUNK_ROWS="$CHUNK_ROWS"

# --time measures each ctddump step with GNU time (wall clock + peak RSS). Resolve
# a GNU-compatible `time` binary (the shell builtin cannot report memory) and
# verify it supports -f up front, so the run fails fast with an install hint rather
# than mid-pipeline. Override the binary with CTDDUMP_TIME_BIN if needed.
TIME_BIN="${CTDDUMP_TIME_BIN:-}"
if [[ "$TIMING" == 1 ]]; then
  if [[ -z "$TIME_BIN" ]]; then
    if [[ -x /usr/bin/time ]]; then
      TIME_BIN=/usr/bin/time
    elif command -v gtime >/dev/null 2>&1; then
      TIME_BIN=$(command -v gtime)
    fi
  fi
  # Probe with -o so we confirm time actually wraps the command and writes stats
  # (a binary that merely ignores -f, or one like /bin/true that swallows every
  # argument, would pass a bare exit-status check yet capture nothing).
  probe=$(mktemp)
  if [[ -z "$TIME_BIN" ]] || ! "$TIME_BIN" -o "$probe" -f '%e' true 2>/dev/null || [[ ! -s "$probe" ]]; then
    rm -f "$probe"
    echo "Error: --time needs GNU time (the 'time' package), not the shell builtin." >&2
    echo "       Install it (e.g. sudo apt-get install time) or set CTDDUMP_TIME_BIN." >&2
    exit 1
  fi
  rm -f "$probe"
fi

# --time-log routes the timing lines to a file (and implies --time). Create it
# fresh up front so a bad path fails now, and parallel workers can append to it.
if [[ -n "$TIME_LOG" ]]; then
  mkdir -p "$(dirname "$TIME_LOG")" 2>/dev/null || true
  if ! : > "$TIME_LOG"; then
    echo "Error: cannot write --time-log file: $TIME_LOG" >&2
    exit 1
  fi
fi

# Stage directories (each step reads the previous one).
SRC_DIR="$OUT/clean/filter"          # clean_data.sh cleaned Parquet (input)
MARK_DIR="$OUT/dedup/markdup"        # marked Parquet + dups TSV
DEDUP_DIR="$OUT/dedup/dedup"         # final de-duplicated data
REPORT_MARK_DIR="$REPORT/dedup/markdup"
REPORT_DEDUP_DIR="$REPORT/dedup/dedup"

REGIONS=(arctic baltic mediterranean)

# Merged-file stems produced by convert_data.sh / carried through clean_data.sh.
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

# Emit one timing line, routed by --time-log: to that file when given (so the
# screen keeps only normal progress), otherwise to the log stream (stderr). The
# file path mirrors log()'s "[time] [region] ..." format; a short single-line
# append is atomic, so parallel workers do not interleave mid-line.
timing_log() {  # <message>
  if [[ -z "$TIME_LOG" ]]; then log "$*"; return; fi
  local p=""
  [[ -n "${REGION:-}" ]] && p="[$REGION] "
  printf '[%s] %s%s\n' "$(date '+%H:%M:%S')" "$p" "$*" >> "$TIME_LOG"
}

# When --time is on, run <cmd...> under GNU time and log its wall-clock seconds and
# peak resident memory; otherwise run <cmd...> unchanged. Each call captures time's
# own output in a private temp file, so it is safe under parallel workers. The
# command's exit status is preserved (propagated to the set -e caller).
measure() {  # <label> <cmd...>
  local label="$1"; shift
  if [[ "$TIMING" != 1 ]]; then
    "$@"
    return
  fi
  local tf rc=0 es rss cpu
  tf=$(mktemp)
  "$TIME_BIN" -o "$tf" -f '%e %M %P' -- "$@" || rc=$?
  if [[ "$rc" -eq 0 ]] && read -r es rss cpu < "$tf"; then
    timing_log "timed $label: ${es}s, $(( (rss + 1023) / 1024 )) MiB peak RSS, $cpu CPU"
  fi
  rm -f "$tf"
  return "$rc"
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
  local timing_desc=off
  if [[ "$TIMING" == 1 ]]; then
    timing_desc="on ($TIME_BIN)"
    if [[ -n "$TIME_LOG" ]]; then timing_desc="$timing_desc -> $TIME_LOG"; fi
  fi
  {
    echo "Configuration:"
    echo "  command : $cmd"
    echo "  regions : ${rs[*]}"
    echo "  files   : $nfiles"
    echo "  out     : $OUT"
    echo "  report  : $REPORT"
    echo "  chunk   : ${CHUNK_ROWS:-default}"
    echo "  mode    : $mode"
    echo "  timing  : $timing_desc"
    echo "Run with -h/--help to see all options."
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

# ---- Per-file steps ------------------------------------------------------
# Each operates on a single merged file (a <stem> within a <region>) and creates
# its output location so a fresh run works from scratch. The <region> argument is
# unused here (kept for a uniform signature with the other scripts).

# A file whose input is missing is skipped (with a note), not an error — so an
# unavailable dataset such as the Baltic Global (GL) product doesn't fail the run.

do_markdup() {  # <region> <stem>
  [[ -f "$SRC_DIR/$2.parquet" ]] || { log "skip markdup $2 (missing input)"; return 0; }
  mkdir -p "$MARK_DIR"; log "markdup $2"
  measure "markdup $2" ctddump markdup "$SRC_DIR/$2.parquet" "$MARK_DIR/$2.parquet" "$MARK_DIR/$2.dups.tsv"
}

do_dedup() {  # <region> <stem>
  [[ -f "$MARK_DIR/$2.parquet" ]] || { log "skip dedup $2 (missing input)"; return 0; }
  mkdir -p "$DEDUP_DIR"; log "dedup $2"
  measure "dedup $2" ctddump dedup "$MARK_DIR/$2.parquet" "$DEDUP_DIR/$2.parquet"
}

do_report_marked() {  # <region> <stem>
  [[ -f "$MARK_DIR/$2.parquet" ]] || { log "skip report (marked) $2 (missing input)"; return 0; }
  mkdir -p "$REPORT_MARK_DIR"; log "report (marked) $2"
  measure "report marked $2" ctddump report parquet --level platform "$MARK_DIR/$2.parquet" "$REPORT_MARK_DIR/$2.parquet.tsv"
}

do_report_deduped() {  # <region> <stem>
  [[ -f "$DEDUP_DIR/$2.parquet" ]] || { log "skip report (deduped) $2 (missing input)"; return 0; }
  mkdir -p "$REPORT_DEDUP_DIR"; log "report (deduped) $2"
  measure "report deduped $2" ctddump report parquet --level platform "$DEDUP_DIR/$2.parquet" "$REPORT_DEDUP_DIR/$2.parquet.tsv"
}

# The standalone `report` command summarises whichever stages exist.
do_report() {  # <region> <stem>
  [[ -d "$MARK_DIR" ]] && do_report_marked "$1" "$2"
  [[ -d "$DEDUP_DIR" ]] && do_report_deduped "$1" "$2"
}

# Run <cmd> for one file. For `all` the steps chain markdup -> report -> dedup ->
# report on that file (each stage reads the previous stage's output).
run_file() {  # <cmd> <region> <stem>
  local cmd="$1" r="$2" s="$3"
  case "$cmd" in
    markdup) do_markdup "$r" "$s" ;;
    dedup)   do_dedup "$r" "$s" ;;
    report)  do_report "$r" "$s" ;;
    all)     do_markdup "$r" "$s"; do_report_marked "$r" "$s"; do_dedup "$r" "$s"; do_report_deduped "$r" "$s" ;;
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
    markdup|dedup|report|all) ;;
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
