#!/usr/bin/env bash
# Black-box integration suite for a RUNNING pdf_oxide_api instance.
#
# Exercises every endpoint over real HTTP (not in-process), so it validates the
# packaged container / a live deployment end-to-end. Used by CI (against the
# freshly-built image) and runnable by anyone against any instance:
#
#   BASE_URL=http://localhost:8080 ./scripts/integration-test.sh
#   API_KEY=secret BASE_URL=https://pdf.example.com ./scripts/integration-test.sh
#
# Requires: bash, curl, python3. Uses the committed fixtures in tests/fixtures.

set -uo pipefail

BASE_URL="${BASE_URL:-http://localhost:8080}"
API_KEY="${API_KEY:-}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HELLO="$HERE/tests/fixtures/hello.pdf"
FORM="$HERE/tests/fixtures/japanese_form.pdf"

auth=()
[ -n "$API_KEY" ] && auth=(-H "authorization: Bearer $API_KEY")

pass=0 fail=0
ok()   { echo "  ✓ $1"; pass=$((pass+1)); }
bad()  { echo "  ✗ $1"; fail=$((fail+1)); }

# expect_status <name> <expected> <curl-args...>
expect_status() {
  local name="$1" want="$2"; shift 2
  local got
  got=$(curl -s -o /dev/null -w '%{http_code}' "${auth[@]}" "$@")
  [ "$got" = "$want" ] && ok "$name ($got)" || bad "$name (want $want, got $got)"
}

# expect_json_key <name> <json-path> <substr> <curl-args...>
expect_json_contains() {
  local name="$1" key="$2" want="$3"; shift 3
  local body
  body=$(curl -s "${auth[@]}" "$@")
  if echo "$body" | python3 -c "import sys,json; d=json.load(sys.stdin); v=d
for k in '$key'.split('.'):
    v = v[int(k)] if k.isdigit() else v[k]
sys.exit(0 if '$want' in str(v) else 1)" 2>/dev/null; then
    ok "$name"
  else
    bad "$name (key '$key' lacked '$want'); body: ${body:0:160}"
  fi
}

b64() { base64 -w0 "$1" 2>/dev/null || base64 "$1"; }

echo "== integration suite against $BASE_URL =="

# --- operational ---
expect_status   "healthz 200"        200 "$BASE_URL/healthz"
expect_status   "readyz 200"         200 "$BASE_URL/readyz"
expect_status   "metrics 200"        200 "$BASE_URL/metrics"
expect_status   "openapi.json 200"   200 "$BASE_URL/openapi.json"
expect_json_contains "version reports engine" "pdf_oxide_version" "0" "$BASE_URL/version"

# --- extraction (multipart + json) ---
expect_json_contains "extract text (multipart)" "text" "Hello" \
  -F "file=@$HELLO" "$BASE_URL/v1/extract/text"
expect_json_contains "extract text (json b64)" "text" "Hello" \
  -H 'content-type: application/json' \
  -d "{\"pdf_base64\":\"$(b64 "$HELLO")\"}" "$BASE_URL/v1/extract/text"
expect_json_contains "extract markdown" "markdown" "Hello" \
  -F "file=@$HELLO" "$BASE_URL/v1/extract/markdown"
expect_json_contains "extract html" "html" "<" \
  -F "file=@$HELLO" "$BASE_URL/v1/extract/html"

# --- forms ---
expect_json_contains "forms fields lists 2" "field_count" "2" \
  -F "file=@$FORM" "$BASE_URL/v1/forms/fields"
expect_status "forms fill returns pdf 200" 200 \
  -H 'content-type: application/json' \
  -d "{\"pdf_base64\":\"$(b64 "$FORM")\",\"fields\":{\"full_name\":\"山田太郎\"}}" \
  "$BASE_URL/v1/forms/fill"

# --- document ops ---
expect_status "merge 200" 200 \
  -F "file=@$HELLO" -F "file=@$HELLO" "$BASE_URL/v1/docs/merge"
expect_status "split 200 (zip)" 200 -F "file=@$HELLO" "$BASE_URL/v1/docs/split"
expect_status "metadata 200" 200 -F "file=@$HELLO" "$BASE_URL/v1/docs/metadata"
expect_json_contains "page-info page_count" "page_count" "1" \
  -F "file=@$HELLO" "$BASE_URL/v1/docs/page-info"

# --- pipeline ---
expect_json_contains "pipeline page_info" "page_count" "1" \
  -H 'content-type: application/json' \
  -d "{\"pdf_base64\":\"$(b64 "$HELLO")\",\"operations\":[{\"op\":\"page_info\"}]}" \
  "$BASE_URL/v1/pipeline"

# --- error contract ---
expect_status "no pdf -> 400" 400 \
  -H 'content-type: application/json' -d '{}' "$BASE_URL/v1/extract/text"
expect_status "garbage -> 422" 422 \
  -H 'content-type: application/json' \
  -d "{\"pdf_base64\":\"$(echo -n 'not a pdf' | base64)\"}" "$BASE_URL/v1/extract/text"

echo "== $pass passed, $fail failed =="
[ "$fail" -eq 0 ]
