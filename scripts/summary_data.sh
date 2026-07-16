#!/usr/bin/env bash
#
# summary_data.sh — build a per-unit summary page (Markdown or HTML) from the TSV
# reports produced by convert_data.sh / clean_data.sh / dedup_data.sh, for the
# Arctic, Baltic, and Mediterranean seas. One page per (region, product) unit —
# the same stems the other scripts use (e.g. nrt_ar_ar, nrt_ar_gl, cora_ar).
#
# Each page collects up to seven sections (File summary, Conversion, the three
# Cleaning stages, and the two Deduplication stages); `ctddump report summary`
# includes only the sections whose report files exist.
#
# Usage:
#   scripts/summary_data.sh [options] [region ...]
#
# Regions:  arctic  baltic  mediterranean   (default: all three; "all" also works)
#
# Options (may appear anywhere on the command line):
#   -r, --report DIR   root of the report/ TSV tree            (default: report)
#   -o, --out DIR      root of the output/ data tree           (default: output)
#                      (holds the markdup .dups.tsv files)
#   -d, --dest DIR     directory for the generated pages        (default: summary)
#   -f, --format FMT   page format: md or html                  (default: md)
#   -y, --yes          Skip the confirmation prompt and start immediately.
#   -h, --help         Show this help.
#
# Each page gets a human-readable title and any product-specific notes; both are
# defined in the "Page text" section below and are the place to edit what a page
# says about a region or dataset. The section prose itself is generic and comes
# from ctddump.
#
# A stem with no report files is skipped (e.g. the not-yet-published Baltic GL),
# not an error. Reports read from <report> and <out> are produced by the earlier
# pipeline scripts. Requires ctddump on PATH.

set -euo pipefail

usage() { awk 'NR<3 {next} /^#/ {sub(/^# ?/, ""); print; next} {exit}' "$0"; }

# ---- Configuration (defaults; override with the options below) -----------
REPORT=report
OUT=output
DEST=summary
FORMAT=md
ASSUME_YES=0

# ---- Parse options -------------------------------------------------------
# Options may appear anywhere; the remaining words are the regions.
ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -r|--report)  REPORT="${2:?--report requires a directory}"; shift 2 ;;
    --report=*)   REPORT="${1#*=}"; shift ;;
    -o|--out)     OUT="${2:?--out requires a directory}"; shift 2 ;;
    --out=*)      OUT="${1#*=}"; shift ;;
    -d|--dest)    DEST="${2:?--dest requires a directory}"; shift 2 ;;
    --dest=*)     DEST="${1#*=}"; shift ;;
    -f|--format)  FORMAT="${2:?--format requires md or html}"; shift 2 ;;
    --format=*)   FORMAT="${1#*=}"; shift ;;
    -y|--yes)     ASSUME_YES=1; shift ;;
    -h|--help)    usage; exit 0 ;;
    --)           shift; ARGS+=("$@"); break ;;
    -*)           echo "Unknown option: $1" >&2; usage; exit 1 ;;
    *)            ARGS+=("$1"); shift ;;
  esac
done

case "$FORMAT" in
  md)   EXT=md ;;
  html) EXT=html ;;
  *)    echo "Unknown format: $FORMAT (use md or html)" >&2; usage; exit 1 ;;
esac

REGIONS=(arctic baltic mediterranean)

# The (region, product) stems produced by convert_data.sh, per region.
stems_for() {  # <region>
  case "$1" in
    arctic)        echo nrt_ar_ar nrt_ar_gl cora_ar ;;
    baltic)        echo nrt_bo_bo nrt_bo_gl cora_bo ;;
    mediterranean) echo nrt_mo_mo nrt_mo_gl cora_mo ;;
  esac
}

# ---- Page text -----------------------------------------------------------
# The page's section prose is generic and lives in ctddump. The title and the
# notes below are the region- and product-specific bits — edit them here.

# Human-readable page title, replacing the default "Summary: <stem>".
title_for() {  # <stem>
  case "$1" in
    nrt_ar_ar) echo "Arctic Ocean: Near Real Time, regional product (AR)" ;;
    nrt_ar_gl) echo "Arctic Ocean: Near Real Time, global product (GL)" ;;
    cora_ar)   echo "Arctic Ocean: CORA reanalysis" ;;
    nrt_bo_bo) echo "Baltic Sea: Near Real Time, regional product (BO)" ;;
    nrt_bo_gl) echo "Baltic Sea: Near Real Time, global product (GL)" ;;
    cora_bo)   echo "Baltic Sea: CORA reanalysis" ;;
    nrt_mo_mo) echo "Mediterranean Sea: Near Real Time, regional product (MO)" ;;
    nrt_mo_gl) echo "Mediterranean Sea: Near Real Time, global product (GL)" ;;
    cora_mo)   echo "Mediterranean Sea: CORA reanalysis" ;;
    *)         echo "Summary: $1" ;;
  esac
}

