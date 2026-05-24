#!/usr/bin/env bash
#
# Fetch and extract the ctddump test data fixtures into tests/.
#
# This data lives in a GitHub Release rather than in the repository so the
# library distribution stays small.
#
# Usage:
#   scripts/fetch_test_data.sh
#
# Override the defaults via environment if needed:
#   TEST_DATA_VERSION=v1.0.0 scripts/fetch_test_data.sh
#   TEST_DATA_REPO=YourFork/ctddump    scripts/fetch_test_data.sh
#
# Requirements:
#   - gh CLI    https://cli.github.com  (authenticated: `gh auth login`)
#   - unzip
#
set -euo pipefail

VERSION="${TEST_DATA_VERSION:-v0.1.0}"
REPO="${TEST_DATA_REPO:-AIQC-Hub/ctddump}"

# Resolve repo root from this script's location so it works from anywhere.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TARGET_DIR="$REPO_ROOT/tests"

if ! command -v gh >/dev/null 2>&1; then
  echo "Error: 'gh' CLI not found. Install from https://cli.github.com" >&2
  exit 1
fi
if ! command -v unzip >/dev/null 2>&1; then
  echo "Error: 'unzip' not found. Install via your package manager." >&2
  exit 1
fi

mkdir -p "$TARGET_DIR"

TMP_ZIP="$(mktemp -t cdtdump-test-data.XXXXXX.zip)"
trap 'rm -f "$TMP_ZIP"' EXIT

echo "Downloading test-data.zip from release '$VERSION' of $REPO ..."
gh release download "$VERSION" \
  --repo "$REPO" \
  --pattern 'test_data.zip'

echo "Extracting to $TARGET_DIR ..."
unzip -q -o "$TMP_ZIP" -d "$TARGET_DIR"

n_files="$(find "$TARGET_DIR" -type f | wc -l | tr -d ' ')"
echo "Done. $n_files files placed in $TARGET_DIR"