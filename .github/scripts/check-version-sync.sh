#!/bin/bash
# Verifies that EVERY version-bearing file in the repo agrees on a single
# version string. This project mirrors the wrapped engine: the API release
# version == the linked `pdf_oxide` version, and that one number is repeated
# across the package manifest, the lockfile, the OpenAPI contract, the GEO/LLM
# discovery file, and the CHANGELOG. Drift between any of these is a release
# bug (e.g. commit 48a8cab bumped Cargo.toml to 0.3.61 but left openapi.* and
# llms.txt on 0.1.0 — this check would have caught it).
#
# The canonical version is the `[package] version` in Cargo.toml. Every other
# location is asserted equal to it.
#
# Usage:
#   check-version-sync.sh            # verify all locations agree; exit 1 on drift
#   check-version-sync.sh --quiet    # only print on failure
#
# Exit codes: 0 = all in sync, 1 = drift found, 2 = a file/field was missing.

set -euo pipefail

QUIET=0
[ "${1:-}" = "--quiet" ] && QUIET=1

# Resolve repo root so the script works from anywhere (CI checkout or local).
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

fail=0
note() { [ "$QUIET" = "1" ] || printf '%s\n' "$*"; }

# --- canonical version: [package] version in Cargo.toml -----------------------
CANON="$(grep -m1 -E '^version = ' Cargo.toml | sed -E 's/version = "([^"]+)".*/\1/')"
if [ -z "$CANON" ]; then
  echo "FATAL: could not read [package] version from Cargo.toml" >&2
  exit 2
fi
note "Canonical version (Cargo.toml [package]): $CANON"
note ""

# check <label> <actual>   — compare a discovered value against the canonical one
check() {
  local label="$1" actual="$2"
  if [ -z "$actual" ]; then
    echo "  MISSING  $label — could not extract a version (expected $CANON)" >&2
    fail=2
  elif [ "$actual" != "$CANON" ]; then
    echo "  DRIFT    $label = $actual  (expected $CANON)" >&2
    fail=${fail:-1}; [ "$fail" = "0" ] && fail=1
  else
    note "  ok       $label = $actual"
  fi
}

# --- Cargo.toml: the pdf_oxide dependency (the engine we mirror) ---------------
# Matches both `pdf_oxide = "0.3.63"` and the table form `pdf_oxide = { version = "..." }`.
DEP="$(grep -m1 -E '^pdf_oxide = ' Cargo.toml | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)"
check "Cargo.toml  pdf_oxide dependency" "$DEP"

# --- Cargo.lock: the resolved pdf_oxide version -------------------------------
LOCK="$(awk '/^name = "pdf_oxide"$/{getline; if ($1=="version") {gsub(/"/,"",$3); print $3}}' Cargo.lock | head -1)"
check "Cargo.lock  pdf_oxide resolved" "$LOCK"

# --- openapi.yaml: info.version -----------------------------------------------
OAPI_YAML="$(grep -m1 -E '^  version:' openapi.yaml | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)"
check "openapi.yaml info.version" "$OAPI_YAML"

# --- openapi.json: info.version -----------------------------------------------
OAPI_JSON="$(grep -m1 -E '"version":' openapi.json | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)"
check "openapi.json info.version" "$OAPI_JSON"

# --- llms.txt: every pinned version must match (the prose line AND the docker
#     pull tags). We assert EVERY 0.x.y in the version-bearing lines equals
#     CANON, so a half-updated llms.txt (some tags bumped, some not) still fails.
LLMS_BAD="$(grep -nE 'Current version:|Engine: pdf_oxide|pull .*pdf_oxide:[0-9]' llms.txt \
  | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | grep -vx "$CANON" | head -1 || true)"
if [ -n "$LLMS_BAD" ]; then
  echo "  DRIFT    llms.txt has a pinned version $LLMS_BAD (expected $CANON)" >&2
  fail=1
else
  # Confirm CANON actually appears (guard against a typo'd grep matching nothing).
  if grep -qE "Current version: $CANON" llms.txt; then
    note "  ok       llms.txt pinned versions = $CANON"
  else
    echo "  MISSING  llms.txt 'Current version: $CANON' line not found" >&2
    fail=2
  fi
fi

# --- CHANGELOG.md: the latest released section header must be the canonical
#     version (the top-most `## [x.y.z]` below `## [Unreleased]`). -------------
CL="$(grep -m1 -E '^## \[[0-9]+\.[0-9]+\.[0-9]+\]' CHANGELOG.md | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)"
check "CHANGELOG.md latest release entry" "$CL"

note ""
if [ "$fail" = "0" ]; then
  note "All version locations are in sync at $CANON."
  exit 0
elif [ "$fail" = "2" ]; then
  echo "FAILED: a version field was missing (see above)." >&2
  exit 2
else
  echo "FAILED: version drift detected — every location must equal $CANON." >&2
  exit 1
fi