# Notes shown under the page title, one per line (blank lines are ignored). Each
# becomes a `--note`. Put anything dataset- or region-specific here.
notes_for() {  # <stem>
  # Product-specific note. The *_gl pattern must precede the general nrt_* one.
  case "$1" in
    nrt_*_gl) echo "Global (GL) Near Real Time product restricted to this region's bounding box. It overlaps the regional product, so a profile can appear in both and duplicates are expected." ;;
    nrt_*)    echo "Regional Near Real Time product. Each source file holds a single platform." ;;
    cora_*)   echo "CORA delayed-mode reanalysis. Source files hold many platforms each, and the profiles are re-processed rather than near real time." ;;
  esac
}

# ---- Logging -------------------------------------------------------------
# Announce each step (timestamped, to stderr) so progress is visible. The current
# region tags its lines "[region]".
log() {
  local p=""
  [[ -n "${REGION:-}" ]] && p="[$REGION] "
  printf '[%s] %s%s\n' "$(date '+%H:%M:%S')" "$p" "$*" >&2
}

# Print the resolved configuration, then ask for confirmation unless -y/--yes was
# given. In a non-interactive shell without -y there is nothing to read, so abort
# with a hint rather than hang.
show_config() {  # <region...>
  local -a rs=("$@")
  local nstems=0 r s
  for r in "${rs[@]}"; do for s in $(stems_for "$r"); do nstems=$((nstems + 1)); done; done
  {
    echo "Configuration:"
    echo "  regions : ${rs[*]}"
    echo "  stems   : $nstems"
    echo "  report  : $REPORT"
    echo "  out     : $OUT"
    echo "  dest    : $DEST"
    echo "  format  : $FORMAT"
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

# ---- Per-stem step -------------------------------------------------------
# `report summary` already omits absent sections; skip a stem entirely when none
# of its report files exist (e.g. the unpublished Baltic GL product), so it does
# not emit an empty page.
has_any_report() {  # <stem>
  local s="$1" f
  for f in \
    "$REPORT/header/$s.yaml.tsv" \
    "$REPORT/convert/$s.parquet.tsv" \
    "$REPORT/clean/dropqc/$s.parquet.tsv" \
    "$REPORT/clean/dropna/$s.parquet.tsv" \
    "$REPORT/clean/filter/$s.parquet.tsv" \
    "$REPORT/dedup/markdup/$s.parquet.tsv" \
    "$REPORT/dedup/dedup/$s.parquet.tsv"; do
    [[ -f "$f" ]] && return 0
  done
  return 1
}

do_summary() {  # <stem>
  local s="$1"
  if ! has_any_report "$s"; then log "skip summary $s (no reports)"; return 0; fi
  mkdir -p "$DEST"; log "summary $s -> $DEST/$s.$EXT"

  local -a nargs=()
  local n
  while IFS= read -r n; do
    [[ -n "$n" ]] && nargs+=(--note "$n")
  done < <(notes_for "$s")

  ctddump report summary "$s" --report-dir "$REPORT" --out-dir "$OUT" \
    --format "$FORMAT" --title "$(title_for "$s")" \
    ${nargs[@]+"${nargs[@]}"} -o "$DEST/$s.$EXT"
}

# ---- Dispatch ------------------------------------------------------------
is_region() {
  local r
  for r in "${REGIONS[@]}"; do [[ "$r" == "$1" ]] && return 0; done
  return 1
}

main() {
  [[ "${1:-}" == "help" ]] && { usage; return 0; }

  # Remaining args are regions; default to all, and "all" is an alias.
  local -a regions=("$@")
  if [[ ${#regions[@]} -eq 0 || "${regions[0]}" == "all" ]]; then
    regions=("${REGIONS[@]}")
  fi
  local r s
  for r in "${regions[@]}"; do
    is_region "$r" || { echo "Unknown region: $r" >&2; usage; return 1; }
  done

  show_config "${regions[@]}"
  confirm || { log "aborted."; return 1; }

  # Summaries only read small TSVs, so a plain sequential pass is plenty fast.
  for r in "${regions[@]}"; do
    REGION="$r"
    log "===== summary: $r ====="
    for s in $(stems_for "$r"); do do_summary "$s"; done
  done
  REGION=""
  log "done."
}

main ${ARGS[@]+"${ARGS[@]}"}
