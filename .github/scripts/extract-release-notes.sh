#!/bin/bash
# Extracts (or validates) release notes for a given version from CHANGELOG.md.
# Ported from the pdf_oxide engine's hardened extractor.
#
# Usage:
#   extract-release-notes.sh <version>           # write release-title.txt + release-notes.md
#   extract-release-notes.sh --check <version>   # validate only, write nothing, exit non-zero on problems
#
# A well-formed CHANGELOG section looks like:
#
#   ## [0.1.0] - 2026-05-31
#
#   > One-line (or multi-line) subtitle describing the release.
#
#   ### Added
#   - ...
#
# STRICT: fails loudly — instead of silently producing a stale or bare title —
# when, for the requested version, the `## [VERSION]` section is missing, has
# no anchored `> ...` subtitle directly under the header, or has no `### `
# heading (an empty stub). The subtitle scan is bounded to the version's own
# section and anchored to the blockquote immediately under the header, so it
# can never scrape a different version's subtitle.

set -euo pipefail

CHECK_ONLY=0
if [ "${1:-}" = "--check" ]; then
  CHECK_ONLY=1
  shift
fi

VERSION="${1:?usage: extract-release-notes.sh [--check] <version>}"
CHANGELOG="CHANGELOG.md"

if [ ! -f "$CHANGELOG" ]; then
  echo "::error::$CHANGELOG not found" >&2
  exit 1
fi

# 1. The version section must exist (literal bracket-token compare).
if ! awk -v ver="$VERSION" '
  /^## \[/ { s=$0; sub(/^## \[/,"",s); sub(/\].*/,"",s); if (s==ver) { found=1; exit } }
  END      { exit(found ? 0 : 1) }
' "$CHANGELOG"; then
  echo "::error file=CHANGELOG.md::No '## [$VERSION]' section found in CHANGELOG.md. Add the release section (with a '> subtitle' and '### ' notes) before tagging." >&2
  exit 1
fi

# 2. Anchored subtitle: the contiguous '>' run IMMEDIATELY under the header.
SUBTITLE=$(awk -v ver="$VERSION" '
  function hdrver(line,   s) { s=line; sub(/^## \[/,"",s); sub(/\].*/,"",s); return s }
  /^## \[/ {
    if (hdrver($0) == ver) { in_section=1; pre=1; next }
    if (in_section) exit
    next
  }
  in_section {
    if (pre && $0 ~ /^[ \t]*$/) next
    if (pre) { if ($0 !~ /^>/) exit; pre=0 }
    if ($0 ~ /^>/) { l=$0; sub(/^>[ \t]?/,"",l); st = (st=="" ? l : st " " l) }
    else { exit }
  }
  END { if (st != "") print st }
' "$CHANGELOG")

if [ -z "$SUBTITLE" ]; then
  echo "::error file=CHANGELOG.md::No '> subtitle' blockquote found under '## [$VERSION]' in CHANGELOG.md. Add a one-line (or multi-line) '> ...' subtitle directly below the version header before tagging." >&2
  exit 1
fi

# 3. The section must contain at least one '### ' heading (real notes).
if ! awk -v ver="$VERSION" '
  function hdrver(line,   s) { s=line; sub(/^## \[/,"",s); sub(/\].*/,"",s); return s }
  /^## \[/ { if (hdrver($0)==ver){in_section=1;next} if(in_section) exit; next }
  in_section && /^### / { found=1; exit }
  END { exit(found ? 0 : 1) }
' "$CHANGELOG"; then
  echo "::error file=CHANGELOG.md::Section '## [$VERSION]' has no '### ' heading — it looks like an empty stub. Add the real release notes before tagging." >&2
  exit 1
fi

TITLE="v${VERSION} | ${SUBTITLE}"

if [ "$CHECK_ONLY" -eq 1 ]; then
  echo "CHANGELOG OK for v${VERSION}: ${TITLE}"
  exit 0
fi

echo "$TITLE" > release-title.txt

# Body: this version's section minus ONLY the leading subtitle block.
awk -v ver="$VERSION" '
  function hdrver(line,   s) { s=line; sub(/^## \[/,"",s); sub(/\].*/,"",s); return s }
  /^## \[/ { if (hdrver($0)==ver){in_section=1;phase="lead";next} if(in_section) exit; next }
  in_section {
    if (phase=="lead") {
      if ($0 ~ /^[ \t]*$/) next
      if ($0 ~ /^>/) { phase="sub"; next }
      phase="body"
    }
    if (phase=="sub") {
      if ($0 ~ /^>/) next
      phase="body"
    }
    print
  }
' "$CHANGELOG" \
  | sed '1{/^$/d}' > changelog-section.md

if [ ! -s changelog-section.md ]; then
  echo "::error file=CHANGELOG.md::No changelog body content found for version ${VERSION}" >&2
  rm -f changelog-section.md
  exit 1
fi

cat changelog-section.md > release-notes.md
cat >> release-notes.md << 'FOOTER'

---

### Run it

```bash
docker run --rm -p 8080:8080 ghcr.io/yfedoseev/pdf_oxide:latest
curl -s -F file=@doc.pdf http://localhost:8080/v1/extract/text
```

Pin a digest for reproducibility:
```bash
docker pull ghcr.io/yfedoseev/pdf_oxide@sha256:<digest-from-assets>
```

The image is **multi-arch** (linux/amd64 + linux/arm64), **cosign-signed**
(keyless), and ships an attached **CycloneDX SBOM** + **SLSA build provenance**.

### Verify the image

```bash
cosign verify ghcr.io/yfedoseev/pdf_oxide:VERSION_TAG \
  --certificate-identity-regexp 'https://github.com/yfedoseev/pdf_oxide_api/.*' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

### API contract
- OpenAPI 3.1: `GET /openapi.json` · interactive docs: `GET /docs`
- Versions: `GET /version` (reports the embedded `pdf_oxide` engine version)

### Changelog
See [CHANGELOG.md](https://github.com/yfedoseev/pdf_oxide_api/blob/main/CHANGELOG.md) for full history.
FOOTER

rm -f changelog-section.md

echo "Generated release-title.txt and release-notes.md for v${VERSION}"
echo "Title: ${TITLE}"
